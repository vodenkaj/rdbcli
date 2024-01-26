use std::ops::{Deref, DerefMut};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::widgets::scrollable_table::Row;

pub struct ConnectorInfo {
    pub uri: String,
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
pub struct DatabaseData(pub Vec<Value>);

impl Deref for DatabaseData {
    type Target = Vec<Value>;

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
    pub limit: i64,
}

impl PaginationInfo {
    pub fn reset(&mut self) {
        self.limit = 0;
        self.start = 0;
    }
}

pub const LIMIT: i64 = 100;

#[async_trait]
pub trait Connector: Send + Sync {
    async fn get_info(&self) -> &ConnectorInfo;
    async fn get_data(&self, query: String, pagination: PaginationInfo) -> Result<DatabaseData>;
    fn set_database(&mut self, database: &str);
}
