use std::str::FromStr;

use anyhow::Result;
use async_trait::async_trait;
use tokio_postgres::{Client, Config, NoTls, SimpleQueryMessage};

use crate::connectors::base::{Connector, ConnectorInfo, DatabaseData, Object, PaginationInfo};

pub struct PostgresqlConnectorBuilder {
    info: Option<ConnectorInfo>,
}

pub struct PostgresqlConnector {
    info: ConnectorInfo,
    pub client: Client,
    pub database: String,
}

impl PostgresqlConnectorBuilder {
    pub fn new(uri: &str) -> Self {
        Self {
            info: Some(ConnectorInfo {
                uri: uri.to_string(),
                host: "unknown".to_string(),
                database: "unknown".to_string(),
            }),
        }
    }

    pub async fn build(self) -> Result<PostgresqlConnector> {
        let mut info = self.info.unwrap();

        let config = Config::from_str(&info.uri)?;

        let (client, connection) = config.connect(NoTls).await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        info.host = "unknown".to_string();
        info.database = config.get_dbname().unwrap().to_string();

        Ok(PostgresqlConnector {
            info,
            client,
            database: config.get_dbname().unwrap().to_string(),
        })
    }
}

#[async_trait]
impl Connector for PostgresqlConnector {
    async fn set_database(&mut self, database: &str) -> Result<()> {
        self.database = String::from(database);

        Ok(())
    }

    fn get_info(&self) -> &crate::connectors::base::ConnectorInfo {
        &self.info
    }

    async fn get_data(&self, str: String, pagination: PaginationInfo) -> Result<DatabaseData> {
        let query = format!("{} LIMIT {};", str.replace(';', ""), pagination.limit);

        let result_typed = self.client.query(&query, &[]).await?;
        let result_raw: Vec<tokio_postgres::SimpleQueryRow> = self
            .client
            .simple_query(&query)
            .await?
            .into_iter()
            .filter_map(|msg| {
                if let SimpleQueryMessage::Row(row) = msg {
                    return Some(row);
                }

                None
            })
            .collect();

        let result: Vec<Object> = result_typed
            .into_iter()
            .zip(result_raw)
            .map(Object::from)
            .collect();

        Ok(DatabaseData(result))
    }

    async fn set_connection(&mut self, uri: String) -> Result<ConnectorInfo> {
        let config = Config::from_str(&uri)?;

        let (client, connection) = config.connect(NoTls).await?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        self.info.host = config.get_hostaddrs().first().unwrap().to_string();
        self.info.database = config.get_dbname().unwrap().to_string();
        self.client = client;

        Ok(self.info.clone())
    }
}
