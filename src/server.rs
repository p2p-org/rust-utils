use jsonrpsee::{
    core::Error,
    http_server::{AccessControl, AccessControlBuilder, HttpServer, HttpServerBuilder, HttpServerHandle},
    RpcModule,
};
use std::net::{SocketAddr, TcpListener};
use tokio::{net::ToSocketAddrs, signal, task::JoinHandle};

pub struct Server {
    address: SocketAddr,
    handle: HttpServerHandle,
}

impl Server {
    pub fn with_listener<Ctx>(
        listener: impl Into<TcpListener>,
        service: RpcModule<Ctx>,
        allow_all: bool,
    ) -> Result<Self, Error> {
        let server = Self::builder(allow_all).build_from_tcp(listener)?;
        Self::start(server, service)
    }

    pub async fn with_address<Ctx>(
        address: impl ToSocketAddrs,
        service: RpcModule<Ctx>,
        allow_all: bool,
    ) -> Result<Self, Error> {
        let server = Self::builder(allow_all).build(address).await?;
        Self::start(server, service)
    }

    pub async fn stop(self) -> Result<(), Error> {
        self.handle
            .stop()?
            .await
            .map_err(|error| Error::Custom(format!("server failed to stop: {error:?}")))
    }

    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(self.handle)
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

    fn builder(allow_all: bool) -> HttpServerBuilder {
        let acl = if allow_all {
            AccessControlBuilder::new()
                .allow_all_headers()
                .allow_all_origins()
                .allow_all_hosts()
                .build()
        } else {
            AccessControl::default()
        };

        HttpServerBuilder::default().set_access_control(acl)
    }

    fn start<Ctx>(server: HttpServer, service: RpcModule<Ctx>) -> Result<Self, Error> {
        Ok(Self {
            address: server.local_addr()?,
            handle: server.start(service)?,
        })
    }
}
