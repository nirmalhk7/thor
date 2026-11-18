use super::{MetricsCollector, ProtocolRunner, ThorScenario};
use reqwest::Client;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;

pub(super) struct Schedule {
    pub(super) index: usize,
    pub(super) count: usize,
    pub(super) deadline: Instant,
    pub(super) interval: Option<Duration>,
}

pub(super) async fn run(
    client: Client,
    scenario: ThorScenario,
    collector: Arc<MetricsCollector>,
    schedule: Schedule,
) {
    if scenario.ramp_up_secs > 0 && schedule.count > 1 {
        let stagger_ms = (scenario.ramp_up_secs * 1000) / (schedule.count as u64);
        let delay = Duration::from_millis(stagger_ms * (schedule.index as u64));
        sleep(delay).await;
    }
    let mut next_wake = tokio::time::Instant::now();
    while Instant::now() < schedule.deadline {
        run_iteration(&client, &scenario, &collector, schedule.deadline).await;
        pace(&mut next_wake, schedule.interval).await;
    }
}

async fn run_iteration(
    client: &Client,
    scenario: &ThorScenario,
    collector: &MetricsCollector,
    deadline: Instant,
) {
    for step in &scenario.steps {
        if Instant::now() >= deadline {
            break;
        }
        ProtocolRunner::execute_step(client, step, scenario, collector).await;
    }
}

async fn pace(next_wake: &mut tokio::time::Instant, interval: Option<Duration>) {
    if let Some(inv) = interval {
        *next_wake += inv;
        let now = tokio::time::Instant::now();
        if *next_wake > now {
            tokio::time::sleep_until(*next_wake).await;
        } else {
            *next_wake = now;
        }
    }
}
