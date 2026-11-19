use std::time::Duration;

use aurelin_framework::timebox::Timebox;
use tokio::time::{Instant, advance};

#[tokio::test(start_paused = true)]
async fn wait_delays_until_the_configured_minimum_duration() {
    let timebox = Timebox::start(Duration::from_millis(200));
    advance(Duration::from_millis(50)).await;

    let waiting_started_at = Instant::now();
    timebox.wait().await;

    assert_eq!(waiting_started_at.elapsed(), Duration::from_millis(150));
}

#[tokio::test(start_paused = true)]
async fn wait_returns_immediately_after_the_minimum_duration_has_elapsed() {
    let timebox = Timebox::start(Duration::from_millis(200));
    advance(Duration::from_millis(250)).await;

    let waiting_started_at = Instant::now();
    timebox.wait().await;

    assert_eq!(waiting_started_at.elapsed(), Duration::ZERO);
}
