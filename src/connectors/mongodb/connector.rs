use crate::{
    connectors::base::{Connector, ConnectorInfo, DatabaseData, PaginationInfo, TableData},
    widgets::scrollable_table::Row,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime};
use futures::stream::TryStreamExt;
use mongodb::{
    bson::{self, doc, Bson, Document},
    options::{AggregateOptions, ClientOptions, FindOptions},
    Client,
};
use regex::Regex;
use serde_json::{Map, Value};
use std::{collections::HashMap, str::FromStr, sync::Arc, time::SystemTime};

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

        Ok(MongodbConnector { info, client })
    }
}

pub struct MongodbConnector {
    info: ConnectorInfo,
    client: Client,
}

impl MongodbConnector {}

const COMMAND_REGEX: &str = r#"db.([A-z0-9"]+).([A-z0-9"]+)\((.*)\)"#;
const KEY_TO_STRING_REGEX: &str = r"(\$?[A-z0-9]+)(?::)";
const REGEX_TO_STRING_REGEX: &str = r"\/([A-z0-9]+)(?:\/)";
const DATE_TO_STRING_REGEX: &str = r"(Date\(([A-z0-9-\/]+?)\))";
const MAXIMUM_DOCUMENTS: usize = 1_000;

enum CommandType {
    Find,
    Aggregate,
    Count,
}

impl FromStr for CommandType {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let l_s = s.to_lowercase();
        match l_s.as_str() {
            "find" => Ok(CommandType::Find),
            "aggregate" => Ok(CommandType::Aggregate),
            "count" => Ok(CommandType::Count),
            _ => Err(()),
        }
    }
}

#[async_trait]
impl Connector for MongodbConnector {
    async fn get_info(&self) -> &crate::connectors::base::ConnectorInfo {
        &self.info
    }

    async fn get_data(&self, str: &str, (start, limit): PaginationInfo) -> Result<DatabaseData> {
        let (collection_name, command_type, query) = Regex::new(COMMAND_REGEX)?
            .captures(str)
            .map(|m| {
                let collection_name = m.get(1).unwrap().as_str();
                let command_type = CommandType::from_str(m.get(2).unwrap().as_str()).unwrap();
                let query = if let Some(query) = m.get(3) {
                    query.as_str().to_string()
                } else {
                    String::from("{}")
                };

                (collection_name, command_type, query)
            })
            .with_context(|| format!("'{}' is not valid mongo query!", str))?;
        let db = self.client.database("reas-dealer");
        let collection: mongodb::Collection<Document> = db.collection(collection_name);

        let mut str_fixed = Regex::new(KEY_TO_STRING_REGEX)?
            .replace_all(&query, "\"$1\":")
            .to_string();
        str_fixed = Regex::new(REGEX_TO_STRING_REGEX)?
            .replace_all(&str_fixed, "\"/$1/\"")
            .to_string();
        str_fixed = Regex::new(DATE_TO_STRING_REGEX)?
            .replace_all(&str_fixed, "\"$1\"")
            .to_string();
        let value: Map<String, Value> = serde_json::from_str(&str_fixed).unwrap();
        let mut bson = Document::try_from(value).unwrap();
        bson.iter_mut().for_each(|(_, value)| resolve(value));

        let mut result = DatabaseData(Vec::new());
        let mut cursor = match command_type {
            CommandType::Find => {
                let mut opt = FindOptions::default();
                opt.skip = Some(start);
                opt.limit = Some(limit);
                collection.find(bson, opt).await?
            }
            CommandType::Aggregate => {
                let opt = AggregateOptions::default();
                collection.aggregate(vec![bson], opt).await?
            }
            CommandType::Count => {
                let opt = AggregateOptions::default();
                let mut match_bson = Document::new();
                match_bson.insert("$match", &bson);
                collection
                    .aggregate(vec![match_bson, doc! {"$count": "count"}], opt)
                    .await?
            }
        };

        while let Some(doc) = cursor.try_next().await? {
            result.push(serde_json::to_value(doc)?);
            if result.len() >= MAXIMUM_DOCUMENTS {
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
            let keys = value[0]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            {
                header = Row::new(keys.clone());
                // TODO: Keys could be in wrong order
                body = value
                    .iter()
                    .cloned()
                    .map(|x| {
                        let mut cloned = x.clone();
                        let obj = cloned.as_object_mut().unwrap();
                        let mut parsed_obj = HashMap::new();
                        obj.iter().for_each(|(key, value)| {
                            let parsed_value = match value {
                                serde_json::Value::Object(v) => {
                                    let bson = Bson::try_from(v.clone()).unwrap();
                                    if let Some(date) = bson.as_datetime() {
                                        date.try_to_rfc3339_string().unwrap()
                                    } else {
                                        value.to_string()
                                    }
                                }
                                v => v.to_string(),
                            };
                            parsed_obj.insert(key, parsed_value);
                        });
                        Row::new(
                            keys.iter()
                                .filter(|key| parsed_obj.get(key.to_owned()).is_some())
                                .map(|key| parsed_obj.get(key).unwrap().to_string()),
                        )
                    })
                    .collect::<Vec<Row>>();
            }
        }

        TableData { header, rows: body }
    }
}

fn resolve(value: &mut Bson) {
    match value {
        Bson::String(str) => {
            if let Some(result) = Regex::new(REGEX_TO_STRING_REGEX).unwrap().captures(str) {
                *value = mongodb::bson::Bson::RegularExpression(bson::Regex {
                    pattern: result.get(1).unwrap().as_str().to_string(),
                    options: String::new(),
                });
            } else if let Some(result) = Regex::new(DATE_TO_STRING_REGEX).unwrap().captures(str) {
                let raw_date = result.get(2).unwrap().as_str().to_string();

                let date_time = match NaiveDate::parse_from_str(&raw_date, "%Y-%m-%d") {
                    Ok(parsed_date) => {
                        // Create a NaiveDateTime at midnight for the given date
                        NaiveDateTime::new(
                            parsed_date,
                            NaiveTime::from_num_seconds_from_midnight_opt(0, 0).unwrap(),
                        )
                    }
                    Err(e) => {
                        panic!("Failed to parse date: {}", e);
                    }
                };

                let date = DateTime::from_timestamp(date_time.timestamp(), 0).unwrap();
                *value =
                    mongodb::bson::Bson::DateTime(bson::DateTime::from(SystemTime::from(date)));
            }
        }
        Bson::Document(doc) => doc.iter_mut().for_each(|(_, v)| resolve(v)),
        _ => {}
    }
}
