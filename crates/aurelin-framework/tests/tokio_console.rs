#![cfg(all(feature = "tokio-console", tokio_unstable))]

use std::{collections::HashMap, error::Error, time::Duration};

use aurelin_framework::telemetry;
use console_api::instrument::{InstrumentRequest, instrument_client::InstrumentClient};
use tokio::{
    task,
    time::{sleep, timeout},
};

#[tokio::test]
async fn console_companion_receives_live_task_telemetry_from_the_framework_subscriber() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    telemetry::init(directory.path(), &HashMap::from([("TOKIO_CONSOLE".into(), "true".into())]))?;
    let mut client = timeout(Duration::from_secs(10), async {
        loop {
            match InstrumentClient::connect("http://127.0.0.1:6669").await {
                Ok(client) => return client,
                Err(_) => sleep(Duration::from_millis(20)).await,
            }
        }
    })
    .await?;
    let mut updates = client.watch_updates(InstrumentRequest {}).await?.into_inner();
    let task = task::spawn(async { sleep(Duration::from_millis(50)).await });
    task.await?;
    timeout(Duration::from_secs(10), async {
        while let Some(update) = updates.message().await? {
            if update.task_update.is_some_and(|tasks| {
                tasks.new_tasks.iter().any(|task| {
                    task.location
                        .as_ref()
                        .and_then(|location| location.file.as_deref())
                        .is_some_and(|file| file.ends_with("tests/tokio_console.rs"))
                })
            }) {
                return Ok::<_, Box<dyn Error>>(());
            }
        }
        Err("console stream ended without task telemetry".into())
    })
    .await??;
    Ok(())
}
