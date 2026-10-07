//! The generator against the mock, in process: every request counted, streams
//! timed to their first byte.

use llmtrack_bench::load::{run, LoadConfig, Scenario};
use llmtrack_bench::mock::{serve, MockConfig};
use std::time::Duration;

async fn mock(config: MockConfig) -> String {
    let (port, server) = serve(("127.0.0.1", 0), config).unwrap();
    actix_web::rt::spawn(server);
    format!("http://127.0.0.1:{port}/v1")
}

fn load(base_url: String, scenario: Scenario) -> LoadConfig {
    LoadConfig {
        base_url,
        api_key: "none".into(),
        model: "bench-model".into(),
        scenario,
        concurrency: 4,
        duration: Duration::from_millis(500),
        warmup: Duration::ZERO,
        prompt_chars: 200,
    }
}

#[actix_web::test]
async fn chat_requests_all_succeed_and_are_counted() {
    let base = mock(MockConfig::default()).await;
    let summary = run(&load(base, Scenario::Chat)).await.unwrap();
    assert!(summary.requests > 10, "{}", summary.requests);
    assert_eq!(summary.ok, summary.requests);
    assert!(summary.first_byte_ms.is_none());
}

#[actix_web::test]
async fn streams_report_time_to_first_byte_under_the_total() {
    let base = mock(MockConfig {
        chunks: 5,
        chunk_delay: Duration::from_millis(10),
        ..MockConfig::default()
    })
    .await;
    let summary = run(&load(base, Scenario::Stream)).await.unwrap();
    let first = summary
        .first_byte_ms
        .expect("streams time their first byte");
    assert!(first.p50 < summary.latency_ms.p50);
    assert!(summary.latency_ms.p50 >= 50.0, "5 chunks 10 ms apart");
}

#[actix_web::test]
async fn an_unreachable_target_is_counted_as_status_zero() {
    let summary = run(&load("http://127.0.0.1:9/v1".into(), Scenario::Chat))
        .await
        .unwrap();
    assert_eq!(summary.ok, 0);
    assert!(summary.statuses.contains_key(&0));
}
