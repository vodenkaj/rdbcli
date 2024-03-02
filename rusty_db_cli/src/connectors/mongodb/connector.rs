use std::{fs::File, io::Write, os::unix::fs::FileExt, time::Duration};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use chrono::TimeZone;
use mongodb::{
    bson::{doc, from_document, to_bson, Bson, Document},
    options::{AggregateOptions, ClientOptions, DistinctOptions, FindOptions},
    Client, Collection, Cursor, Database,
};
use rusty_db_cli_mongo::{
    interpreter::InterpreterError,
    lexer::Literal,
    parser::{ArrayExpression, ObjectExpression, ParametersExpression},
    to_interpter_error,
};

use super::interpreter::InterpreterMongo;
use crate::{
    connectors::base::{
        Connector, ConnectorInfo, DatabaseData, DatabaseValue, Object, PaginationInfo,
    },
    try_from,
    utils::external_editor::MONGO_COLLECTIONS_FILE,
};

pub struct MongodbConnectorBuilder {
    info: Option<ConnectorInfo>,
}

impl MongodbConnectorBuilder {
    pub fn new(uri: &str) -> Self {
        Self {
            info: Some(ConnectorInfo {
                uri: uri.to_string(),
                host: "unknown".to_string(),
                database: "unknown".to_string(),
            }),
        }
    }

    pub async fn build(self) -> Result<MongodbConnector> {
        let mut info = self.info.unwrap();
        let client_opts = ClientOptions::parse(info.uri.clone()).await?;
        let client = Client::with_options(client_opts.clone())?;

        if !client_opts.hosts.is_empty() {
            info.host = client_opts.hosts[0].to_string();
        }
        let database = client_opts.default_database.unwrap_or("admin".to_string());
        info.database = database.clone();

        let collections = client
            .database(&database)
            .list_collection_names(None)
            .await?
            .iter()
            .fold(String::new(), |acc, name| acc + name + "\n");

        let mut file = File::create(MONGO_COLLECTIONS_FILE.to_string()).unwrap();
        file.write_all_at(collections.as_bytes(), 0)?;
        file.flush()?;

        Ok(MongodbConnector {
            info,
            client,
            database,
        })
    }
}

pub struct MongodbConnector {
    info: ConnectorInfo,
    pub client: Client,
    pub database: String,
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
            "distinct" => {
                if params.params.len() > 3 {
                    return Err(InterpreterError {
                        message: "Distinct {} only accepts 3 parameters".to_string(),
                    });
                } else if params.params.is_empty() {
                    return Err(InterpreterError {
                        message: "Distinct {} requires at least one parameter".to_string(),
                    });
                }

                let field = String::try_from(params.get_nth_of_type::<Literal>(0)?).unwrap();
                let filter = params
                    .get_nth_of_type::<ObjectExpression>(1)
                    .ok()
                    .and_then(|obj| to_bson(&obj).ok())
                    .and_then(|bson| match bson {
                        Bson::Document(doc) => Some(doc),
                        _ => None,
                    });

                let opts_values = params
                    .get_nth_of_type::<ObjectExpression>(2)
                    .ok()
                    .and_then(|obj| to_bson(&obj).ok())
                    .and_then(|bson| match bson {
                        Bson::Document(doc) => Some(doc),
                        _ => None,
                    });
                let mut opts = DistinctOptions::default();
                if let Some(value) = opts_values {
                    if let Ok(max_time) = value.get_i64("maxTime") {
                        opts.max_time = Some(Duration::from_millis(max_time as u64));
                    }
                    if let Ok(collation) = value.get_document("collation") {
                        let result = to_interpter_error!(from_document(collation.clone()))?;
                        opts.collation = Some(result)
                    }
                    if let Ok(selection_criteria) = value.get_document("selectionCriteria") {
                        let result =
                            to_interpter_error!(from_document(selection_criteria.clone()))?;
                        opts.selection_criteria = Some(result)
                    }
                    if let Ok(read_concern) = value.get_document("readConcern") {
                        let result = to_interpter_error!(from_document(read_concern.clone()))?;
                        opts.read_concern = Some(result)
                    }
                }

                Ok(Command::Distinct(DistinctQuery {
                    field,
                    filter,
                    options: opts,
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

#[derive(Default)]
pub struct DistinctQuery {
    field: String,
    filter: Option<Document>,
    options: DistinctOptions,
}

pub enum Command {
    Find(FindQuery),
    Count(CountQuery),
    Aggregate(AggregateQuery),
    Distinct(DistinctQuery),
}

// TODO: Update queries

#[async_trait]
impl QueryBuilder for Command {
    fn add_sub_query(&mut self, query: SubCommand) -> Result<(), InterpreterError> {
        match self {
            Command::Find(find) => find.add_sub_query(query),
            Command::Count(count) => count.add_sub_query(query),
            Command::Aggregate(aggregate) => aggregate.add_sub_query(query),
            _ => self.add_sub_query(query),
        }
    }

    async fn build(
        self,
        collection: Collection<Document>,
        pagination: PaginationInfo,
    ) -> Result<DatabaseResponse, mongodb::error::Error> {
        match self {
            Command::Find(find) => find.build(collection, pagination).await,
            Command::Count(count) => count.build(collection, pagination).await,
            Command::Aggregate(aggregate) => aggregate.build(collection, pagination).await,
            Command::Distinct(distinct) => distinct.build(collection, pagination).await,
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
    ) -> Result<DatabaseResponse, mongodb::error::Error> {
        Ok(if self.count {
            let mut pipelines = Vec::new();
            if self.filter.is_some() {
                pipelines.push(doc! { "$match": self.filter.unwrap()});
            };
            pipelines.push(doc! {"$count": "count"});

            let mut aggregate_options = AggregateOptions::default();
            aggregate_options.allow_disk_use = self.options.allow_disk_use;

            DatabaseResponse::Cursor(collection.aggregate(pipelines, aggregate_options).await?)
        } else {
            self.options.skip = Some(pagination.start);
            self.options.limit = Some(pagination.limit as i64);
            DatabaseResponse::Cursor(collection.find(self.filter, self.options).await?)
        })
    }
}

#[async_trait]
impl QueryBuilder for DistinctQuery {
    async fn build(
        self,
        collection: Collection<Document>,
        _: PaginationInfo,
    ) -> Result<DatabaseResponse, mongodb::error::Error> {
        Ok(DatabaseResponse::Bson(
            collection
                .distinct(self.field, self.filter, self.options)
                .await?,
        ))
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
    ) -> Result<DatabaseResponse, mongodb::error::Error> {
        let mut pipelines = vec![doc! {"$count": "count"}];
        if self.filter.is_some() {
            pipelines.push(self.filter.unwrap());
        };

        let mut aggregate_options = AggregateOptions::default();
        aggregate_options.allow_disk_use = self.options.allow_disk_use;

        Ok(DatabaseResponse::Cursor(
            collection.aggregate(pipelines, aggregate_options).await?,
        ))
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
    ) -> Result<DatabaseResponse, mongodb::error::Error> {
        let mut aggregate_options = AggregateOptions::default();
        aggregate_options.allow_disk_use = self.options.allow_disk_use;
        self.pipelines.push(doc! {"$skip": pagination.start as u32});
        self.pipelines.push(doc! {"$limit": pagination.limit});

        Ok(DatabaseResponse::Cursor(
            collection
                .aggregate(self.pipelines, aggregate_options)
                .await?,
        ))
    }
}

pub enum DatabaseResponse {
    Cursor(Cursor<Document>),
    Bson(Vec<Bson>),
}

#[async_trait]
pub trait QueryBuilder {
    fn add_sub_query(&mut self, query: SubCommand) -> Result<(), InterpreterError> {
        Err(InterpreterError {
            message: format!("QueryBuilder does not support {:?}", query),
        })
    }
    async fn build(
        self,
        collection: Collection<Document>,
        pagination: PaginationInfo,
    ) -> Result<DatabaseResponse, mongodb::error::Error>;
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
    async fn set_database(&mut self, database: &str) -> Result<()> {
        self.database = String::from(database);

        let collections = self
            .client
            .database(database)
            .list_collection_names(None)
            .await
            .unwrap()
            .iter()
            .fold(String::new(), |acc, name| acc + name + "\n");

        let mut file = File::create(MONGO_COLLECTIONS_FILE.to_string())?;
        file.write_all_at(collections.as_bytes(), 0)?;
        file.flush()?;

        Ok(())
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

    async fn set_connection(&mut self, uri: String) -> Result<ConnectorInfo> {
        let mut client_opts = ClientOptions::parse(uri.clone()).await?;
        client_opts.server_selection_timeout = Some(Duration::from_secs(5));
        let client = Client::with_options(client_opts.clone())?;
        client
            .database("admin")
            .run_command(doc! {"ping": 1}, None)
            .await
            .with_context(|| "Failed to connect to the database")?;

        let info = ConnectorInfo {
            host: client_opts
                .hosts
                .first()
                .map(|host| host.to_string())
                .unwrap_or("unknown".to_string()),
            uri,
            database: client_opts.default_database.unwrap_or("admin".to_string()),
        };

        let collections = client
            .database(&info.database)
            .list_collection_names(None)
            .await?
            .iter()
            .fold(String::new(), |acc, name| acc + name + "\n");

        let mut file = File::create(MONGO_COLLECTIONS_FILE.to_string()).unwrap();
        file.write_all_at(collections.as_bytes(), 0)?;
        file.flush()?;

        //self.client.shutdown().await; -- may be needed?

        self.database = info.database.clone();
        self.info = info;
        self.client = client;

        Ok(self.info.clone())
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
