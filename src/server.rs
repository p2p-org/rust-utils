use axum_tracing_opentelemetry::opentelemetry_tracing_layer;
use jsonrpsee::{
    core::error::Error,
    server::{
        logger::Logger, middleware::proxy_get_request::ProxyGetRequestLayer, AllowHosts,
        ServerBuilder as RpcServerBuilder, ServerHandle,
    },
    Methods,
};
use std::{future::Future, net::SocketAddr};
use tokio::{net::ToSocketAddrs, signal, task::JoinHandle};
use tower::{
    layer::util::{Identity, Stack},
    Layer, Service, ServiceBuilder,
};
use tower_http::cors::CorsLayer;

pub struct Server {
    address: SocketAddr,
    handle: ServerHandle,
}

pub struct ServerBuilder<A, L> {
    address: A,
    middleware: ServiceBuilder<L>,
}

impl<A> ServerBuilder<A, Identity>
where
    A: ToSocketAddrs,
{
    pub fn new(address: A) -> Self {
        Self {
            address,
            middleware: ServiceBuilder::default(),
        }
    }
}

impl<A, L> ServerBuilder<A, L> {
    pub async fn build(self, service: impl Into<Methods>) -> Result<Server, Error> {
        let server = RpcServerBuilder::default()
            .set_host_filtering(AllowHosts::Any)
            .set_middleware(self.middleware)
            .http_only()
            .build(self.address)
            .await?;
        Ok(Server {
            address: server.local_addr()?,
            handle: server.start(service)?,
        })
    }

    pub async fn build_with_default_middleware(self, service: impl Into<Methods>) -> Result<Server, Error> {
        let service = service.into();
        self.map_middleware(|middleware| {
            middleware
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
                )
        })
        .build(service)
        .await
    }

    pub fn middleware<T>(self, middleware: ServiceBuilder<T>) -> ServerBuilder<A, T> {
        Self {
            address: self.address,
            middleware,
        }
    }

    pub fn map_middleware<F, T>(self, f: F) -> ServerBuilder<A, T>
    where
        F: FnOnce(ServiceBuilder<L>) -> ServiceBuilder<T>,
    {
        Self {
            address: self.address,
            middleware: f(self.middleware),
        }
    }

    pub fn add_get_route(
        self,
        path: impl Into<String>,
        method: impl Into<String>,
    ) -> ServerBuilder<A, Stack<ProxyGetRequestLayer, L>> {
        let mut path = path.into();
        if !path.starts_with("/") {
            path = format!("/{path}");
        }

        self.map_middleware(|middleware| {
            middleware.layer(ProxyGetRequestLayer::new(path, method).expect("Invalid URL"))
        })
    }
}

impl Server {
    /// Create and start a new server from TcpListener
    pub async fn with_address(address: impl ToSocketAddrs, service: impl Into<Methods>) -> Result<Self, Error> {
        ServerBuilder::new(address).build_with_default_middleware(service).await
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
