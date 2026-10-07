//! CPU and memory of what is under test, sampled while the load runs.
//!
//! Shells out to `docker stats` for a container and to `ps` for a local
//! process: both exist wherever a benchmark is run, and a Docker API client
//! (Rustrak's bollard) is a heavy dependency for two numbers.

use serde::{Deserialize, Serialize};
use std::process::Command;
use std::time::Duration;
use tokio::sync::watch;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Target {
    Container(String),
    Pid(u32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    /// Percent of one core.
    pub cpu: f64,
    pub memory_mb: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ResourceSummary {
    pub samples: usize,
    pub cpu_avg: f64,
    pub cpu_max: f64,
    pub memory_avg_mb: f64,
    pub memory_max_mb: f64,
}

/// `12.5MiB`, `1.2GiB`, `512kB` → megabytes.
fn parse_size(text: &str) -> Option<f64> {
    let text = text.trim();
    let split = text.find(|c: char| c.is_ascii_alphabetic())?;
    let (number, unit) = text.split_at(split);
    let number: f64 = number.trim().parse().ok()?;
    let factor = match unit.to_ascii_lowercase().as_str() {
        "b" => 1.0 / 1_048_576.0,
        "kib" | "kb" => 1.0 / 1024.0,
        "mib" | "mb" => 1.0,
        "gib" | "gb" => 1024.0,
        _ => return None,
    };
    Some(number * factor)
}

/// One line of `docker stats --format '{{.CPUPerc}};{{.MemUsage}}'`.
pub fn parse_docker_line(line: &str) -> Option<Sample> {
    let (cpu, memory) = line.trim().split_once(';')?;
    let cpu = cpu.trim().trim_end_matches('%').parse().ok()?;
    let used = memory.split('/').next()?;
    Some(Sample {
        cpu,
        memory_mb: parse_size(used)?,
    })
}

/// One line of `ps -o %cpu=,rss=` (RSS in KiB).
pub fn parse_ps_line(line: &str) -> Option<Sample> {
    let mut fields = line.split_whitespace();
    let cpu = fields.next()?.parse().ok()?;
    let rss_kib: f64 = fields.next()?.parse().ok()?;
    Some(Sample {
        cpu,
        memory_mb: rss_kib / 1024.0,
    })
}

fn sample(target: &Target) -> Option<Sample> {
    let output = match target {
        Target::Container(name) => Command::new("docker")
            .args([
                "stats",
                "--no-stream",
                "--format",
                "{{.CPUPerc}};{{.MemUsage}}",
                name,
            ])
            .output(),
        Target::Pid(pid) => Command::new("ps")
            .args(["-o", "%cpu=,rss=", "-p", &pid.to_string()])
            .output(),
    }
    .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next()?;
    match target {
        Target::Container(_) => parse_docker_line(line),
        Target::Pid(_) => parse_ps_line(line),
    }
}

pub fn summarize(samples: &[Sample]) -> ResourceSummary {
    if samples.is_empty() {
        return ResourceSummary::default();
    }
    let n = samples.len() as f64;
    ResourceSummary {
        samples: samples.len(),
        cpu_avg: samples.iter().map(|s| s.cpu).sum::<f64>() / n,
        cpu_max: samples.iter().map(|s| s.cpu).fold(0.0, f64::max),
        memory_avg_mb: samples.iter().map(|s| s.memory_mb).sum::<f64>() / n,
        memory_max_mb: samples.iter().map(|s| s.memory_mb).fold(0.0, f64::max),
    }
}

/// Samples `target` every `every` until `stop` flips to true.
pub async fn sample_until(
    target: Target,
    every: Duration,
    mut stop: watch::Receiver<bool>,
) -> ResourceSummary {
    let mut samples = Vec::new();
    loop {
        let target = target.clone();
        if let Ok(Some(s)) = tokio::task::spawn_blocking(move || sample(&target)).await {
            samples.push(s);
        }
        tokio::select! {
            _ = tokio::time::sleep(every) => {}
            _ = stop.changed() => break,
        }
        if *stop.borrow() {
            break;
        }
    }
    summarize(&samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docker_lines_parse() {
        let s = parse_docker_line("12.34%;45.5MiB / 1.94GiB").unwrap();
        assert_eq!(s.cpu, 12.34);
        assert_eq!(s.memory_mb, 45.5);
        assert_eq!(
            parse_docker_line("0.50%;1.5GiB / 8GiB").unwrap().memory_mb,
            1536.0
        );
        assert!(parse_docker_line("--").is_none());
    }

    #[test]
    fn ps_lines_parse() {
        let s = parse_ps_line(" 23.0  20480").unwrap();
        assert_eq!(s.cpu, 23.0);
        assert_eq!(s.memory_mb, 20.0);
    }

    #[test]
    fn summaries_average_and_peak() {
        let summary = summarize(&[
            Sample {
                cpu: 10.0,
                memory_mb: 20.0,
            },
            Sample {
                cpu: 30.0,
                memory_mb: 40.0,
            },
        ]);
        assert_eq!(summary.cpu_avg, 20.0);
        assert_eq!(summary.cpu_max, 30.0);
        assert_eq!(summary.memory_max_mb, 40.0);
        assert_eq!(summarize(&[]), ResourceSummary::default());
    }
}
