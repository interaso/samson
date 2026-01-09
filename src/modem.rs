use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use tracing::warn;
use zbus::{proxy, Connection};

use crate::utils::parse_rfc3339_timestamp;

#[proxy(
    interface = "org.freedesktop.ModemManager1.Modem",
    default_service = "org.freedesktop.ModemManager1"
)]
trait Modem {
    #[zbus(property)]
    fn equipment_identifier(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "org.freedesktop.ModemManager1.Modem.Messaging",
    default_service = "org.freedesktop.ModemManager1"
)]
trait ModemMessaging {
    fn list(&self) -> zbus::Result<Vec<zbus::zvariant::OwnedObjectPath>>;
    fn delete(&self, path: &zbus::zvariant::ObjectPath<'_>) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.ModemManager1.Sms",
    default_service = "org.freedesktop.ModemManager1"
)]
trait Sms {
    #[zbus(property)]
    fn number(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn text(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn timestamp(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "org.freedesktop.DBus.ObjectManager",
    default_service = "org.freedesktop.ModemManager1",
    default_path = "/org/freedesktop/ModemManager1"
)]
trait ObjectManager {
    fn get_managed_objects(
        &self,
    ) -> zbus::Result<
        std::collections::HashMap<
            zbus::zvariant::OwnedObjectPath,
            std::collections::HashMap<
                String,
                std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
            >,
        >,
    >;
}

#[derive(serde::Serialize)]
pub struct ModemInfo {
    pub path: String,
    pub imei: String,
}

pub struct SmsInfo {
    pub sender: String,
    pub text: String,
    pub timestamp: DateTime<Utc>,
    pub sms_path: String,
}

pub struct ModemManager {
    conn: Connection,
}

impl ModemManager {
    pub async fn new() -> Result<Self> {
        let conn = Connection::system()
            .await
            .context("Failed to connect to system D-Bus")?;
        Ok(Self { conn })
    }

    async fn create_modem_proxy<'a>(
        &'a self,
        path: zbus::zvariant::OwnedObjectPath,
    ) -> Result<ModemProxy<'a>> {
        ModemProxy::builder(&self.conn)
            .path(path)?
            .build()
            .await
            .context("Failed to create modem proxy")
    }

    async fn create_messaging_proxy<'a>(
        &'a self,
        path: &'a str,
    ) -> Result<ModemMessagingProxy<'a>> {
        ModemMessagingProxy::builder(&self.conn)
            .path(path)?
            .build()
            .await
            .context("Failed to create messaging proxy")
    }

    async fn create_sms_proxy<'a>(
        &'a self,
        path: zbus::zvariant::OwnedObjectPath,
    ) -> Result<SmsProxy<'a>> {
        SmsProxy::builder(&self.conn)
            .path(path)?
            .build()
            .await
            .context("Failed to create SMS proxy")
    }

    pub async fn get_modems(&self) -> Result<Vec<ModemInfo>> {
        let proxy = ObjectManagerProxy::new(&self.conn)
            .await
            .context("Failed to create ObjectManager proxy")?;
        let objects = proxy
            .get_managed_objects()
            .await
            .context("Failed to get managed objects from ModemManager")?;

        let mut modems = Vec::new();

        for (path, interfaces) in objects {
            if interfaces.contains_key("org.freedesktop.ModemManager1.Modem") {
                let modem_proxy = self.create_modem_proxy(path.clone()).await?;

                let imei = modem_proxy
                    .equipment_identifier()
                    .await
                    .context("Failed to get modem IMEI")?;

                modems.push(ModemInfo {
                    path: path.to_string(),
                    imei,
                });
            }
        }

        modems.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(modems)
    }

    pub async fn get_messages(&self, modem_path: &str) -> Result<Vec<SmsInfo>> {
        let messaging_proxy = self.create_messaging_proxy(modem_path).await?;

        let sms_paths = messaging_proxy
            .list()
            .await
            .context("Failed to list SMS messages")?;
        let mut messages = Vec::new();

        for sms_path in sms_paths {
            let sms_proxy = self.create_sms_proxy(sms_path.clone()).await?;

            let sender = sms_proxy
                .number()
                .await
                .context("Failed to get SMS sender")?;
            let text = sms_proxy.text().await.context("Failed to get SMS text")?;
            let timestamp_str = sms_proxy
                .timestamp()
                .await
                .context("Failed to get SMS timestamp")?;

            // Parse timestamp with warning on failure
            let timestamp = match parse_rfc3339_timestamp(&timestamp_str) {
                Ok(dt) => dt,
                Err(e) => {
                    warn!(
                        "Failed to parse SMS timestamp '{}': {}. Using current time.",
                        timestamp_str, e
                    );
                    Utc::now()
                }
            };

            messages.push(SmsInfo {
                sender,
                text,
                timestamp,
                sms_path: sms_path.to_string(),
            });
        }

        Ok(messages)
    }

    pub async fn delete_message(&self, modem_path: &str, sms_path: &str) -> Result<()> {
        let messaging_proxy = self.create_messaging_proxy(modem_path).await?;

        let sms_obj_path = zbus::zvariant::ObjectPath::try_from(sms_path)
            .context(format!("Invalid SMS path: {}", sms_path))?;

        messaging_proxy
            .delete(&sms_obj_path)
            .await
            .context("Failed to delete SMS from modem")?;
        Ok(())
    }
}
