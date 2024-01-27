use super::interpreter::InterpreterMongo;
use crate::{
    connectors::base::{Connector, ConnectorInfo, DatabaseData, PaginationInfo, TableData},
    try_from,
    widgets::scrollable_table::Row,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use mongodb::{
    bson::{doc, to_bson, Bson, Document},
    options::{AggregateOptions, ClientOptions, CountOptions, FindOptions},
    Client, Collection, Cursor, Database,
};
use rusty_db_cli_mongo::{
    interpreter::InterpreterError,
    parser::{ArrayExpression, ObjectExpression, ParametersExpression},
    to_interpter_error,
};
use std::collections::{HashMap, HashSet};

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
    pub client: Client,
    database: String,
}

// TODO: Replace ALL regexes - they does not work in certain cases
pub const KEY_TO_STRING_REGEX: &str = r"(\$?[A-z0-9]+)(?::)";
pub const REGEX_TO_STRING_REGEX: &str = r"\/([A-z0-9]+)(?:\/)";
pub const DATE_TO_STRING_REGEX: &str = r##"(Date\(([A-z0-9-\/]+?)\))"##;
pub const OBJECT_ID_TO_STRING_REGEX: &str = r##"(ObjectId\(([A-z0-9-\/]+?)\))"##;

impl TryFrom<(String, ParametersExpression)> for Command {
    type Error = InterpreterError;

    fn try_from((command, params): (String, ParametersExpression)) -> Result<Self, Self::Error> {
        match command.to_lowercase().as_str() {
            "find" => {
                if params.params.len() > 2 {
                    return Err(InterpreterError {
                        message: "Find {} only accepts 2 parameters".to_string(),
                    });
                }

                let filter = params.get_nth_of_type::<ObjectExpression>(0).ok();
                let projection = params.get_nth_of_type::<ObjectExpression>(1).ok();

                let mut opts = FindOptions::default();
                if let Bson::Document(doc) = to_interpter_error!(to_bson(&projection))? {
                    opts.projection = Some(doc);
                }

                if filter.is_some() && !filter.as_ref().unwrap().properties.is_empty() {
                    if let Bson::Document(doc) = to_interpter_error!(to_bson(&filter))? {
                        return Ok(Command::Find(FindQuery {
                            options: opts,
                            filter: Some(doc),
                            ..Default::default()
                        }));
                    }
                }

                Ok(Command::Find(FindQuery {
                    options: opts,
                    ..Default::default()
                }))
            }
            "count" => {
                let filter = params.get_nth_of_type::<ObjectExpression>(0).ok();

                if filter.is_some() && !filter.as_ref().unwrap().properties.is_empty() {
                    if let Bson::Document(doc) = to_interpter_error!(to_bson(&filter))? {
                        return Ok(Command::Count(CountQuery {
                            filter: Some(doc),
                            ..Default::default()
                        }));
                    }
                }

                Ok(Command::Count(CountQuery {
                    ..Default::default()
                }))
            }
            "aggregate" => {
                if params.params.is_empty() {
                    return Err(InterpreterError {
                        message: "Aggregate requires at least one parameter".to_string(),
                    });
                }
                let arr = try_from!(<ArrayExpression>(params.params[0].clone()))?.elements;

                if arr.is_empty() {
                    return Err(InterpreterError {
                        message: "Aggregate requires at least one pipeline".to_string(),
                    });
                }

                let pipelines = arr
                    .into_iter()
                    .map(|p| {
                        let object = try_from!(<ObjectExpression>(p))?;
                        if let Bson::Document(doc) = to_interpter_error!(to_bson(&object))? {
                            Ok(doc)
                        } else {
                            Err(InterpreterError {
                                message: "Bson could not be converted to document".to_string(),
                            })
                        }
                    })
                    .collect::<Result<Vec<Document>, InterpreterError>>()?;

                Ok(Command::Aggregate(AggregateQuery {
                    pipelines,
                    options: AggregateOptions::default(),
                }))
            }
            _ => Err(InterpreterError {
                message: (format!("Command {} not implemented", command)),
            }),
        }
    }
}

#[derive(Debug)]
pub enum MainCommand {
    Find(Document, FindOptions),
    Aggregate(Vec<Document>, AggregateOptions),
    Count(Document, CountOptions),
}

#[derive(Default)]
pub struct FindQuery {
    options: FindOptions,
    count: bool,
    filter: Option<Document>,
}

#[derive(Default)]
pub struct AggregateQuery {
    pipelines: Vec<Document>,
    options: AggregateOptions,
}

#[derive(Default)]
pub struct CountQuery {
    filter: Option<Document>,
    options: AggregateOptions,
}

// TODO: Distinct
pub enum Command {
    Find(FindQuery),
    Count(CountQuery),
    Aggregate(AggregateQuery),
}

// TODO: Update queries

#[async_trait]
impl QueryBuilder for Command {
    fn add_sub_query(&mut self, query: SubCommand) -> Result<(), InterpreterError> {
        match self {
            Command::Find(find) => find.add_sub_query(query),
            Command::Count(count) => count.add_sub_query(query),
            Command::Aggregate(aggregate) => aggregate.add_sub_query(query),
        }
    }

    async fn build(
        self,
        collection: Collection<Document>,
        pagination: PaginationInfo,
    ) -> Result<Cursor<Document>, mongodb::error::Error> {
        match self {
            Command::Find(find) => find.build(collection, pagination).await,
            Command::Count(count) => count.build(collection, pagination).await,
            Command::Aggregate(aggregate) => aggregate.build(collection, pagination).await,
        }
    }
}

#[async_trait]
impl QueryBuilder for FindQuery {
    fn add_sub_query(&mut self, query: SubCommand) -> Result<(), InterpreterError> {
        self.options.limit = Some(100);
        self.options.batch_size = Some(50);
        match query {
            SubCommand::Count => {
                self.count = true;
            }
            SubCommand::Sort(doc) => {
                self.options.sort = doc;
            }
            SubCommand::AllowDiskUse => {
                self.options.allow_disk_use = Some(true);
            }
        }

        Ok(())
    }

    async fn build(
        mut self,
        collection: Collection<Document>,
        pagination: PaginationInfo,
    ) -> Result<Cursor<Document>, mongodb::error::Error> {
        if self.count {
            let mut pipelines = vec![doc! {"$count": "count"}];
            if self.filter.is_some() {
                pipelines.push(self.filter.unwrap());
            };

            let mut aggregate_options = AggregateOptions::default();
            aggregate_options.allow_disk_use = self.options.allow_disk_use;

            collection.aggregate(pipelines, aggregate_options).await
        } else {
            self.options.skip = Some(pagination.start);
            self.options.limit = Some(pagination.limit as i64);
            collection.find(self.filter, self.options).await
        }
    }
}

#[async_trait]
impl QueryBuilder for CountQuery {
    fn add_sub_query(&mut self, query: SubCommand) -> Result<(), InterpreterError> {
        match query {
            SubCommand::AllowDiskUse => todo!(),
            _ => Err(InterpreterError {
                message: "Count only supports AllowDiskUse".to_string(),
            }),
        }
    }

    async fn build(
        self,
        collection: Collection<Document>,
        _: PaginationInfo,
    ) -> Result<Cursor<Document>, mongodb::error::Error> {
        let mut pipelines = vec![doc! {"$count": "count"}];
        if self.filter.is_some() {
            pipelines.push(self.filter.unwrap());
        };

        let mut aggregate_options = AggregateOptions::default();
        aggregate_options.allow_disk_use = self.options.allow_disk_use;

        collection.aggregate(pipelines, aggregate_options).await
    }
}

#[async_trait]
impl QueryBuilder for AggregateQuery {
    fn add_sub_query(&mut self, query: SubCommand) -> Result<(), InterpreterError> {
        match query {
            SubCommand::Count => todo!(),
            SubCommand::AllowDiskUse => todo!(),
            _ => Err(InterpreterError {
                message: format!("Aggregate does not support {:?}", query),
            }),
        }
    }

    async fn build(
        mut self,
        collection: Collection<Document>,
        pagination: PaginationInfo,
    ) -> Result<Cursor<Document>, mongodb::error::Error> {
        let mut aggregate_options = AggregateOptions::default();
        aggregate_options.allow_disk_use = self.options.allow_disk_use;
        self.pipelines.push(doc! {"$skip": pagination.start as u32});
        self.pipelines.push(doc! {"$limit": pagination.limit});

        collection
            .aggregate(self.pipelines, aggregate_options)
            .await
    }
}

#[async_trait]
pub trait QueryBuilder {
    fn add_sub_query(&mut self, query: SubCommand) -> Result<(), InterpreterError>;
    async fn build(
        self,
        collection: Collection<Document>,
        pagination: PaginationInfo,
    ) -> Result<Cursor<Document>, mongodb::error::Error>;
}

// TODO: Limit, Skip
#[derive(Debug)]
pub enum SubCommand {
    Count,
    Sort(Option<Document>),
    AllowDiskUse,
}

impl TryFrom<(String, ParametersExpression)> for SubCommand {
    type Error = InterpreterError;

    fn try_from(
        (command, params): (String, ParametersExpression),
    ) -> Result<Self, InterpreterError> {
        match command.to_lowercase().as_str() {
            "count" => {
                if params.params.is_empty() {
                    return Ok(SubCommand::Count);
                }
                Err(InterpreterError {
                    message: "Count command doesn't accept any parameter".to_string(),
                })
            }
            "sort" => {
                if params.params.len() > 1 {
                    return Err(InterpreterError {
                        message: "Sort command only accepts 1 parameter".to_string(),
                    });
                }
                let sort_params = params.get_nth_of_type::<ObjectExpression>(0)?;

                if let Bson::Document(doc) = to_interpter_error!(to_bson(&sort_params))? {
                    return Ok(SubCommand::Sort(Some(doc)));
                }
                Err(InterpreterError {
                    message: "Bson could not be converted to document".to_string(),
                })
            }
            "allowdiskuse" => {
                if !params.params.is_empty() {
                    return Err(InterpreterError {
                        message: "AllowDiskUse doesn't accept any parameter".to_string(),
                    });
                }

                Ok(SubCommand::AllowDiskUse)
            }
            _ => Err(InterpreterError {
                message: "Unknown subcommand".to_string(),
            }),
        }
    }
}

impl MongodbConnector {
    pub fn get_handle(&self) -> Database {
        self.client.database(&self.database)
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

    async fn get_data(&self, str: String, pagination: PaginationInfo) -> Result<DatabaseData> {
        match InterpreterMongo::new(self, pagination)
            .interpret(str.to_string())
            .await
        {
            Ok(result) => Ok(result),
            Err(err) => Err(anyhow!(err.message)),
        }
    }
}

impl<'a> From<DatabaseData> for TableData<'a> {
    fn from(value: DatabaseData) -> Self {
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
