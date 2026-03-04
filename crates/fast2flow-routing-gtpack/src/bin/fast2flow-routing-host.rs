use std::io::{self, Read};

use anyhow::Context;
use fast2flow_contracts::{Fast2FlowHookInV1, Fast2FlowHookOutV1};
use fast2flow_routing_gtpack::{HostRuntime, ENV_TRACE_POLICY};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut payload = String::new();
    io::stdin()
        .read_to_string(&mut payload)
        .context("failed reading hook input JSON from stdin")?;

    let request: Fast2FlowHookInV1 =
        serde_json::from_str(&payload).context("failed parsing Fast2FlowHookInV1 JSON payload")?;

    let runtime = HostRuntime::boot_from_env()
        .await
        .context("failed to bootstrap host runtime from environment")?;
    let trace_enabled = std::env::var(ENV_TRACE_POLICY)
        .ok()
        .map(|value| {
            let lowered = value.to_ascii_lowercase();
            lowered == "1" || lowered == "true" || lowered == "yes"
        })
        .unwrap_or(false);

    let output: Fast2FlowHookOutV1 = if trace_enabled {
        let (out, trace) = runtime.route_from_mounts_with_trace(request).await;
        eprintln!("{}", serde_json::to_string(&trace)?);
        out
    } else {
        runtime.route_from_mounts(request).await
    };

    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}
