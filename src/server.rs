use axum_tracing_opentelemetry::opentelemetry_tracing_layer;
use jsonrpsee::{
    core::error::Error,
    server::{AllowHosts, RpcModule, ServerBuilder, ServerHandle},
};
use std::{
    future::Future,
    net::{SocketAddr, TcpListener},
};
use tokio::{net::ToSocketAddrs, signal};
use tower_http::cors::CorsLayer;

pub struct Server {
    address: SocketAddr,
    handle: ServerHandle,
}

impl Server {
    /// Create and start a new server from TcpListener
    pub fn with_listener<Ctx>(listener: impl Into<TcpListener>, service: RpcModule<Ctx>) -> Result<Self, Error> {
        let middleware = tower::ServiceBuilder::new()
            .layer(opentelemetry_tracing_layer())
            .layer(CorsLayer::permissive());

        let server = ServerBuilder::default()
            .set_host_filtering(AllowHosts::Any)
            .set_middleware(middleware)
            .build_from_tcp(listener)?;

        Ok(Self {
            address: server.local_addr()?,
            handle: server.start(service)?,
        })
    }

    /// Create and start a new server from TcpListener
    pub async fn with_address<Ctx>(address: impl ToSocketAddrs, service: RpcModule<Ctx>) -> Result<Self, Error> {
        let cors = CorsLayer::permissive();
        let middleware = tower::ServiceBuilder::new()
            .layer(opentelemetry_tracing_layer())
            .layer(cors);

        let server = ServerBuilder::default()
            .set_host_filtering(AllowHosts::Any)
            .set_middleware(middleware)
            .build(address)
            .await?;

        Ok(Self {
            address: server.local_addr()?,
            handle: server.start(service)?,
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
