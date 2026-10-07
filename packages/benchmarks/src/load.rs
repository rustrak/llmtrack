//! Closed-loop load: `concurrency` workers each send a request, wait for the
//! whole answer, and send the next, until the deadline. A worker never starts
//! a request past the deadline and never abandons one in flight, so every
//! sample is a complete request.

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::{Duration, Instant};

use crate::stats::{Outcome, Recorder, Summary};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Scenario {
    /// Non-streaming chat completions.
    Chat,
    /// Streamed chat completions; also reports time to first byte.
    Stream,
}

#[derive(Debug, Clone)]
pub struct LoadConfig {
    /// Up to and including `/v1`, e.g. `http://localhost:4000/v1`.
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub scenario: Scenario,
    pub concurrency: usize,
    pub duration: Duration,
    pub warmup: Duration,
    /// Size of the prompt, in characters.
    pub prompt_chars: usize,
}

fn body(config: &LoadConfig) -> bytes::Bytes {
    let prompt = "lorem ipsum ".repeat(config.prompt_chars / 12 + 1);
    let body = json!({
        "model": config.model,
        "messages": [{"role": "user", "content": &prompt[..config.prompt_chars.min(prompt.len())]}],
        "stream": config.scenario == Scenario::Stream,
    });
    bytes::Bytes::from(body.to_string())
}

async fn one(
    client: &reqwest::Client,
    url: &str,
    key: &str,
    body: bytes::Bytes,
    stream: bool,
) -> Outcome {
    let started = Instant::now();
    let response = client
        .post(url)
        .bearer_auth(key)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await;
    let Ok(response) = response else {
        return Outcome {
            status: 0,
            latency: started.elapsed(),
            first_byte: None,
        };
    };
    let status = response.status().as_u16();
    let mut first_byte = None;
    if stream {
        let mut chunks = response.bytes_stream();
        while let Some(chunk) = chunks.next().await {
            if chunk.is_err() {
                break;
            }
            first_byte.get_or_insert_with(|| started.elapsed());
        }
    } else {
        let _ = response.bytes().await;
    }
    Outcome {
        status,
        latency: started.elapsed(),
        first_byte,
    }
}

/// Runs the scenario and returns what it measured (warm-up excluded).
pub async fn run(config: &LoadConfig) -> anyhow::Result<Summary> {
    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(config.concurrency)
        .timeout(Duration::from_secs(120))
        .build()?;
    let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
    let payload = body(config);
    let stream = config.scenario == Scenario::Stream;

    // Warm-up: connections opened, caches filled, nothing recorded.
    let warmup_end = Instant::now() + config.warmup;
    while Instant::now() < warmup_end {
        one(&client, &url, &config.api_key, payload.clone(), stream).await;
    }

    let started = Instant::now();
    let deadline = started + config.duration;
    let workers: Vec<_> = (0..config.concurrency)
        .map(|_| {
            let (client, url, key, payload) = (
                client.clone(),
                url.clone(),
                config.api_key.clone(),
                payload.clone(),
            );
            tokio::spawn(async move {
                let mut recorder = Recorder::default();
                while Instant::now() < deadline {
                    recorder.record(one(&client, &url, &key, payload.clone(), stream).await);
                }
                recorder
            })
        })
        .collect();
    let mut total = Recorder::default();
    for worker in workers {
        total.merge(&worker.await?);
    }
    Ok(total.summary(started.elapsed()))
}
