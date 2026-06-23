// SPDX-License-Identifier: MIT OR Apache-2.0
//! OpenTelemetry-compatible span instrumentation for Tasken.
//!
//! Vendored from `spikes/rust/otel_schema_ext.rs` (the v11 router T4.1
//! spike) and extended with a Tasken-specific `add_event` extension that
//! emits OTel span events (timestamped annotations on the span, attached
//! to a `tracing::info!` event while entered in the span context).
//!
//! # Layout
//!
//! 1. [`RouterSpanKind`] — OTel span kinds Tasken emits (currently:
//!    [`RouterSpanKind::Decision`], [`RouterSpanKind::ProviderCall`]).
//! 2. [`AttrValue`] + [`RouterSpanAttributes`] — strongly-typed attribute
//!    set; deterministic iteration order for snapshot tests.
//! 3. [`OtelSpanExt`] — backend-agnostic trait; the contract every
//!    router code path holds. Production wires the [`TracingOtelSpan`]
//!    impl; tests can substitute the in-memory [`TracingOtelSpan`] (which
//!    is itself tracing-based, so a `tracing` subscriber can capture).
//! 4. [`TracingOtelSpan`] — `tracing::Span`-backed impl with
//!    `add_event()` + `record_task_state()` extensions.
//! 5. [`duration_ms`] — `Duration` → `latency_ms` helper.
//!
//! # Wiring
//!
//! See `application::services::TaskService::run_task` and
//! `TaskService::execute_workflow` for the two integration points. Both
//! create a `TracingOtelSpan` on entry, call `add_event(...)` on each
//! state transition, and end the span on return.
//!
//! # Feature flag
//!
//! This module is compiled only when `Cargo.toml`'s `otel` feature is
//! enabled. The `tracing` and `opentelemetry` optional dependencies are
//! activated by the same feature.

use std::time::Duration;

// ---------------------------------------------------------------------------
// 1. Span kind enum
// ---------------------------------------------------------------------------

/// Canonical OTel span kinds emitted by Tasken.
///
/// The string form (`as_str`) is the OTel span name and is the primary
/// routing key in backends (Jaeger service, Tempo, Honeycomb, etc.).
/// `otlp_kind()` mirrors the OTel proto enum
/// (`opentelemetry.proto.trace.v1.SpanKind`).
///
/// Tasken currently emits two of the four kinds defined in the v11
/// router spike:
/// - [`Self::Decision`] — a workflow-level routing decision (one per
///   `execute_workflow` call).
/// - [`Self::ProviderCall`] — an outbound plugin/shell execution
///   (one per `run_task` call).
///
/// The other two ([`Self::Plugin`], [`Self::HotReload`]) are reserved
/// for future router features and are re-exported so Tasken's span
/// vocabulary is a strict subset of the spike's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RouterSpanKind {
    /// Workflow / run-level routing decision (provider + plugin selection).
    Decision,
    /// Plugin pre- or post-processing step.
    Plugin,
    /// Provider / plugin execution (LLM, embedding, shell command).
    ProviderCall,
    /// Plugin or config hot-reload.
    HotReload,
}

impl RouterSpanKind {
    /// OTel span name. Matches the Rust enum variant 1:1 and the Go
    /// `SpanKindForRouter()` switch in `pheno_tracing_go_client.go`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Decision => "router.decision",
            Self::Plugin => "router.plugin",
            Self::ProviderCall => "router.provider_call",
            Self::HotReload => "router.hot_reload",
        }
    }

    /// OTel proto `SpanKind` value:
    ///   1 = INTERNAL, 2 = SERVER, 3 = CLIENT, 4 = PRODUCER, 5 = CONSUMER.
    ///
    /// Decision / Plugin / HotReload are internal lifecycle events;
    /// ProviderCall is an outbound call to a plugin / shell.
    pub const fn otlp_kind(self) -> u32 {
        match self {
            Self::Decision | Self::Plugin | Self::HotReload => 1, // INTERNAL
            Self::ProviderCall => 3,                              // CLIENT
        }
    }
}

// ---------------------------------------------------------------------------
// 2. Typed attribute value + attribute set
// ---------------------------------------------------------------------------

/// Typed OTel attribute value passed to `OtelSpanExt::set_attribute`.
#[derive(Debug, Clone, Copy)]
pub enum AttrValue<'a> {
    Str(&'a str),
    F64(f64),
    I64(i64),
    Bool(bool),
}

/// Strongly-typed attribute set for a router span.
///
/// All fields are optional. Only `Some(_)` values are emitted as OTel
/// attributes; `None` fields are silently skipped (no `null` payloads).
#[derive(Debug, Default, Clone)]
pub struct RouterSpanAttributes {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub plugin_name: Option<String>,
    pub decision_reason: Option<String>,
    pub latency_ms: Option<f64>,
    pub cost_usd: Option<f64>,
    pub status_code: Option<i32>,
}

impl RouterSpanAttributes {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            provider: None,
            model: None,
            plugin_name: None,
            decision_reason: None,
            latency_ms: None,
            cost_usd: None,
            status_code: None,
        }
    }

    #[must_use]
    pub fn provider(mut self, v: impl Into<String>) -> Self {
        self.provider = Some(v.into());
        self
    }
    #[must_use]
    pub fn model(mut self, v: impl Into<String>) -> Self {
        self.model = Some(v.into());
        self
    }
    #[must_use]
    pub fn plugin_name(mut self, v: impl Into<String>) -> Self {
        self.plugin_name = Some(v.into());
        self
    }
    #[must_use]
    pub fn decision_reason(mut self, v: impl Into<String>) -> Self {
        self.decision_reason = Some(v.into());
        self
    }
    #[must_use]
    pub fn latency_ms(mut self, v: f64) -> Self {
        self.latency_ms = Some(v);
        self
    }
    #[must_use]
    pub fn cost_usd(mut self, v: f64) -> Self {
        self.cost_usd = Some(v);
        self
    }
    #[must_use]
    pub fn status_code(mut self, v: i32) -> Self {
        self.status_code = Some(v);
        self
    }

    /// Yield `(key, typed-value)` pairs for every present attribute.
    /// Iteration order is fixed so the OTel payload is deterministic
    /// (helpful for snapshot/golden tests).
    pub fn iter(&self) -> impl Iterator<Item = (&'static str, AttrValue<'_>)> {
        [
            self.provider.as_deref().map(|v| ("provider", AttrValue::Str(v))),
            self.model.as_deref().map(|v| ("model", AttrValue::Str(v))),
            self.plugin_name.as_deref().map(|v| ("plugin_name", AttrValue::Str(v))),
            self.decision_reason.as_deref().map(|v| ("decision_reason", AttrValue::Str(v))),
            self.latency_ms.map(|v| ("latency_ms", AttrValue::F64(v))),
            self.cost_usd.map(|v| ("cost_usd", AttrValue::F64(v))),
            self.status_code.map(|v| ("status_code", AttrValue::I64(i64::from(v)))),
        ]
        .into_iter()
        .flatten()
    }
}

// ---------------------------------------------------------------------------
// 3. OtelSpanExt trait
// ---------------------------------------------------------------------------

/// Backend-agnostic abstraction over the OTel span API.
///
/// Every router code path holds an `OtelSpanExt` impl — never a concrete
/// `tracing::Span` — so observability can be retargeted (tonic OTLP/gRPC,
/// OTLP/HTTP, in-memory test exporter, no-op for prod paths where tracing
/// is disabled) without touching the call sites.
///
/// **Tasken-specific note:** the production impl is
/// [`TracingOtelSpan`], which wraps `tracing::Span`. The `add_event` /
/// `record_task_state` extension methods are NOT part of the trait — they
/// are inherent methods on `TracingOtelSpan` because they require the
/// `tracing::Span::enter()` borrow. Trait-only code paths can still use
/// `set_attribute` / `set_ok` / `set_error` / `end` to satisfy the
/// OTel-shaped schema.
pub trait OtelSpanExt {
    /// Override the span kind (default is set in the constructor).
    fn set_kind(&self, kind: RouterSpanKind);

    /// Set a typed attribute. Overwrites if the key already exists.
    fn set_attribute(&self, key: &'static str, value: AttrValue<'_>);

    /// Record the end-to-end latency of the operation (milliseconds).
    fn record_latency_ms(&self, ms: f64);

    /// Record the dollar cost of the operation (US dollars).
    fn record_cost_usd(&self, usd: f64);

    /// Record the HTTP/Provider status code (e.g. 200, 429, 500).
    fn set_status_code(&self, code: i32);

    /// Mark the span OK (OTel StatusCode::Ok = 1).
    fn set_ok(&self);

    /// Mark the span Error with a message (OTel StatusCode::Error = 2).
    fn set_error(&self, msg: impl AsRef<str>);

    /// Finish and emit the span. After `end` the impl is consumed.
    fn end(self);
}

// ---------------------------------------------------------------------------
// 4. Concrete impl: TracingOtelSpan
// ---------------------------------------------------------------------------

/// Internal helper: record a value into a `tracing::Span` by field name.
/// Silently no-ops if the field was not declared on the span (defensive —
/// we always declare every key in the constructor, but this keeps the
/// API safe against future partial declarations).
fn record_field<V: tracing::Value>(span: &tracing::Span, name: &'static str, value: &V) {
    if let Some(meta) = span.metadata() {
        if let Some(field) = meta.fields().field(name) {
            span.record(&field, value);
        }
    }
}

/// Concrete `OtelSpanExt` impl that wraps a `tracing::Span`.
///
/// All 7 attribute keys + the 2 OTel status fields are pre-declared
/// as `field::Empty` so subsequent `record_field` calls never panic
/// on "field not found".
#[derive(Debug)]
pub struct TracingOtelSpan {
    span: tracing::Span,
}

impl TracingOtelSpan {
    /// Build a router span with the canonical name and all attribute
    /// slots pre-declared, then populate the initial attribute set.
    ///
    /// Span name: the OTel span name (one of the 4 `RouterSpanKind::as_str`
    /// values). The OTel kind enum value is recorded as `otel.kind_code`.
    pub fn new(kind: RouterSpanKind, attrs: RouterSpanAttributes) -> Self {
        use tracing::field;

        let span = tracing::info_span!(
            "router",
            otel.kind = kind.as_str(),
            otel.kind_code = kind.otlp_kind(),
            // 7 attribute slots:
            provider = field::Empty,
            model = field::Empty,
            plugin_name = field::Empty,
            decision_reason = field::Empty,
            latency_ms = field::Empty,
            cost_usd = field::Empty,
            status_code = field::Empty,
            // OTel status sub-message:
            otel.status_code = field::Empty,
            otel.status_message = field::Empty,
            // Tasken extension slots (read by record_task_state):
            task.id = field::Empty,
            task.name = field::Empty,
            task.priority = field::Empty,
        );
        let me = Self { span };
        for (k, v) in attrs.iter() {
            OtelSpanExt::set_attribute(&me, k, v);
        }
        me
    }

    /// Borrow the inner `tracing::Span` (for `#[instrument]` adapters and
    /// callers that need the entered-guard pattern via `.entered()`).
    #[must_use]
    pub fn inner(&self) -> &tracing::Span {
        &self.span
    }

    /// Consume the wrapper and return the inner `tracing::Span`.
    /// The caller can then use `.entered()` (modern tracing returns
    /// `EnteredSpan`) or simply drop the span to end it.
    pub fn into_inner(self) -> tracing::Span {
        self.span
    }

    /// Emit a span event (OTel "span event" — timestamped annotation
    /// attached to the current span). Maps to a `tracing::info!` event
    /// emitted while entered in the span's context, which the OTel
    /// exporter turns into an OTel `Span.Event`.
    ///
    /// `name` should be a stable identifier like `"state.running"`,
    /// `"state.completed"`, `"state.failed"`. Any downstream OTel
    /// tooling can filter / count / alert on these.
    pub fn add_event(&self, name: &'static str) {
        // Enter the span so the `info!` event is parented to it. The
        // guard `_g` is dropped at the end of the statement, so the
        // span is only entered for the duration of `tracing::info!`.
        let _g = self.span.enter();
        tracing::info!(otel.event = name, task.event = name, "{name}");
    }

    /// Record a Tasken-specific state transition in one shot: emits a
    /// span event AND records the TaskState as an attribute. Use this at
    /// each `task.transition_to(...)` site to keep the OTel payload
    /// consistent.
    pub fn record_task_state(&self, state: &str) {
        self.add_event(match state {
            "Pending" => "state.pending",
            "Running" => "state.running",
            "Completed" => "state.completed",
            "Failed" => "state.failed",
            "Cancelled" => "state.cancelled",
            "Skipped" => "state.skipped",
            other => "state.unknown", // unknown states still emit, just with a fallback name
        });
        record_field(&self.span, "task.state", &state);
    }
}

impl OtelSpanExt for TracingOtelSpan {
    fn set_kind(&self, kind: RouterSpanKind) {
        record_field(&self.span, "otel.kind", &kind.as_str());
        record_field(&self.span, "otel.kind_code", &kind.otlp_kind());
    }

    fn set_attribute(&self, key: &'static str, value: AttrValue<'_>) {
        match value {
            AttrValue::Str(s) => record_field(&self.span, key, &s),
            AttrValue::F64(f) => record_field(&self.span, key, &f),
            AttrValue::I64(i) => record_field(&self.span, key, &i),
            AttrValue::Bool(b) => record_field(&self.span, key, &b),
        }
    }

    fn record_latency_ms(&self, ms: f64) {
        record_field(&self.span, "latency_ms", &ms);
    }

    fn record_cost_usd(&self, usd: f64) {
        record_field(&self.span, "cost_usd", &usd);
    }

    fn set_status_code(&self, code: i32) {
        record_field(&self.span, "status_code", &i64::from(code));
    }

    fn set_ok(&self) {
        // OTel StatusCode value: 1 = Ok, 2 = Error.
        record_field(&self.span, "otel.status_code", &1_i64);
        record_field(&self.span, "otel.status_message", &"OK");
    }

    fn set_error(&self, msg: impl AsRef<str>) {
        record_field(&self.span, "otel.status_code", &2_i64);
        let s = msg.as_ref();
        record_field(&self.span, "otel.status_message", &s);
    }

    fn end(self) {
        // tracing::Span ends automatically when dropped. Consuming self
        // here ensures all recorded attributes are flushed before the
        // span closes; the explicit drop is documentation as much as code.
        drop(self);
    }
}

// ---------------------------------------------------------------------------
// 5. Convenience: turn a std::time::Duration into latency_ms
// ---------------------------------------------------------------------------

/// Convenience for the very common "elapsed since start" pattern:
/// `span.record_latency_ms(duration_ms(start.elapsed()))`.
#[must_use]
pub fn duration_ms(d: Duration) -> f64 {
    // 1 ms = 1_000_000 ns; f64 has plenty of headroom for any realistic
    // router span (sub-microsecond resolution through multi-day uptime).
    d.as_nanos() as f64 / 1_000_000.0
}

// ---------------------------------------------------------------------------
// 6. Tracer init stub (Tasken-specific)
// ---------------------------------------------------------------------------

/// Initialize an OpenTelemetry tracer provider (stub).
///
/// This is a **deliberate stub**: Tasken does not commit to a specific
/// exporter (stdout, OTLP/HTTP, OTLP/gRPC, Jaeger) — that's a deployment
/// decision. Embedders should replace this with a real provider, then
/// bridge it to `tracing` via `tracing-opentelemetry`:
///
/// ```ignore
/// use opentelemetry::trace::TracerProvider;
/// use opentelemetry_sdk::trace::TracerProvider as SdkTracerProvider;
/// use tracing_opentelemetry::OpenTelemetryLayer;
/// use tracing_subscriber::layer::SubscriberExt;
/// use tracing_subscriber::Registry;
///
/// let provider = SdkTracerProvider::builder().build();
/// let tracer = provider.tracer("tasken");
/// let otel_layer = OpenTelemetryLayer::new(tracer);
/// let subscriber = Registry::default().with(otel_layer);
/// tracing::subscriber::set_global_default(subscriber).unwrap();
/// ```
///
/// The [`opentelemetry`](https://docs.rs/opentelemetry) crate is
/// re-exported from this module so callers don't need a direct dep.
pub fn init_tracer() {
    // Intentionally empty — see doc comment above. We don't install a
    // global subscriber here; that's a process-wide side effect that
    // embedding applications should control. The presence of this
    // function is the integration point.
}

// Re-export the `opentelemetry` crate so embedding apps can build
// exporters without taking a direct dep on a specific version. This
// also gives a stable compile-time anchor: if `opentelemetry`'s API
// breaks, this re-export breaks first and loudly.
pub use opentelemetry;

// ---------------------------------------------------------------------------
// 7. Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tracing::field::Visit;
    use tracing::span::{Attributes, Id, Record};
    use tracing::{Event, Subscriber};
    use tracing_subscriber::layer::{Context, Layer};
    use tracing_subscriber::Registry;

    use super::*;

    /// Test subscriber state: captures both span field recordings AND
    /// `tracing::info!` events as (field_name, value) pairs / strings.
    #[derive(Default)]
    struct Captured {
        /// `(field_name, value_debug)` for every `span.record(...)` call.
        field_records: Vec<(String, String)>,
        /// Decoded message from every `tracing::info!` event.
        events: Vec<String>,
    }

    /// Subscriber that snapshots all span field recordings + events.
    /// `Arc<Mutex<_>>` so tests can pull the captured data out of the
    /// `with_default` closure.
    #[derive(Clone, Default)]
    struct TestLayer {
        captured: Arc<Mutex<Captured>>,
    }

    impl<S> Layer<S> for TestLayer
    where
        S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    {
        fn on_new_span(&self, _attrs: &Attributes<'_>, _id: &Id, _ctx: Context<'_, S>) {
            // Span creation isn't what we're testing — we test attribute
            // recording + event emission. No-op.
        }

        fn on_record(&self, _id: &Id, values: &Record<'_>, _ctx: Context<'_, S>) {
            struct V(Vec<(String, String)>);
            impl Visit for V {
                fn record_debug(
                    &mut self,
                    field: &tracing::field::Field,
                    value: &dyn std::fmt::Debug,
                ) {
                    self.0.push((field.name().to_string(), format!("{value:?}")));
                }
                fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                    self.0.push((field.name().to_string(), format!("{value:?}")));
                }
                fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
                    self.0.push((field.name().to_string(), value.to_string()));
                }
                fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
                    self.0.push((field.name().to_string(), value.to_string()));
                }
                fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
                    self.0.push((field.name().to_string(), value.to_string()));
                }
                fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
                    self.0.push((field.name().to_string(), value.to_string()));
                }
            }
            let mut visitor = V(Vec::new());
            values.record(&mut visitor);
            self.captured.lock().expect("captured mutex poisoned").field_records.extend(visitor.0);
        }

        fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
            struct V(String);
            impl Visit for V {
                fn record_debug(
                    &mut self,
                    field: &tracing::field::Field,
                    value: &dyn std::fmt::Debug,
                ) {
                    if field.name() == "message" {
                        self.0 = format!("{value:?}");
                    } else if self.0.is_empty() {
                        // First non-message field — also capture for visibility.
                        self.0 = format!("{}={:?}", field.name(), value);
                    }
                }
                fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                    if field.name() == "message" {
                        self.0 = format!("{value:?}");
                    } else if self.0.is_empty() {
                        self.0 = format!("{}={value:?}", field.name());
                    }
                }
            }
            let mut visitor = V(String::new());
            event.record(&mut visitor);
            self.captured.lock().expect("captured mutex poisoned").events.push(visitor.0);
        }
    }

    /// Test 1: verify that `OtelSpanExt::set_attribute` (and the
    /// `RouterSpanAttributes` constructor) actually record values on the
    /// underlying `tracing::Span`. Uses a custom subscriber layer to
    /// capture every `span.record(...)` call so we can assert on the
    /// recorded `(field, value)` pairs.
    #[test]
    fn attributes_are_set_on_span() {
        let layer = TestLayer::default();
        let captured = Arc::clone(&layer.captured);
        let subscriber = Registry::default().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            let span = TracingOtelSpan::new(
                RouterSpanKind::ProviderCall,
                RouterSpanAttributes::new()
                    .provider("shell")
                    .model("echo hello")
                    .decision_reason("run_task"),
            );
            // Mutation after construction — must also be recorded.
            span.set_attribute("plugin_name", AttrValue::Str("shell"));
            span.record_latency_ms(42.5);
            span.record_cost_usd(0.0);
            span.set_status_code(0);
            span.set_ok();
            span.end();
        });

        let cap = captured.lock().expect("captured mutex poisoned");

        // Helper: returns true iff a record for `field` with `value_substr`
        // exists in the captured list.
        let has_record = |field: &str, value_substr: &str| -> bool {
            cap.field_records.iter().any(|(k, v)| k == field && v.contains(value_substr))
        };

        // Constructor-populated attributes:
        assert!(
            has_record("provider", "shell"),
            "missing 'provider' record; got: {:?}",
            cap.field_records
        );
        assert!(
            has_record("model", "echo hello"),
            "missing 'model' record; got: {:?}",
            cap.field_records
        );
        assert!(
            has_record("decision_reason", "run_task"),
            "missing 'decision_reason' record; got: {:?}",
            cap.field_records
        );
        // otel.kind / otel.kind_code are set by `new`:
        assert!(
            has_record("otel.kind", "router.provider_call"),
            "missing 'otel.kind' record; got: {:?}",
            cap.field_records
        );
        assert!(
            has_record("otel.kind_code", "3"),
            "missing 'otel.kind_code' record; got: {:?}",
            cap.field_records
        );

        // Post-construction mutations:
        assert!(
            has_record("plugin_name", "shell"),
            "missing 'plugin_name' record; got: {:?}",
            cap.field_records
        );
        assert!(
            has_record("latency_ms", "42.5"),
            "missing 'latency_ms' record; got: {:?}",
            cap.field_records
        );
        assert!(
            has_record("status_code", "0"),
            "missing 'status_code' record; got: {:?}",
            cap.field_records
        );

        // set_ok should record otel.status_code = 1 and status_message = OK:
        assert!(
            has_record("otel.status_code", "1"),
            "missing 'otel.status_code=1' (set_ok); got: {:?}",
            cap.field_records
        );
        assert!(
            has_record("otel.status_message", "OK"),
            "missing 'otel.status_message' (set_ok); got: {:?}",
            cap.field_records
        );
    }

    /// Test 2: verify that span events fire on state transitions. The
    /// `add_event` method emits a `tracing::info!` event while entered
    /// in the span's context, so a subscriber's `on_event` callback
    /// should see one event per `add_event` call, AND `record_task_state`
    /// should both fire an event AND record the `task.state` attribute.
    #[test]
    fn events_fire_on_state_transitions() {
        let layer = TestLayer::default();
        let captured = Arc::clone(&layer.captured);
        let subscriber = Registry::default().with(layer);

        tracing::subscriber::with_default(subscriber, || {
            let span = TracingOtelSpan::new(
                RouterSpanKind::ProviderCall,
                RouterSpanAttributes::new().provider("shell"),
            );

            // Simulate the run_task state machine:
            span.record_task_state("Pending");
            span.record_task_state("Running");
            span.record_task_state("Completed");
            span.set_ok();
            span.end();
        });

        let cap = captured.lock().expect("captured mutex poisoned");

        // Three events should have been emitted, one per state transition.
        // The visitor stores the message field; for `tracing::info!(otel.event = name, task.event = name, "{name}")`
        // the message is the formatted name string.
        assert_eq!(
            cap.events.len(),
            3,
            "expected 3 events, got {}: {:?}",
            cap.events.len(),
            cap.events
        );
        // Event messages should reflect the state-transition event names:
        let joined: Vec<&str> = cap.events.iter().map(String::as_str).collect();
        assert!(
            joined.iter().any(|m| m.contains("state.pending")),
            "missing 'state.pending' event; events: {:?}",
            cap.events
        );
        assert!(
            joined.iter().any(|m| m.contains("state.running")),
            "missing 'state.running' event; events: {:?}",
            cap.events
        );
        assert!(
            joined.iter().any(|m| m.contains("state.completed")),
            "missing 'state.completed' event; events: {:?}",
            cap.events
        );

        // Additionally, `record_task_state` records `task.state` as an
        // attribute. The last value wins (overwritten on each call), so
        // we should see "Completed" as the final recorded value.
        let final_state = cap.field_records.iter().rev().find(|(k, _)| k == "task.state");
        assert!(
            final_state.is_some(),
            "missing 'task.state' field record; got: {:?}",
            cap.field_records
        );
        let (k, v) = final_state.expect("just checked is_some");
        assert_eq!(k, "task.state");
        assert!(v.contains("Completed"), "expected last task.state to be 'Completed', got: {v:?}");
    }

    // -- Additional sanity tests (cheap, keep coverage tight) --

    #[test]
    fn kind_names_match_schema() {
        assert_eq!(RouterSpanKind::Decision.as_str(), "router.decision");
        assert_eq!(RouterSpanKind::Plugin.as_str(), "router.plugin");
        assert_eq!(RouterSpanKind::ProviderCall.as_str(), "router.provider_call");
        assert_eq!(RouterSpanKind::HotReload.as_str(), "router.hot_reload");
    }

    #[test]
    fn otlp_kind_is_internal_or_client() {
        for k in [RouterSpanKind::Decision, RouterSpanKind::Plugin, RouterSpanKind::HotReload] {
            assert_eq!(k.otlp_kind(), 1, "{k:?}");
        }
        assert_eq!(RouterSpanKind::ProviderCall.otlp_kind(), 3);
    }

    #[test]
    fn duration_ms_conversion_is_correct() {
        assert_eq!(duration_ms(Duration::from_millis(1)), 1.0);
        assert_eq!(duration_ms(Duration::from_micros(1_500)), 1.5);
        assert_eq!(duration_ms(Duration::from_secs(2)), 2_000.0);
    }

    #[test]
    fn attributes_iter_is_deterministic() {
        let a =
            RouterSpanAttributes::new().provider("openai").model("gpt-4o-mini").latency_ms(123.4);

        let kvs: Vec<&'static str> = a.iter().map(|(k, _)| k).collect();
        assert_eq!(kvs, vec!["provider", "model", "latency_ms"]);
    }

    #[test]
    fn end_to_end_ok_span() {
        let span = TracingOtelSpan::new(
            RouterSpanKind::Decision,
            RouterSpanAttributes::new()
                .provider("openai")
                .model("gpt-4o-mini")
                .decision_reason("cost_optimizer"),
        );
        span.record_latency_ms(42.5);
        span.set_status_code(200);
        span.set_ok();
        span.end();
    }

    #[test]
    fn end_to_end_error_span() {
        let span = TracingOtelSpan::new(
            RouterSpanKind::ProviderCall,
            RouterSpanAttributes::new().provider("openai").model("gpt-4o").status_code(429),
        );
        span.record_latency_ms(1_250.0);
        span.set_error("rate limit exceeded");
        span.end();
    }
}
