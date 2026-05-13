//! Telemetry bootstrap shared by the Fast2Flow binaries.
//!
//! When an OTLP export target is configured via the standard environment
//! variables (`TELEMETRY_EXPORT`, `OTLP_ENDPOINT`, `OTEL_EXPORTER_OTLP_ENDPOINT`)
//! we hand off to [`greentic_telemetry`] so spans/events flow to the collector
//! with the project's redaction rules applied.
//!
//! Otherwise we install a plain `tracing` formatter that writes to **stderr**.
//! Both binaries use stdout as a data/protocol channel (JSON results), so logs
//! must never go there. Levels follow `RUST_LOG`; absent that, `default_level`.

use tracing_subscriber::{fmt, EnvFilter};

fn export_target_configured() -> bool {
    ["TELEMETRY_EXPORT", "OTLP_ENDPOINT", "OTEL_EXPORTER_OTLP_ENDPOINT"]
        .iter()
        .any(|key| std::env::var_os(key).is_some())
}

/// Installs the process-wide telemetry subscriber. Safe to call once at startup;
/// errors are reported on stderr and otherwise ignored so a misconfigured
/// collector never takes the binary down.
pub fn init(service_name: &str, default_level: &str) {
    if export_target_configured() {
        if let Err(err) = greentic_telemetry::init_telemetry_auto(greentic_telemetry::TelemetryConfig {
            service_name: service_name.to_string(),
        }) {
            eprintln!("warn: failed to initialize telemetry export: {err}");
        }
        return;
    }

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_level));
    let _ = fmt()
        .with_writer(std::io::stderr)
        .with_target(false)
        .with_env_filter(filter)
        .try_init();
}

/// Flushes any buffered spans (no-op unless an OTLP exporter is active).
pub fn shutdown() {
    greentic_telemetry::shutdown();
}
