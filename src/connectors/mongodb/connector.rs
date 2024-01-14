use super::parser::{parse_mongo_query, CommandQuery, CommandQueryPair, ParsedValue};
use crate::{
    connectors::base::{Connector, ConnectorInfo, DatabaseData, PaginationInfo, TableData},
    widgets::scrollable_table::Row,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use mongodb::{
    bson::{doc, Bson, Document},
    options::{AggregateOptions, ClientOptions, FindOptions},
    Client,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio_stream::StreamExt;

pub struct MongodbConnectorBuilder {
    info: Option<ConnectorInfo>,
}

impl MongodbConnectorBuilder {
    pub fn new(uri: &str) -> Self {
        Self {
            info: Some(ConnectorInfo {
                uri: uri.to_owned(),
            }),
        }
    }

    pub async fn build(self) -> Result<MongodbConnector> {
        let info = self.info.unwrap();
        let client_opts = ClientOptions::parse(info.uri.clone()).await?;
        let client = Client::with_options(client_opts)?;

        Ok(MongodbConnector {
            info,
            client,
            database: String::from("admin"),
        })
    }
}

pub struct MongodbConnector {
    info: ConnectorInfo,
    client: Client,
    database: String,
}

// TODO: Replace ALL regexes - they does not work in certain cases
pub const KEY_TO_STRING_REGEX: &str = r"(\$?[A-z0-9]+)(?::)";
pub const REGEX_TO_STRING_REGEX: &str = r"\/([A-z0-9]+)(?:\/)";
pub const DATE_TO_STRING_REGEX: &str = r##"(Date\(([A-z0-9-\/]+?)\))"##;
pub const OBJECT_ID_TO_STRING_REGEX: &str = r##"(ObjectId\(([A-z0-9-\/]+?)\))"##;
const MAXIMUM_DOCUMENTS: u32 = 100;

pub enum CommandType {
    MainCommand(MainCommand),
    SubCommand(SubCommand),
}

#[derive(Debug)]
pub enum MainCommand {
    Find,
    Aggregate,
    Count,
}

pub enum SubCommand {
    Count,
    Sort,
    AllowDiskUse,
}

impl SubCommand {
    pub fn from_str(s: &str) -> Result<SubCommand> {
        let l_s = s.to_lowercase();
        match l_s.as_str() {
            "sort" => Ok(SubCommand::Sort),
            "count" => Ok(SubCommand::Count),
            "allowdiskuse" => Ok(SubCommand::AllowDiskUse),
            _ => Err(anyhow!(
                "Expected valid sub command type, found '{}' instead.",
                s
            )),
        }
    }
}

impl MainCommand {
    pub fn from_str(s: &str) -> Result<MainCommand> {
        let l_s = s.to_lowercase();
        match l_s.as_str() {
            "find" => Ok(MainCommand::Find),
            "aggregate" => Ok(MainCommand::Aggregate),
            "count" => Ok(MainCommand::Count),
            _ => Err(anyhow!(
                "Expected valid command type, found '{}' instead.",
                s
            )),
        }
    }
}

#[async_trait]
impl Connector for MongodbConnector {
    fn set_database(&mut self, database: &str) {
        self.database = String::from(database);
    }

    async fn get_info(&self) -> &crate::connectors::base::ConnectorInfo {
        &self.info
    }

    async fn get_data(
        &self,
        str: String,
        PaginationInfo { start, limit }: PaginationInfo,
    ) -> Result<DatabaseData> {
        let parsed_value = parse_mongo_query(&str)?;
        let db = self.client.database(&self.database);

        let ParsedValue::Query(CommandQuery {
            collection_name,
            mut command,
            mut sub_commands,
        }) = parsed_value;

        let collection: mongodb::Collection<Document> = db.collection(&collection_name);

        let mut cursor = match command.command_type {
            MainCommand::Find => {
                let mut opt = FindOptions::builder().batch_size(MAXIMUM_DOCUMENTS).build();
                opt.skip = Some(start);
                opt.limit = Some(limit as i64);
                opt.projection = command.query.get(1).cloned();

                let mut return_count = false;
                while let Some(CommandQueryPair {
                    command_type,
                    mut query,
                }) = sub_commands.pop()
                {
                    match command_type {
                        SubCommand::Count => {
                            return_count = true;
                        }
                        SubCommand::Sort => {
                            opt.sort = Some(query.remove(0));
                        }
                        SubCommand::AllowDiskUse => {
                            opt.allow_disk_use = Some(true);
                        }
                    }
                }

                if return_count {
                    let mut match_query = Document::new();
                    match_query.insert("$match", command.query.get(0));
                    collection
                        .aggregate(
                            vec![match_query, doc! {"$count": "count"}],
                            AggregateOptions::builder()
                                .batch_size(MAXIMUM_DOCUMENTS)
                                .build(),
                        )
                        .await?
                } else {
                    collection.find(command.query[0].clone(), opt).await?
                }
            }
            MainCommand::Aggregate => {
                let opt = AggregateOptions::builder()
                    .batch_size(MAXIMUM_DOCUMENTS)
                    .build();
                command.query.append(&mut vec![
                    doc! {"$skip": start as i32},
                    doc! {
                    "$limit": limit as i32
                    },
                ]);
                collection.aggregate(command.query, opt).await?
            }
            MainCommand::Count => {
                let opt = AggregateOptions::builder()
                    .batch_size(MAXIMUM_DOCUMENTS)
                    .build();
                let mut match_query = Document::new();
                match_query.insert("$match", command.query.get(0));
                collection
                    .aggregate(vec![match_query, doc! {"$count": "count"}], opt)
                    .await?
            }
        };

        let mut result = DatabaseData(Vec::new());

        while let Some(doc) = cursor.try_next().await? {
            result.push(serde_json::to_value(doc)?);
            if result.len() >= MAXIMUM_DOCUMENTS as usize {
                break;
            }
        }

        Ok(result)
    }
}

impl<'a> From<Arc<DatabaseData>> for TableData<'a> {
    fn from(value: Arc<DatabaseData>) -> Self {
        let mut header = Row::default();
        let mut body = Vec::new();

        if !value.is_empty() {
            let mut unique_keys = HashSet::new();
            let keys: Vec<String> = value
                .iter()
                .fold(Vec::new(), |mut acc, value| {
                    let keys: Vec<String> = value
                        .as_object()
                        .unwrap()
                        .keys()
                        .filter(|key| !unique_keys.contains(*key))
                        .cloned()
                        .collect();
                    acc.extend::<Vec<String>>(keys.clone());
                    unique_keys.extend(keys);
                    acc
                })
                .to_vec();
            {
                header = Row::new(keys.clone());
                body = value
                    .iter()
                    .cloned()
                    .map(|x| {
                        let mut cloned = x.clone();
                        let obj = cloned.as_object_mut().unwrap();
                        let mut parsed_obj = HashMap::new();
                        keys.iter().for_each(|key| {
                            let mut parsed_value = String::new();
                            if let Some(value) = obj.get(key) {
                                parsed_value = match value {
                                    serde_json::Value::Object(v) => {
                                        let bson = Bson::try_from(v.clone()).unwrap();
                                        if let Some(date) = bson.as_datetime() {
                                            date.try_to_rfc3339_string().unwrap()
                                        } else if let Some(object_id) = bson.as_object_id() {
                                            object_id.to_hex()
                                        } else {
                                            value.to_string()
                                        }
                                    }
                                    v => v.to_string(),
                                };
                            }
                            parsed_obj.insert(key, parsed_value);
                        });
                        Row::new(
                            keys.iter()
                                .filter(|key| parsed_obj.get(key.to_owned()).is_some())
                                .map(|key| String::from(parsed_obj.get(key).unwrap())),
                        )
                    })
                    .collect::<Vec<Row>>();
            }
        }

        TableData { header, rows: body }
    }
}
