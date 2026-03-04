use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use fast2flow_contracts::{Fast2FlowHookInV1, FlowDoc, MessageEnvelope, RoutingPolicyV1};
use fast2flow_core::{CandidateIndex, CoreRouter, RouterConfig};
use fast2flow_hooks::DefaultHookFilter;
use fast2flow_indexer::{build_index, load_latest, IndexStore};
use fast2flow_routing_gtpack::load_policy_from_path;
use fast2flow_strategy_phase1::Phase1DeterministicStrategy;

#[derive(Parser)]
#[command(name = "fast2flow")]
#[command(about = "Fast2Flow developer CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Index {
        #[command(subcommand)]
        command: IndexCommands,
    },
    Route {
        #[command(subcommand)]
        command: RouteCommands,
    },
    Policy {
        #[command(subcommand)]
        command: PolicyCommands,
    },
}

#[derive(Subcommand)]
enum IndexCommands {
    Build {
        #[arg(long)]
        scope: String,
        #[arg(long)]
        flows: PathBuf,
        #[arg(long, default_value = "/mnt/indexes")]
        output: PathBuf,
        #[arg(long, default_value_t = 0)]
        now_unix_ms: u64,
    },
    Inspect {
        #[arg(long)]
        scope: String,
        #[arg(long, default_value = "/mnt/indexes")]
        input: PathBuf,
    },
}

#[derive(Subcommand)]
enum RouteCommands {
    Simulate {
        #[arg(long)]
        scope: String,
        #[arg(long)]
        text: String,
        #[arg(long, default_value = "/mnt/indexes")]
        indexes_path: PathBuf,
        #[arg(long, default_value_t = false)]
        session_active: bool,
        #[arg(long, default_value_t = 250)]
        time_budget_ms: u64,
    },
}

#[derive(Subcommand)]
enum PolicyCommands {
    Validate {
        #[arg(long)]
        file: PathBuf,
    },
    PrintDefault,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Index { command } => run_index(command),
        Commands::Route { command } => run_route(command).await,
        Commands::Policy { command } => run_policy(command),
    }
}

fn run_index(command: IndexCommands) -> Result<()> {
    match command {
        IndexCommands::Build {
            scope,
            flows,
            output,
            now_unix_ms,
        } => {
            let data = fs::read_to_string(&flows)
                .with_context(|| format!("failed reading {}", flows.display()))?;
            let docs: Vec<FlowDoc> = serde_json::from_str(&data)
                .with_context(|| format!("failed parsing {}", flows.display()))?;
            let manifest = build_index(&scope, &docs, &output, now_unix_ms)?;
            println!("{}", serde_json::to_string_pretty(&manifest)?);
            Ok(())
        }
        IndexCommands::Inspect { scope, input } => {
            let store = load_latest(&input, &scope)?;
            println!(
                "scope={} entries={}",
                store.manifest().scope,
                store.manifest().entries.len()
            );
            Ok(())
        }
    }
}

async fn run_route(command: RouteCommands) -> Result<()> {
    match command {
        RouteCommands::Simulate {
            scope,
            text,
            indexes_path,
            session_active,
            time_budget_ms,
        } => {
            let store = load_latest(&indexes_path, &scope)?;
            let lookup = CliIndexLookup { store };
            let strategy = Arc::new(Phase1DeterministicStrategy);
            let filter = Arc::new(DefaultHookFilter::default());
            let router = CoreRouter::new(strategy, vec![filter], None, RouterConfig::default());
            let request = Fast2FlowHookInV1 {
                scope,
                envelope: MessageEnvelope {
                    text,
                    channel: Some("cli".to_string()),
                    provider: Some("simulate".to_string()),
                },
                session_active,
                input_locale: "en-US".to_string(),
                time_budget_ms,
                registry_path: "/mnt/registry/latest.json".to_string(),
                indexes_path: indexes_path.display().to_string(),
                now_unix_ms: 0,
            };
            let output = router.route(request, &lookup).await;
            println!("{}", serde_json::to_string_pretty(&output)?);
            Ok(())
        }
    }
}

fn run_policy(command: PolicyCommands) -> Result<()> {
    match command {
        PolicyCommands::Validate { file } => {
            let policy = load_policy_from_path(&file)?;
            match policy {
                Some(policy) => {
                    let scope_count = policy.scope_overrides.len();
                    let channel_count = policy.channel_overrides.len();
                    let provider_count = policy.provider_overrides.len();
                    println!(
                        "valid policy: stage_order={} scope_overrides={} channel_overrides={} provider_overrides={}",
                        policy.stage_order.len(),
                        scope_count,
                        channel_count,
                        provider_count
                    );
                }
                None => {
                    anyhow::bail!("policy file does not exist: {}", file.display());
                }
            }
            Ok(())
        }
        PolicyCommands::PrintDefault => {
            let policy = RoutingPolicyV1::default();
            println!("{}", serde_json::to_string_pretty(&policy)?);
            Ok(())
        }
    }
}

struct CliIndexLookup {
    store: IndexStore,
}

impl CandidateIndex for CliIndexLookup {
    fn search(
        &self,
        _scope: &str,
        text: &str,
        limit: usize,
    ) -> Vec<fast2flow_contracts::Candidate> {
        self.store.search(text, limit)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use fast2flow_contracts::{PolicyRuleV1, RespondRuleV1, RoutingPolicyV1, TextMatchModeV1};

    use super::{run_policy, PolicyCommands};

    fn temp_file(name: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!("fast2flow-cli-{name}-{suffix}.json"))
    }

    #[test]
    fn validate_policy_command_accepts_valid_policy_file() {
        let path = temp_file("valid-policy");
        let payload =
            serde_json::to_string_pretty(&RoutingPolicyV1::default()).expect("serialize policy");
        fs::write(&path, payload).expect("write policy file");

        let result = run_policy(PolicyCommands::Validate { file: path.clone() });
        let _ = fs::remove_file(path);
        assert!(result.is_ok(), "expected valid policy to pass");
    }

    #[test]
    fn validate_policy_command_rejects_missing_file() {
        let path = temp_file("missing-policy");
        let result = run_policy(PolicyCommands::Validate { file: path });
        assert!(result.is_err(), "expected missing policy to fail");
    }

    #[test]
    fn validate_policy_command_rejects_invalid_regex_policy() {
        let path = temp_file("invalid-policy");
        let policy = RoutingPolicyV1 {
            default: PolicyRuleV1 {
                respond_rules: Some(vec![RespondRuleV1 {
                    needle: "(unclosed".to_string(),
                    message: "bad regex".to_string(),
                    mode: TextMatchModeV1::Regex,
                }]),
                ..PolicyRuleV1::default()
            },
            ..RoutingPolicyV1::default()
        };
        let payload = serde_json::to_string_pretty(&policy).expect("serialize invalid policy");
        fs::write(&path, payload).expect("write invalid policy file");

        let result = run_policy(PolicyCommands::Validate { file: path.clone() });
        let _ = fs::remove_file(path);
        assert!(result.is_err(), "expected invalid policy to fail");
    }
}
