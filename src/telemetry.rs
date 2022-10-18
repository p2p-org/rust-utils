//! Boilerplate for setting up and initializing tracing and OpenTelemetry
//!
//! This module contains methods for setting up and initializing tracing and OpenTelemetry.
//! We are using the `tracing` crate instead of `log` for logging
//!
//! # Usage
//! ```
//! use rust_utils::telemetry::Telemetry;
//! use rust_utils::telemetry::TracingSettings;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let tracing = TracingSettings::default(); // or use your own settings
//!
//!     let (telemetry, subscriber) = Telemetry::init("service-name".into(), tracing)?;
//!     Telemetry::init_subscriber(subscriber)?;
//!
//!     // ...
//!
//!     // proper flush of telemetry data
//!     telemetry.shutdown();
//!     Ok(())
//! }
//! ```

use anyhow::Context as anyhowContext;
use jsonrpsee::http_client::{HeaderMap, HeaderValue, Middleware};
use opentelemetry::{global, runtime, sdk::propagation::TraceContextPropagator};
use opentelemetry::propagation::Injector;
use sentry::ClientInitGuard;
use serde::Deserialize;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_stackdriver::Stackdriver;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};

use tracing::{subscriber::set_global_default, Subscriber, Span};
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub struct Telemetry(Option<ClientInitGuard>);

impl Telemetry {
    /// Compose multiple layers into a `tracing`'s subscriber.
    ///
    /// # Implementation Notes
    ///
    /// We are using `impl Subscriber` as return type to avoid having to spell out the actual
    /// type of the returned subscriber, which is indeed quite complex.
    pub fn init(
        name: String,
        tracing_settings: TracingSettings,
    ) -> anyhow::Result<(Self, impl Subscriber + Sync + Send)> {
        global::set_text_map_propagator(TraceContextPropagator::default());

        let tracer = match tracing_settings.jaeger_collector {
            Some(collector_endpoint) => {
                opentelemetry_jaeger::new_collector_pipeline()
                    .with_reqwest()
                    .with_service_name(&name)
                    .with_endpoint(collector_endpoint)
                    .install_batch(runtime::Tokio)?
            }
            // No explicit Jaeger collector set up, but we have environment
            // obviously set up to Jaeger collector
            None if std::env::var("OTEL_EXPORTER_JAEGER_ENDPOINT").is_ok() => {
                opentelemetry_jaeger::new_collector_pipeline()
                    .with_reqwest()
                    .with_service_name(&name)
                    .install_batch(runtime::Tokio)?
            }
            None => {
                opentelemetry_jaeger::new_agent_pipeline()
                    .with_service_name(&name)
                    .install_batch(runtime::Tokio)?
            }
        };

        let tracer = tracing_opentelemetry::layer().with_tracer(tracer);

        let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&tracing_settings.spec));

        // Google Cloud Operations Suite structured logging (formerly Stackdriver).
        // https://cloud.google.com/logging/docs/structured-logging
        let stackdriver = if tracing_settings.gclogs {
            Some(Stackdriver::layer())
        } else {
            None
        };

        // We are using BunyanFormattingLayer instead of tracing_subscriber::fmt because
        // fmt does not implement metadata inheritance
        let formatting_layer = if stackdriver.is_none() {
            Some(BunyanFormattingLayer::new(name, std::io::stdout))
        } else {
            None
        };

        let (sentry_layer, sentry_guard) = if let Some(sentry_url) = tracing_settings.sentry_server {
            let guard = Some(sentry::init((sentry_url, sentry::ClientOptions {
                release: sentry::release_name!(),
                ..Default::default()
            })));
            let layer = Some(sentry_tracing::layer());
            (layer, guard)
        } else {
            (None, None)
        };

        let subscriber = Registry::default()
            .with(env_filter)
            .with(JsonStorageLayer)
            .with(tracer)
            .with(sentry_layer)
            .with(formatting_layer)
            .with(stackdriver);

        Ok((Self(sentry_guard), subscriber))
    }

    /// Register a subscriber as global default to process span data.
    ///
    /// It should only be called once!
    pub fn init_subscriber(subscriber: impl Subscriber + Sync + Send) -> anyhow::Result<()> {
        LogTracer::init().context("Failed to set logger")?;
        set_global_default(subscriber).context("Failed to set subscriber")?;
        Ok(())
    }

    pub fn shutdown(self) {
        global::shutdown_tracer_provider();
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(default)]
pub struct TracingSettings {
    #[serde(default = "default_spec")]
    pub spec: String,

    #[serde(default)]
    pub gclogs: bool,

    #[serde(default)]
    pub sentry_server: Option<String>,

    #[serde(default)]
    pub jaeger_collector: Option<String>,
}

impl Default for TracingSettings {
    fn default() -> Self {
        Self {
            spec: default_spec(),
            gclogs: false,
            sentry_server: None,
            jaeger_collector: None,
        }
    }
}

fn default_spec() -> String {
    "info".into()
}

pub struct TracePropagatorMiddleware;

/// Injects the given OpenTelemetry Context into headers to allow propagation downstream.
#[async_trait::async_trait]
impl Middleware for TracePropagatorMiddleware {
    async fn handle(&self, headers: &mut HeaderMap) {
        let context = Span::current().context();

        global::get_text_map_propagator(|injector| {
            injector.inject_context(&context, &mut HeadersCarrier::new(headers))
        });
    }
}

// "traceparent" => https://www.w3.org/TR/trace-context/#trace-context-http-headers-format

/// Injector used via opentelemetry propagator to tell the extractor how to insert the "traceparent" header value
/// This will allow the propagator to inject opentelemetry context into a standard data structure. Will basically
/// insert a "traceparent" string value "{version}-{trace_id}-{span_id}-{trace-flags}" of the spans context into the headers.
/// Listeners can then re-hydrate the context to add additional spans to the same trace.
struct HeadersCarrier<'a> {
    headers: &'a mut HeaderMap,
}

impl<'a> HeadersCarrier<'a> {
    pub fn new(headers: &'a mut HeaderMap) -> Self {
        HeadersCarrier { headers }
    }
}

impl<'a> Injector for HeadersCarrier<'a> {
    fn set(&mut self, key: &str, value: String) {
        let header_value = HeaderValue::from_str(&value).expect("Must be a header value");
        self.headers.insert(key, header_value);
    }
}

