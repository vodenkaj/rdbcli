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

#[derive(Debug)]
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

pub struct PaginationInfo {
    pub start: u64,
    pub limit: u64,
}

pub const LIMIT: u64 = 100;

#[async_trait]
pub trait Connector: Send {
    async fn get_info(&self) -> &ConnectorInfo;
    async fn get_data(&self, query: &str, pagination: &PaginationInfo) -> Result<DatabaseData>;
}
