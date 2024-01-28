use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::TimeZone;
use mongodb::{
    bson::{doc, to_bson, Bson, Document},
    options::{AggregateOptions, ClientOptions, FindOptions},
    Client, Collection, Cursor, Database,
};
use rusty_db_cli_mongo::{
    interpreter::InterpreterError,
    parser::{ArrayExpression, ObjectExpression, ParametersExpression},
    to_interpter_error,
};

use super::interpreter::InterpreterMongo;
use crate::{
    connectors::base::{
        Connector, ConnectorInfo, DatabaseData, DatabaseValue, Object, PaginationInfo,
    },
    try_from,
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
        let client = Client::with_options(client_opts.clone())?;

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
            let mut pipelines = Vec::new();
            if self.filter.is_some() {
                pipelines.push(doc! { "$match": self.filter.unwrap()});
            };
            pipelines.push(doc! {"$count": "count"});

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
            SubCommand::AllowDiskUse => {
                self.options.allow_disk_use = Some(true);
                Ok(())
            }
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
            SubCommand::AllowDiskUse => {
                self.options.allow_disk_use = Some(true);
                Ok(())
            }
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

    fn get_info(&self) -> &crate::connectors::base::ConnectorInfo {
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

impl TryFrom<Document> for DatabaseValue {
    type Error = ();

    fn try_from(value: Document) -> Result<Self, Self::Error> {
        Ok(DatabaseValue::Object(value.into_iter().fold(
            Object::new(),
            |mut acc, (key, value)| {
                acc.insert(key, try_from!(<DatabaseValue>(value)).unwrap());

                acc
            },
        )))
    }
}

impl TryFrom<Bson> for DatabaseValue {
    type Error = ();

    fn try_from(value: Bson) -> Result<Self, Self::Error> {
        match value {
            Bson::String(str) => Ok(DatabaseValue::String(str)),
            Bson::Array(arr) => Ok(DatabaseValue::Array(
                arr.into_iter()
                    .map(|value| DatabaseValue::try_from(value).unwrap())
                    .collect(),
            )),
            Bson::Document(doc) => DatabaseValue::try_from(doc),
            Bson::Boolean(bool) => Ok(DatabaseValue::Bool(bool)),
            Bson::Null => Ok(DatabaseValue::Null),
            Bson::Double(num) => Ok(DatabaseValue::Number(num as i32)),
            Bson::Int32(num) => Ok(DatabaseValue::Number(num as i32)),
            Bson::Int64(num) => Ok(DatabaseValue::Number(num as i32)),
            Bson::Timestamp(timestamp) => Ok(DatabaseValue::DateTime(
                chrono::Utc.timestamp_opt(timestamp.time as i64, 0).unwrap(),
            )),
            Bson::DateTime(date_time) => Ok(DatabaseValue::DateTime(date_time.into())),
            Bson::ObjectId(object_id) => Ok(DatabaseValue::ObjectId(object_id)),
            _ => Ok(DatabaseValue::String(value.to_string())),
        }
    }
}
