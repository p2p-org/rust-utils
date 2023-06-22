use axum_tracing_opentelemetry::opentelemetry_tracing_layer;
use jsonrpsee::{
    core::error::Error,
    server::{
        logger::Logger, middleware::proxy_get_request::ProxyGetRequestLayer, AllowHosts, ServerBuilder, ServerHandle,
    },
    Methods,
};
use std::{future::Future, net::SocketAddr};
use tokio::{net::ToSocketAddrs, signal, task::JoinHandle};
use tower::{
    layer::util::{Identity, Stack},
    util::Either,
    Layer, Service, ServiceBuilder,
};
use tower_http::{
    classify::{ServerErrorsAsFailures, SharedClassifier},
    cors::CorsLayer,
    trace::TraceLayer,
};

pub struct Server {
    address: SocketAddr,
    handle: ServerHandle,
}

impl Server {
    pub fn from_handle(address: SocketAddr, handle: ServerHandle) -> Self {
        Self { address, handle }
    }

    pub async fn with_address(address: impl ToSocketAddrs, service: impl Into<Methods>) -> Result<Self, Error> {
        let middleware = ServiceBuilder::default()
            .layer(opentelemetry_tracing_layer())
            .layer(CorsLayer::permissive())
            .option_layer(
                service
                    .method("system_liveness")
                    .map(|_| ProxyGetRequestLayer::new("/liveness", "system_liveness").unwrap()),
            )
            .option_layer(
                service
                    .method("system_readiness")
                    .map(|_| ProxyGetRequestLayer::new("/readiness", "system_readiness").unwrap()),
            )
            .option_layer(
                service
                    .method("version")
                    .map(|_| ProxyGetRequestLayer::new("/version", "version").unwrap()),
            );

        let server = ServerBuilder::default()
            .set_host_filtering(AllowHosts::Any)
            .set_middleware(middleware)
            .http_only()
            .build(address)
            .await?;

        Ok(Self {
            address: server.local_addr()?,
            handle: server.start(service.into())?,
        })
    }

    pub async fn stop(self) -> Result<(), Error> {
        self.handle.stop()?;
        self.handle.stopped().await;
        Ok(())
    }

    pub async fn with_graceful_shutdown<F>(self, signal: F)
    where
        F: Future<Output = ()>,
    {
        signal.await;
        match self.stop().await {
            Ok(_) => {
                tracing::info!("server stopped successfully");
            },
            Err(error) => {
                tracing::warn!("failed to stop the server: {error}");
            },
        }
    }

    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(self.handle.stopped())
    }

    pub fn address(&self) -> &SocketAddr {
        &self.address
    }
}

#[allow(dead_code)]
pub async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::warn!("signal received, starting graceful shutdown");
}
