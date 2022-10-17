
use std::net::{SocketAddr, TcpListener};
use jsonrpsee::server::{AllowHosts, RpcModule, ServerBuilder, ServerHandle};
use jsonrpsee::core::error::Error;
use tokio::{net::ToSocketAddrs, signal};
use tower_http::cors::CorsLayer;

pub struct Server {
    address: SocketAddr,
    handle: ServerHandle,
}

impl Server {
    /// Create and start a new server from TcpListener
    pub fn with_listener<Ctx>(
        listener: impl Into<TcpListener>,
        service: RpcModule<Ctx>) -> Result<Self, Error>
    {
        let cors = CorsLayer::permissive();
        let middleware = tower::ServiceBuilder::new().layer(cors);

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
    pub async fn with_address<Ctx>(
        address: impl ToSocketAddrs,
        service: RpcModule<Ctx>,
    ) -> Result<Self, Error> {
        let cors = CorsLayer::permissive();
        let middleware = tower::ServiceBuilder::new().layer(cors);

        let server = ServerBuilder::default()
            .set_host_filtering(AllowHosts::Any)
            .set_middleware(middleware)
            .build(address).await?;

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


    pub async fn wait(self) {
        log::info!(
            "running server on http://{}, press Ctrl-C to terminate...",
            self.address
        );

        match signal::ctrl_c().await {
            Ok(_) => {
                log::info!("received Ctrl-C, terminating...");
            },
            Err(error) => {
                log::warn!("failed to wait for Ctrl-C: {error}, terminating...");
            },
        }

        match self.stop().await {
            Ok(_) => {
                log::info!("server stopped successfully");
            },
            Err(error) => {
                log::warn!("failed to stop the server: {error}");
            },
        }
    }

    pub fn address(&self) -> &SocketAddr {
        &self.address
    }
}
