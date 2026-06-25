#[cfg(not(target_arch = "wasm32"))]
use std::io::{self, Read};

#[cfg(not(target_arch = "wasm32"))]
use anyhow::Context;
#[cfg(not(target_arch = "wasm32"))]
use fast2flow_contracts::{Fast2FlowHookInV1, Fast2FlowHookOutV1};
#[cfg(not(target_arch = "wasm32"))]
use fast2flow_routing_gtpack::{telemetry, HostRuntime, ENV_TRACE_POLICY};
#[cfg(not(target_arch = "wasm32"))]
use tracing::{info, info_span, warn, Instrument};

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Logs go to stderr (or OTLP when configured); stdout carries the JSON
    // directive that callers parse, so it must stay clean. Default to `warn`
    // for this production hook; raise with `RUST_LOG`.
    telemetry::init("greentic-fast2flow-routing-host", "warn");

    let result = run().await;
    telemetry::shutdown();
    result
}

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
async fn run() -> anyhow::Result<()> {
    let mut payload = String::new();
    io::stdin()
        .read_to_string(&mut payload)
        .context("failed reading hook input JSON from stdin")?;

    let request: Fast2FlowHookInV1 =
        serde_json::from_str(&payload).context("failed parsing Fast2FlowHookInV1 JSON payload")?;

    let span = info_span!(
        "fast2flow.routing_host",
        scope = %request.scope,
        channel = request.envelope.channel.as_deref().unwrap_or(""),
        provider = request.envelope.provider.as_deref().unwrap_or(""),
        session_active = request.session_active,
        time_budget_ms = request.time_budget_ms,
    );

    async move {
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
            if let Some(policy) = &trace.policy {
                if !policy.warnings.is_empty() {
                    warn!(warnings = ?policy.warnings, "policy resolution produced warnings");
                }
            }
            eprintln!("{}", serde_json::to_string(&trace)?);
            out
        } else {
            runtime.route_from_mounts(request).await
        };

        info!(directive = ?output.directive, "routing hook resolved");
        println!("{}", serde_json::to_string(&output)?);
        Ok(())
    }
    .instrument(span)
    .await
}
