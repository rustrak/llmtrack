use clap::{Parser, Subcommand};
use llmtrack_bench::load::{self, LoadConfig, Scenario};
use llmtrack_bench::mock::{self, MockConfig};
use llmtrack_bench::report::{self, RunResult};
use llmtrack_bench::resources::{self, Target};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Parser)]
#[command(about = "Load tests for llmtrack and the gateways it is compared with")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Serve a mock OpenAI-compatible provider.
    Mock {
        #[arg(long, default_value = "0.0.0.0")]
        host: String,
        #[arg(long, default_value_t = 9000)]
        port: u16,
        /// Delay before each answer, to model a real provider.
        #[arg(long, default_value_t = 0)]
        latency_ms: u64,
        #[arg(long, default_value_t = 20)]
        chunks: usize,
        #[arg(long, default_value_t = 0)]
        chunk_delay_ms: u64,
    },
    /// Load a gateway (or the mock itself) and save the result.
    Run {
        /// Name in the report: direct, llmtrack, litellm, …
        #[arg(long)]
        label: String,
        /// Up to and including /v1.
        #[arg(long)]
        base_url: String,
        #[arg(long, env = "BENCH_API_KEY", default_value = "none")]
        api_key: String,
        #[arg(long, default_value = "bench-model")]
        model: String,
        #[arg(long, value_enum, default_value = "chat")]
        scenario: Scenario,
        #[arg(long, default_value_t = 16)]
        concurrency: usize,
        #[arg(long, default_value_t = 20)]
        duration_secs: u64,
        #[arg(long, default_value_t = 3)]
        warmup_secs: u64,
        #[arg(long, default_value_t = 2000)]
        prompt_chars: usize,
        /// Docker container to sample CPU and memory from.
        #[arg(long, conflicts_with = "pid")]
        container: Option<String>,
        /// Local process to sample CPU and memory from.
        #[arg(long)]
        pid: Option<u32>,
        #[arg(long, default_value = "results")]
        output: PathBuf,
    },
    /// Turn saved results into a Markdown comparison.
    Compare {
        /// Result files (JSON).
        files: Vec<PathBuf>,
        /// The label latencies are compared against.
        #[arg(long, default_value = "direct")]
        baseline: String,
        #[arg(long)]
        markdown: Option<PathBuf>,
    },
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Mock {
            host,
            port,
            latency_ms,
            chunks,
            chunk_delay_ms,
        } => {
            let config = MockConfig {
                latency: Duration::from_millis(latency_ms),
                chunks,
                chunk_delay: Duration::from_millis(chunk_delay_ms),
            };
            let (port, server) = mock::serve((host.as_str(), port), config)?;
            eprintln!("mock provider on :{port}");
            server.await?;
        }
        Command::Run {
            label,
            base_url,
            api_key,
            model,
            scenario,
            concurrency,
            duration_secs,
            warmup_secs,
            prompt_chars,
            container,
            pid,
            output,
        } => {
            let config = LoadConfig {
                base_url: base_url.clone(),
                api_key,
                model,
                scenario,
                concurrency,
                duration: Duration::from_secs(duration_secs),
                warmup: Duration::from_secs(warmup_secs),
                prompt_chars,
            };
            let target = container.map(Target::Container).or(pid.map(Target::Pid));
            let (stop, stopped) = tokio::sync::watch::channel(false);
            let sampler = target.map(|t| {
                tokio::spawn(resources::sample_until(
                    t,
                    Duration::from_millis(500),
                    stopped,
                ))
            });
            let started_at = chrono::Utc::now();
            let summary = load::run(&config).await?;
            let _ = stop.send(true);
            let resources = match sampler {
                Some(handle) => Some(handle.await?),
                None => None,
            };
            let result = RunResult {
                label: label.clone(),
                scenario,
                concurrency,
                base_url,
                started_at,
                summary,
                resources,
            };
            std::fs::create_dir_all(&output)?;
            let file =
                output.join(format!("{label}-{scenario:?}-c{concurrency}.json").to_lowercase());
            std::fs::write(&file, serde_json::to_string_pretty(&result)?)?;
            let s = &result.summary;
            println!(
                "{label}: {:.0} req/s, p50 {:.2} ms, p99 {:.2} ms, {} errors → {}",
                s.throughput_rps,
                s.latency_ms.p50,
                s.latency_ms.p99,
                s.requests - s.ok,
                file.display()
            );
        }
        Command::Compare {
            files,
            baseline,
            markdown,
        } => {
            let results = files
                .iter()
                .map(|f| Ok(serde_json::from_str(&std::fs::read_to_string(f)?)?))
                .collect::<anyhow::Result<Vec<RunResult>>>()?;
            let md = report::markdown(&results, &baseline);
            match markdown {
                Some(path) => std::fs::write(path, &md)?,
                None => print!("{md}"),
            }
        }
    }
    Ok(())
}
