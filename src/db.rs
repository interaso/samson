use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::utils::parse_rfc3339_timestamp;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsMessage {
    pub id: Option<i64>,
    #[serde(skip_serializing)]
    pub imei: String,
    pub sender: String,
    pub text: String,
    pub timestamp: DateTime<Utc>,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                imei TEXT NOT NULL,
                sender TEXT NOT NULL,
                text TEXT NOT NULL,
                timestamp TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute("CREATE INDEX IF NOT EXISTS idx_imei ON messages(imei)", [])?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_timestamp ON messages(timestamp)",
            [],
        )?;

        Ok(Self { conn })
    }

    pub fn insert_message(&self, msg: &SmsMessage) -> Result<()> {
        self.conn.execute(
            "INSERT INTO messages (imei, sender, text, timestamp) VALUES (?1, ?2, ?3, ?4)",
            params![msg.imei, msg.sender, msg.text, msg.timestamp.to_rfc3339(),],
        )?;
        Ok(())
    }

    pub fn get_messages(
        &self,
        imei: Option<&str>,
        after: Option<DateTime<Utc>>,
    ) -> Result<Vec<SmsMessage>> {
        let (query, params) = self.build_query(imei, after);

        let mut stmt = self
            .conn
            .prepare(&query)
            .context("Failed to prepare query")?;

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let messages = stmt
            .query_map(param_refs.as_slice(), |row| {
                let timestamp_str: String = row.get(4)?;
                let timestamp = parse_rfc3339_timestamp(&timestamp_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Text,
                        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                    )
                })?;

                Ok(SmsMessage {
                    id: Some(row.get(0)?),
                    imei: row.get(1)?,
                    sender: row.get(2)?,
                    text: row.get(3)?,
                    timestamp,
                })
            })
            .context("Failed to query messages")?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to collect message results")?;

        Ok(messages)
    }

    fn build_query(
        &self,
        imei: Option<&str>,
        after: Option<DateTime<Utc>>,
    ) -> (String, Vec<Box<dyn rusqlite::ToSql>>) {
        let mut query =
            String::from("SELECT id, imei, sender, text, timestamp FROM messages WHERE 1=1");
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        let mut param_num = 1;

        if let Some(imei) = imei {
            query.push_str(&format!(" AND imei = ?{}", param_num));
            params.push(Box::new(imei.to_string()));
            param_num += 1;
        }

        if let Some(after) = after {
            query.push_str(&format!(" AND timestamp > ?{}", param_num));
            params.push(Box::new(after.to_rfc3339()));
        }

        query.push_str(" ORDER BY timestamp ASC");

        (query, params)
    }

    pub fn message_exists(&self, msg: &SmsMessage) -> Result<bool> {
        let mut stmt = self.conn.prepare(
            "SELECT COUNT(*) FROM messages WHERE imei = ?1 AND sender = ?2 AND text = ?3 AND timestamp = ?4"
        )?;

        let count: i64 = stmt.query_row(
            params![msg.imei, msg.sender, msg.text, msg.timestamp.to_rfc3339(),],
            |row| row.get(0),
        )?;

        Ok(count > 0)
    }
}
