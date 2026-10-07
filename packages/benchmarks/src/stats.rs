//! Latency bookkeeping. Each worker owns a [`Recorder`]; they are merged once
//! at the end, so no sample is ever dropped to a contended lock (the bug in
//! Rustrak's runner, which `try_lock`ed one shared histogram).

use hdrhistogram::Histogram;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

/// How one request went.
#[derive(Debug, Clone, Copy)]
pub struct Outcome {
    /// 0 when no response arrived at all.
    pub status: u16,
    pub latency: Duration,
    /// Time to the first body bytes of a stream.
    pub first_byte: Option<Duration>,
}

pub struct Recorder {
    latency: Histogram<u64>,
    first_byte: Histogram<u64>,
    statuses: BTreeMap<u16, u64>,
}

impl Default for Recorder {
    fn default() -> Self {
        // Microseconds, 1 µs to 5 min, 3 significant figures.
        let histogram = || Histogram::new_with_bounds(1, 300_000_000, 3).expect("valid bounds");
        Self {
            latency: histogram(),
            first_byte: histogram(),
            statuses: BTreeMap::new(),
        }
    }
}

fn micros(d: Duration) -> u64 {
    u64::try_from(d.as_micros()).unwrap_or(u64::MAX).max(1)
}

impl Recorder {
    pub fn record(&mut self, outcome: Outcome) {
        self.latency.saturating_record(micros(outcome.latency));
        if let Some(first) = outcome.first_byte {
            self.first_byte.saturating_record(micros(first));
        }
        *self.statuses.entry(outcome.status).or_default() += 1;
    }

    pub fn merge(&mut self, other: &Recorder) {
        self.latency.add(&other.latency).expect("same bounds");
        self.first_byte.add(&other.first_byte).expect("same bounds");
        for (status, count) in &other.statuses {
            *self.statuses.entry(*status).or_default() += count;
        }
    }

    pub fn summary(&self, elapsed: Duration) -> Summary {
        let requests: u64 = self.statuses.values().sum();
        let ok: u64 = self
            .statuses
            .iter()
            .filter(|(s, _)| (200..300).contains(*s))
            .map(|(_, n)| n)
            .sum();
        Summary {
            requests,
            ok,
            statuses: self.statuses.clone(),
            elapsed_secs: elapsed.as_secs_f64(),
            throughput_rps: requests as f64 / elapsed.as_secs_f64().max(f64::EPSILON),
            latency_ms: Percentiles::of(&self.latency),
            first_byte_ms: (!self.first_byte.is_empty()).then(|| Percentiles::of(&self.first_byte)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Percentiles {
    pub p50: f64,
    pub p90: f64,
    pub p99: f64,
    pub max: f64,
    pub mean: f64,
}

impl Percentiles {
    fn of(h: &Histogram<u64>) -> Self {
        let ms = |us: u64| us as f64 / 1000.0;
        Self {
            p50: ms(h.value_at_quantile(0.50)),
            p90: ms(h.value_at_quantile(0.90)),
            p99: ms(h.value_at_quantile(0.99)),
            max: ms(h.max()),
            mean: h.mean() / 1000.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub requests: u64,
    pub ok: u64,
    /// Status code → count; 0 is "no response".
    pub statuses: BTreeMap<u16, u64>,
    pub elapsed_secs: f64,
    pub throughput_rps: f64,
    pub latency_ms: Percentiles,
    pub first_byte_ms: Option<Percentiles>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(ms: u64, status: u16) -> Outcome {
        Outcome {
            status,
            latency: Duration::from_millis(ms),
            first_byte: None,
        }
    }

    #[test]
    fn merging_keeps_every_sample() {
        let mut a = Recorder::default();
        let mut b = Recorder::default();
        for ms in 1..=50 {
            a.record(outcome(ms, 200));
        }
        for ms in 51..=100 {
            b.record(outcome(ms, if ms == 100 { 502 } else { 200 }));
        }
        a.merge(&b);
        let summary = a.summary(Duration::from_secs(10));
        assert_eq!(summary.requests, 100);
        assert_eq!(summary.ok, 99);
        assert_eq!(summary.statuses[&502], 1);
        assert!(
            (summary.latency_ms.p50 - 50.0).abs() < 0.5,
            "{}",
            summary.latency_ms.p50
        );
        assert!((summary.latency_ms.p99 - 99.0).abs() < 0.5);
        assert_eq!(summary.throughput_rps, 10.0);
        assert!(summary.first_byte_ms.is_none());
    }

    #[test]
    fn first_byte_is_reported_for_streams() {
        let mut r = Recorder::default();
        r.record(Outcome {
            status: 200,
            latency: Duration::from_millis(40),
            first_byte: Some(Duration::from_millis(5)),
        });
        let first = r.summary(Duration::from_secs(1)).first_byte_ms.unwrap();
        assert!((first.p50 - 5.0).abs() < 0.1);
    }
}
