use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    vec::IntoIter,
};

use anyhow::Result;
use async_trait::async_trait;
use mongodb::bson::oid::ObjectId;
use rusty_db_cli_derive_internals::TryFrom;

use crate::widgets::scrollable_table::Row;

pub struct ConnectorInfo {
    pub uri: String,
    pub host: String,
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
pub struct DatabaseData(pub Vec<DatabaseValue>);

impl IntoIterator for DatabaseData {
    type Item = DatabaseValue;

    type IntoIter = IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, TryFrom)]
pub enum DatabaseValue {
    String(String),
    DateTime(chrono::DateTime<chrono::Utc>),
    Number(i32),
    ObjectId(ObjectId),
    Array(Vec<DatabaseValue>),
    Object(Object),
    Bool(bool),
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object(HashMap<String, DatabaseValue>);

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
    type Target = Vec<DatabaseValue>;

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
    fn set_database(&mut self, database: &str);
}

impl Into<serde_json::Value> for DatabaseValue {
    fn into(self) -> serde_json::Value {
        match self {
            DatabaseValue::String(str) => serde_json::Value::String(str),
            DatabaseValue::DateTime(date_time) => serde_json::Value::String(date_time.to_rfc3339()),
            DatabaseValue::Number(number) => serde_json::Value::Number(number.into()),
            DatabaseValue::ObjectId(object_id) => serde_json::Value::String(object_id.to_string()),
            DatabaseValue::Array(arr) => {
                serde_json::Value::Array(arr.into_iter().map(Into::into).collect())
            }
            DatabaseValue::Object(obj) => serde_json::Value::Object(obj.into_iter().fold(
                serde_json::Map::new(),
                |mut acc, (key, value)| {
                    acc.insert(key, value.into());
                    acc
                },
            )),
            DatabaseValue::Bool(bool) => serde_json::Value::Bool(bool),
            DatabaseValue::Null => serde_json::Value::Null,
        }
    }
}
