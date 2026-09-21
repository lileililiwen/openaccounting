//! Opt-in OpenTelemetry tracing (`ops-hardening`).
//!
//! Disabled by default (`TRACING_ENABLED` unset or anything but
//! `true`/`1`/`yes`): small deploys pay no exporter overhead and
//! the existing log subscriber keeps working. When enabled, spans
//! export OTLP/HTTP to `OTEL_EXPORTER_OTLP_ENDPOINT`
//! (default `http://localhost:4318`).
//!
//! Span redaction (see `docs/threat-model.md`): spans record only
//! `request_id`, `method`, `route`, and `status`. Emails, names,
//! amounts, tokens, and cookie values are never span fields.

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;

/// Build the OTLP span-export layer when `TRACING_ENABLED` is set.
/// Returns `None` (disabled) otherwise, or when the exporter
/// cannot be constructed — callers fall back to logs only.
pub fn otel_layer() -> Option<
    tracing_opentelemetry::OpenTelemetryLayer<
        tracing_subscriber::Registry,
        opentelemetry_sdk::trace::Tracer,
    >,
> {
    if !enabled() {
        return None;
    }
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:4318".to_string());
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(format!("{endpoint}/v1/traces"))
        .build()
        .ok()?;
    let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .build();
    let tracer = provider.tracer("openaccounting");
    // The provider must outlive the process so batch exports
    // flush; the registry owns the layer, the provider is
    // intentionally leaked here (process-lifetime singleton).
    std::mem::forget(provider);
    Some(tracing_opentelemetry::layer().with_tracer(tracer))
}

pub fn enabled() -> bool {
    matches!(
        std::env::var("TRACING_ENABLED")
            .unwrap_or_default()
            .to_lowercase()
            .as_str(),
        "true" | "1" | "yes"
    )
}

/// Best-effort init for entry points that do not own a subscriber
/// builder (tests, CLI). Production `main` composes
/// [`otel_layer`] into its `fmt` subscriber instead.
pub fn init() {
    use tracing_subscriber::layer::SubscriberExt;
    static ONCE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        if let Some(layer) = otel_layer() {
            let subscriber = tracing_subscriber::registry().with(layer);
            let _ = tracing::subscriber::set_global_default(subscriber);
        }
    });
}

/// Redaction helper: hash an identifier before it may appear in
/// any span or log field. Callers pass user/ledger IDs; raw emails
/// and tokens must never reach spans at all.
pub fn hash_id(id: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_by_default() {
        // TRACING_ENABLED unset in the test env: no layer, no panic.
        assert!(!enabled());
        assert!(otel_layer().is_none());
        init();
    }

    #[test]
    fn hash_is_stable_and_hides_input() {
        let a = hash_id("user-123");
        assert_eq!(a, hash_id("user-123"));
        assert!(!a.contains("user-123"));
        assert_ne!(a, hash_id("user-124"));
    }
}
