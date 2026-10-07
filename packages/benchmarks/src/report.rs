//! A run's result on disk, and the comparison of several as Markdown.

use serde::{Deserialize, Serialize};
use std::fmt::Write;

use crate::load::Scenario;
use crate::resources::ResourceSummary;
use crate::stats::Summary;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunResult {
    /// What was measured: `direct`, `llmtrack`, `litellm`, …
    pub label: String,
    pub scenario: Scenario,
    pub concurrency: usize,
    pub base_url: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub summary: Summary,
    pub resources: Option<ResourceSummary>,
}

fn ms(value: f64) -> String {
    format!("{value:.2}")
}

/// One table per scenario: every label's numbers, and how much latency each
/// adds over `baseline` (the provider called directly).
pub fn markdown(results: &[RunResult], baseline: &str) -> String {
    let mut out = String::from("# llmtrack benchmark\n\n");
    let mut scenarios: Vec<(Scenario, usize)> = results
        .iter()
        .map(|r| (r.scenario, r.concurrency))
        .collect();
    scenarios.sort_by_key(|(s, c)| (format!("{s:?}"), *c));
    scenarios.dedup();
    for (scenario, concurrency) in scenarios {
        let runs: Vec<&RunResult> = results
            .iter()
            .filter(|r| r.scenario == scenario && r.concurrency == concurrency)
            .collect();
        let base = runs.iter().find(|r| r.label == baseline);
        let _ = writeln!(
            out,
            "## {scenario:?}, {concurrency} concurrent\n\n\
             | target | req/s | p50 ms | p99 ms | added p50 | added p99 | first byte p50 | errors | CPU avg % | mem max MB |\n\
             |---|---:|---:|---:|---:|---:|---:|---:|---:|---:|"
        );
        for run in &runs {
            let s = &run.summary;
            let added = |pick: fn(&Summary) -> f64| match base {
                Some(b) if b.label != run.label => format!("{:+.2}", pick(s) - pick(&b.summary)),
                _ => "—".to_string(),
            };
            let resources = run.resources.as_ref();
            let _ = writeln!(
                out,
                "| {} | {:.0} | {} | {} | {} | {} | {} | {} | {} | {} |",
                run.label,
                s.throughput_rps,
                ms(s.latency_ms.p50),
                ms(s.latency_ms.p99),
                added(|s| s.latency_ms.p50),
                added(|s| s.latency_ms.p99),
                s.first_byte_ms.as_ref().map_or("—".into(), |f| ms(f.p50)),
                s.requests - s.ok,
                resources.map_or("—".into(), |r| format!("{:.1}", r.cpu_avg)),
                resources.map_or("—".into(), |r| format!("{:.1}", r.memory_max_mb)),
            );
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::Percentiles;
    use std::collections::BTreeMap;

    fn run(label: &str, p50: f64, p99: f64) -> RunResult {
        let percentiles = Percentiles {
            p50,
            p90: p50,
            p99,
            max: p99,
            mean: p50,
        };
        RunResult {
            label: label.into(),
            scenario: Scenario::Chat,
            concurrency: 16,
            base_url: String::new(),
            started_at: chrono::Utc::now(),
            summary: Summary {
                requests: 100,
                ok: 99,
                statuses: BTreeMap::new(),
                elapsed_secs: 1.0,
                throughput_rps: 100.0,
                latency_ms: percentiles,
                first_byte_ms: None,
            },
            resources: Some(ResourceSummary {
                samples: 1,
                cpu_avg: 12.0,
                cpu_max: 20.0,
                memory_avg_mb: 10.0,
                memory_max_mb: 15.5,
            }),
        }
    }

    #[test]
    fn the_table_shows_what_each_target_adds_over_the_baseline() {
        let md = markdown(
            &[
                run("direct", 1.0, 2.0),
                run("llmtrack", 1.5, 3.0),
                run("litellm", 9.0, 40.0),
            ],
            "direct",
        );
        assert!(md.contains("## Chat, 16 concurrent"));
        assert!(
            md.contains("| llmtrack | 100 | 1.50 | 3.00 | +0.50 | +1.00 |"),
            "{md}"
        );
        assert!(md.contains("| litellm | 100 | 9.00 | 40.00 | +8.00 | +38.00 |"));
        assert!(md.contains("| direct | 100 | 1.00 | 2.00 | — | — |"));
        assert!(md.contains("| 12.0 | 15.5 |"));
    }
}
