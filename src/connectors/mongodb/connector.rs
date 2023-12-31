use crate::{
    connectors::base::{Connector, ConnectorInfo, DatabaseData, PaginationInfo, TableData},
    widgets::scrollable_table::Row,
};
use anyhow::{anyhow, Context, Result};
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
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::SystemTime,
};

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

const COMMAND_REGEX: &str = r#"db.([A-z0-9"]+).(.*)"#;
const KEY_TO_STRING_REGEX: &str = r"(\$?[A-z0-9]+)(?::)";
const REGEX_TO_STRING_REGEX: &str = r"\/([A-z0-9]+)(?:\/)";
const DATE_TO_STRING_REGEX: &str = r"(Date\(([A-z0-9-\/]+?)\))";
const MAXIMUM_DOCUMENTS: usize = 1_000;

#[derive(Debug)]
enum CommandType {
    Find,
    Aggregate,
    Count,
}

enum SubCommandType {
    Count,
    Sort,
}

impl SubCommandType {
    fn from_str(s: &str) -> Result<SubCommandType> {
        let l_s = s.to_lowercase();
        match l_s.as_str() {
            "sort" => Ok(SubCommandType::Sort),
            "count" => Ok(SubCommandType::Count),
            _ => Err(anyhow!("Invalid command type")),
        }
    }
}

impl CommandType {
    fn from_str(s: &str) -> Result<CommandType> {
        let l_s = s.to_lowercase();
        match l_s.as_str() {
            "find" => Ok(CommandType::Find),
            "aggregate" => Ok(CommandType::Aggregate),
            "count" => Ok(CommandType::Count),
            _ => Err(anyhow!("Invalid command type")),
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
        str: &str,
        PaginationInfo { start, limit }: &PaginationInfo,
    ) -> Result<DatabaseData> {
        let (collection_name, commands, sub_commands) = Regex::new(COMMAND_REGEX)?
            .captures(str)
            .map(|m| {
                let collection_name = m
                    .get(1)
                    .context("Did not find collection name in the query")?
                    .as_str();

                let raw_command = m
                    .get(2)
                    .context("Did not find command type in the query")?
                    .as_str();

                let mut inside_str = false;
                let mut args = Vec::new();
                let mut command = String::new();
                let mut brackets = Vec::new();
                let chars: Vec<char> = raw_command.chars().collect();
                for (idx, ch) in chars.iter().cloned().enumerate() {
                    command += &ch.to_string();
                    let is_escaped = if idx > 0 {
                        chars[idx - 1] == '\\'
                    } else {
                        false
                    };
                    match ch {
                        '(' => {
                            if !inside_str && !is_escaped {
                                if brackets.is_empty() {
                                    command.pop();
                                    args.push(command.to_string());
                                    command.clear();
                                }
                                brackets.push(ch);
                            }
                        }
                        ')' => {
                            if !inside_str && !is_escaped {
                                brackets.pop();
                                if brackets.is_empty() {
                                    command.pop();
                                    args.push(command.to_string());
                                    command.clear();
                                }
                            }
                        }
                        '"' | '\'' => {
                            if !is_escaped {
                                inside_str = !inside_str;
                            }
                        }
                        '.' => {
                            if brackets.is_empty() {
                                command.clear();
                            };
                        }
                        _ => {}
                    }
                }

                let main_command_args = (
                    CommandType::from_str(&args[0].clone()).unwrap(),
                    validate_query(&args[1].clone()).unwrap(),
                );
                let sub_commands = args.chunks(2).skip(1).try_fold(Vec::new(), |mut acc, w| {
                    let query = validate_query(&w[1])?;
                    let command = SubCommandType::from_str(&w[0])?;
                    acc.push((command, query));
                    anyhow::Ok(acc)
                })?;

                anyhow::Ok((collection_name.to_string(), main_command_args, sub_commands))
            })
            .with_context(|| format!("'{}' is not valid mongo query!", str))??;
        let db = self.client.database(&self.database);
        let collection: mongodb::Collection<Document> = db.collection(&collection_name);

        let (command_type, query) = commands;

        let mut cursor = match command_type {
            CommandType::Find => {
                let mut opt = FindOptions::default();
                opt.skip = Some(*start);
                opt.limit = Some(*limit as i64);

                let mut return_count = false;
                sub_commands.iter().for_each(|(cmd, query)| match cmd {
                    SubCommandType::Count => {
                        return_count = true;
                    }
                    SubCommandType::Sort => {
                        opt.sort = Some(query.clone());
                    }
                });

                if return_count {
                    let mut match_query = Document::new();
                    match_query.insert("$match", &query);
                    collection
                        .aggregate(
                            vec![match_query, doc! {"$count": "count"}],
                            AggregateOptions::default(),
                        )
                        .await?
                } else {
                    collection.find(query, opt).await?
                }
            }
            CommandType::Aggregate => {
                let opt = AggregateOptions::default();
                collection.aggregate(vec![query], opt).await?
            }
            CommandType::Count => {
                let opt = AggregateOptions::default();
                let mut match_query = Document::new();
                match_query.insert("$match", &query);
                collection
                    .aggregate(vec![match_query, doc! {"$count": "count"}], opt)
                    .await?
            }
        };

        let mut result = DatabaseData(Vec::new());

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

fn validate_query(query: &str) -> Result<Document> {
    if query.is_empty() {
        Ok(Document::new())
    } else {
        let mut str_fixed = Regex::new(KEY_TO_STRING_REGEX)?
            .replace_all(&query, "\"$1\":")
            .to_string();
        str_fixed = Regex::new(REGEX_TO_STRING_REGEX)?
            .replace_all(&str_fixed, "\"/$1/\"")
            .to_string();
        str_fixed = Regex::new(DATE_TO_STRING_REGEX)?
            .replace_all(&str_fixed, "\"$1\"")
            .to_string();
        let value: Map<String, Value> = serde_json::from_str(&str_fixed)
            .with_context(|| format!("'{}' is not valid mongo query!", &str_fixed))?;
        let mut bson = Document::try_from(value)?;
        bson.iter_mut().for_each(|(_, value)| resolve(value));
        Ok(bson)
    }
}
