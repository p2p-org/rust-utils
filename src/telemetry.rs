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
use http::header::HeaderName;
use jsonrpsee::http_client::{HeaderMap, HeaderValue, Middleware};
use opentelemetry::{
    global,
    propagation::Injector,
    runtime,
    sdk::{propagation::TraceContextPropagator, trace as sdktrace, Resource},
};
use opentelemetry_semantic_conventions as semcov;
use sentry::ClientInitGuard;
use serde::Deserialize;
use std::str::FromStr;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_stackdriver::Stackdriver;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Registry};

use tracing::{subscriber::set_global_default, Span, Subscriber};
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub struct Telemetry(Option<ClientInitGuard>);

macro_rules! tracer {
    ($resource:ident, $pipeline:expr) => {{
        let mut pipeline = $pipeline;
        if let Some(ref name) = $resource.get(semcov::resource::SERVICE_NAME) {
            pipeline = pipeline.with_service_name(name.to_string());
        }

        pipeline = pipeline.with_trace_config(
            sdktrace::config()
                .with_resource($resource)
                .with_sampler(sdktrace::Sampler::AlwaysOn),
        );

        pipeline.install_batch(runtime::Tokio)?
    }};
}

impl Telemetry {
    /// Compose multiple layers into a `tracing`'s subscriber.
    ///
    /// # Implementation Notes
    ///
    /// We are using `impl Subscriber` as return type to avoid having to spell out the actual
    /// type of the returned subscriber, which is indeed quite complex.
    pub fn init(
        resource: Resource,
        tracing_settings: TracingSettings,
    ) -> anyhow::Result<(Self, impl Subscriber + Sync + Send)> {
        global::set_text_map_propagator(TraceContextPropagator::default());

        let name = resource.get(semcov::resource::SERVICE_NAME);

        let tracer = match tracing_settings.jaeger_collector {
            Some(collector_endpoint) => {
                let pipeline = opentelemetry_jaeger::new_collector_pipeline()
                    .with_reqwest()
                    .with_endpoint(collector_endpoint);

                tracer!(resource, pipeline)
            },
            // No explicit Jaeger collector set up, but we have environment
            // obviously set up to Jaeger collector
            None if std::env::var("OTEL_EXPORTER_JAEGER_ENDPOINT").is_ok() => {
                let pipeline = opentelemetry_jaeger::new_collector_pipeline().with_reqwest();

                tracer!(resource, pipeline)
            },
            None => {
                let pipeline = opentelemetry_jaeger::new_agent_pipeline();

                tracer!(resource, pipeline)
            },
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

        let name = name.map(|it| it.to_string()).unwrap_or_default();

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
/// insert a "traceparent" string value "{version}-{trace_id}-{span_id}-{trace-flags}" of the spans context into the
/// headers. Listeners can then re-hydrate the context to add additional spans to the same trace.
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
        let header_name = HeaderName::from_str(key).expect("Must be a header name value");
        let header_value = HeaderValue::from_str(&value).expect("Must be a header value");
        self.headers.insert(header_name, header_value);
    }
}

/// call with service name and version
///
/// ```rust
/// use axum_tracing_opentelemetry::make_resource;
/// # fn main() {
/// let r = make_resource(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
/// # }
/// ```
pub fn make_resource<S>(service_name: S, service_version: S) -> Resource
where
    S: Into<String>,
{
    Resource::new(vec![
        semcov::resource::SERVICE_NAME.string(service_name.into()),
        semcov::resource::SERVICE_VERSION.string(service_version.into()),
    ])
}
