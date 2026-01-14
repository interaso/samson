use crate::db::{Database, SmsMessage};
use crate::modem::ModemManager;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

pub struct SmsPoller {
    modem_manager: Arc<ModemManager>,
    db: Arc<Mutex<Database>>,
    poll_interval: Duration,
}

impl SmsPoller {
    pub fn new(
        modem_manager: Arc<ModemManager>,
        db: Arc<Mutex<Database>>,
        poll_interval_secs: u64,
    ) -> Self {
        Self {
            modem_manager,
            db,
            poll_interval: Duration::from_secs(poll_interval_secs),
        }
    }

    pub async fn start(self: Arc<Self>) {
        info!("Starting SMS polling service");

        loop {
            if let Err(e) = self.poll_modems().await {
                error!("Error polling modems: {}", e);
            }

            tokio::time::sleep(self.poll_interval).await;
        }
    }

    async fn poll_modems(&self) -> Result<()> {
        let modems = match self.modem_manager.get_modems().await {
            Ok(modems) => modems,
            Err(e) => {
                error!("Failed to get modems list: {}", e);
                return Ok(()); // Continue polling even if we can't get modems list
            }
        };

        debug!(modem_count = modems.len(), "Polling modems");

        for modem in modems {
            debug!(path = %modem.path, imei = %modem.imei, imsi = %modem.imsi, "Checking modem");

            match self.modem_manager.get_messages(&modem.path).await {
                Ok(messages) => {
                    if messages.is_empty() {
                        continue;
                    }

                    info!(
                        message_count = messages.len(),
                        imei = %modem.imei,
                        imsi = %modem.imsi,
                        "Found messages on modem"
                    );

                    for sms in messages {
                        if let Err(e) = self.process_message(&modem, sms).await {
                            error!(error = %e, "Failed to process message");
                        }
                    }
                }
                Err(e) => {
                    error!(imei = %modem.imei, imsi = %modem.imsi, error = %e, "Failed to get messages from modem");
                }
            }
        }

        Ok(())
    }

    async fn process_message(
        &self,
        modem: &crate::modem::ModemInfo,
        sms: crate::modem::SmsInfo,
    ) -> Result<()> {
        let msg = SmsMessage {
            id: None,
            imei: modem.imei.clone(),
            imsi: modem.imsi.clone(),
            sender: sms.sender.clone(),
            text: sms.text.clone(),
            timestamp: sms.timestamp,
        };

        // Check if message already exists (without holding lock during network operations)
        let message_exists = {
            let db = self.db.lock().await;
            db.message_exists(&msg)?
        };

        if message_exists {
            info!(
                "Message from {} already exists, deleting duplicate from modem",
                sms.sender
            );
            self.modem_manager
                .delete_message(&modem.path, &sms.sms_path)
                .await?;
            return Ok(());
        }

        // Save message to database
        {
            let db = self.db.lock().await;
            db.insert_message(&msg)?;
        }

        info!("Saved message from {} to database", msg.sender);

        // Only delete from modem after successful database insert
        if let Err(e) = self
            .modem_manager
            .delete_message(&modem.path, &sms.sms_path)
            .await
        {
            error!(
                "Failed to delete message from modem: {} - message will be reprocessed next poll",
                e
            );
            // Don't propagate this error - the message is saved, deletion can be retried
        }

        Ok(())
    }
}
