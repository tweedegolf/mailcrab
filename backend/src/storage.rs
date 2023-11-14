use std::{convert::Infallible, ops::Sub, sync::Arc, time::SystemTime};
use tokio::{sync::broadcast::Receiver, time::Duration};
use tokio_graceful_shutdown::SubsystemHandle;

use crate::{types::MailMessage, AppState};

/// storage task, stores all messages from the queue and optionally
/// deletes old messages
pub(crate) async fn storage(
    mut storage_rx: Receiver<MailMessage>,
    state: Arc<AppState>,
    handle: SubsystemHandle,
) -> Result<(), Infallible> {
    let mut running = true;
    // every retention_period / 10 seconds the messages will be filtered, keeping only messages
    // that are older than retention_period
    let min_retention_interval = Duration::from_secs(60);
    let mut retention_interval =
        tokio::time::interval(if state.retention_period / 10 < min_retention_interval {
            min_retention_interval
        } else {
            state.retention_period / 10
        });

    while running {
        tokio::select! {
            incoming = storage_rx.recv() => {
                if let Ok(message) = incoming {
                    if let Ok(mut storage) = state.storage.write() {
                        storage.insert(message.id, message);
                    }
                }
            },
            _ = retention_interval.tick() => {
                if state.retention_period > Duration::from_secs(0) {
                    if let Ok(mut storage) = state.storage.write() {
                        let remove_before = std::time::SystemTime::now()
                            .sub(state.retention_period)
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as i64;

                        storage.retain(|_, mail_message| mail_message.time > remove_before);
                    }
                }
            },
            _ = handle.on_shutdown_requested() => {
                running = false;
            },
        }
    }

    Ok(())
}
