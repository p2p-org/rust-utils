use jsonrpsee::{
    core::Error,
    http_client::{transport::HttpBackend, HttpClient as JsonRpcClient, HttpClientBuilder},
};
use tower::ServiceBuilder;
use tower_opentelemetry::{Layer as OpenTelemetryLayer, Service as OpenTelemetryService};

pub type HttpClient = JsonRpcClient<OpenTelemetryService<HttpBackend>>;

pub trait HttpClientExt {
    fn from_url(url: impl AsRef<str>) -> Result<Self, Error>
    where
        Self: Sized;
}

impl HttpClientExt for HttpClient {
    fn from_url(url: impl AsRef<str>) -> Result<Self, Error>
    where
        Self: Sized,
    {
        let middleware = ServiceBuilder::default().layer(OpenTelemetryLayer::new());
        let client = HttpClientBuilder::default().set_middleware(middleware).build(url)?;
        Ok(client)
    }
}
