use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    time::SystemTime,
    vec::IntoIter,
};

use anyhow::Result;
use async_trait::async_trait;
use mongodb::{
    bson::oid::ObjectId,
    results::{CollectionSpecification, CollectionType},
    IndexModel,
};
use rusty_db_cli_derive_internals::TryFrom;
use rusty_db_cli_mongo::types::literals::Number;

use crate::widgets::scrollable_table::Row;

#[derive(Debug, Clone)]
pub struct ConnectorInfo {
    pub uri: String,
    pub host: String,
    pub database: String,
}

pub struct TableData<'a> {
    pub header: Row<'a>,
    pub rows: Vec<Row<'a>>,
}

impl<'a> Default for TableData<'a> {
    fn default() -> Self {
        Self {
            header: Row::default(),
            rows: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseData(pub Vec<Object>);

#[derive(Debug, Clone)]
pub struct DatabaseFetchResult {
    pub fetch_start: SystemTime,
    pub data: DatabaseData,
    pub trigger_query_took_message: bool,
}

impl IntoIterator for DatabaseData {
    type Item = Object;

    type IntoIter = IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl From<DatabaseData> for serde_json::Value {
    fn from(val: DatabaseData) -> Self {
        serde_json::Value::Array(val.into_iter().map(Into::into).collect())
    }
}

#[derive(Debug, Clone, TryFrom)]
pub enum DatabaseValue {
    String(String),
    DateTime(chrono::DateTime<chrono::Utc>),
    Number(Number),
    ObjectId(ObjectId),
    Array(Vec<DatabaseValue>),
    Object(Object),
    Bool(bool),
    CollectionInfo(CollectionSpecification),
    Index(IndexModel),
    Null,
}

impl Into<Object> for CollectionSpecification {
    fn into(self) -> Object {
        fn get_str(value: &Object, str: &str) -> Result<String, ()> {
            match value.get(str).unwrap() {
                DatabaseValue::String(str) => Ok(str.to_owned()),
                _ => return Err(()),
            }
        }

        let collection_type_str = match self.collection_type {
            CollectionType::View => "View",
            CollectionType::Collection => "Collection",
            CollectionType::Timeseries => "Timeseries",
            _ => "Missing",
        };

        Object(HashMap::from_iter([
            (String::from("name"), DatabaseValue::String(self.name)),
            (
                String::from("collection_type"),
                DatabaseValue::String(collection_type_str.to_owned()),
            ),
        ]))
    }
}

#[derive(Debug, Clone)]
pub struct Object(pub HashMap<String, DatabaseValue>);

impl From<Object> for serde_json::Value {
    fn from(val: Object) -> Self {
        serde_json::Value::Object(val.0.into_iter().fold(
            serde_json::Map::new(),
            |mut acc, (key, value)| {
                acc.insert(key, value.into());
                acc
            },
        ))
    }
}

impl IntoIterator for Object {
    type Item = (String, DatabaseValue);
    type IntoIter = std::collections::hash_map::IntoIter<String, DatabaseValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl Deref for Object {
    type Target = HashMap<String, DatabaseValue>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Object {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Object {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
}

impl Deref for DatabaseData {
    type Target = Vec<Object>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for DatabaseData {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Clone, Copy)]
pub struct PaginationInfo {
    pub start: u64,
    pub limit: u32,
}

impl PaginationInfo {
    pub fn reset(&mut self) {
        self.limit = LIMIT;
        self.start = 0;
    }
}

pub const LIMIT: u32 = 100;

#[async_trait]
pub trait Connector: Send + Sync {
    fn get_info(&self) -> &ConnectorInfo;
    async fn get_data(&self, query: String, pagination: PaginationInfo) -> Result<DatabaseData>;
    async fn set_database(&mut self, database: &str) -> Result<()>;
    async fn set_connection(&mut self, uri: String) -> anyhow::Result<ConnectorInfo>;
}

impl From<DatabaseValue> for serde_json::Value {
    fn from(val: DatabaseValue) -> Self {
        match val {
            DatabaseValue::String(str) => serde_json::Value::String(str),
            DatabaseValue::DateTime(date_time) => serde_json::Value::String(date_time.to_rfc3339()),
            DatabaseValue::Number(number) => serde_json::Value::Number(number.into()),
            DatabaseValue::ObjectId(object_id) => serde_json::Value::String(object_id.to_string()),
            DatabaseValue::Array(arr) => {
                serde_json::Value::Array(arr.into_iter().map(Into::into).collect())
            }
            DatabaseValue::Object(obj) => obj.into(),
            DatabaseValue::Bool(bool) => serde_json::Value::Bool(bool),
            DatabaseValue::Null => serde_json::Value::Null,
            DatabaseValue::CollectionInfo(_) => {
                todo!("Should not be ever needed")
            }
            DatabaseValue::Index(index) => {
                todo!();
            }
        }
    }
}
