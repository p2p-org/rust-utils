use anyhow::Context;
use async_trait::async_trait;
use backoff::ExponentialBackoff;
use borsh::BorshSerialize;
use lapin::{
    options::BasicPublishOptions, topology::TopologyDefinition, BasicProperties, Channel, Connection,
    ConnectionProperties,
};
use std::sync::Arc;
use tokio::sync::RwLock;

#[async_trait]
pub trait MessagePublisher {
    async fn publish_payload(&self, exchange: &str, routing_key: &str, payload: &[u8]) -> anyhow::Result<()>;

    async fn publish<T>(&self, exchange: &str, routing_key: &str, message: &T) -> anyhow::Result<()>
    where
        T: BorshSerialize + Sync,
    {
        self.publish_payload(exchange, routing_key, borsh::to_vec(message)?.as_ref())
            .await
    }
}

#[derive(Clone)]
pub struct RabbitMessagePublisher {
    url: String,
    channel: Arc<RwLock<Channel>>,
    topology: TopologyDefinition,
}

#[async_trait]
impl MessagePublisher for RabbitMessagePublisher {
    async fn publish_payload(&self, exchange: &str, routing_key: &str, payload: &[u8]) -> anyhow::Result<()> {
        while self.basic_publish(exchange, routing_key, payload).await.is_err() {
            self.reconnect().await?;
        }
        Ok(())
    }
}

impl RabbitMessagePublisher {
    pub async fn try_connect(url: &str, topology: &TopologyDefinition) -> anyhow::Result<Self> {
        Self::connect(url, topology)
            .await
            .map(|channel| Self {
                url: url.to_owned(),
                channel: Arc::new(RwLock::new(channel)),
                topology: topology.clone(),
            })
            .context("failed to connect")
    }

    async fn connect(url: &str, topology: &TopologyDefinition) -> lapin::Result<Channel> {
        let options = ConnectionProperties::default()
            // Use tokio executor and reactor.
            // At the moment the reactor is only available for unix.
            .with_executor(tokio_executor_trait::Tokio::current());

        #[cfg(unix)]
        let options = options.with_reactor(tokio_reactor_trait::Tokio);

        let connection = Connection::connect(url, options).await?;
        log::trace!("Connected to rabbitmq");

        let _ = connection.restore(topology.clone()).await.map_err(|error| {
            log::warn!("Failed to restore topology: {error:?}");
            error
        })?;
        log::trace!("Restored topology");

        connection.create_channel().await
    }

    fn topology_definition(topology: &[u8]) -> TopologyDefinition {
        serde_json::from_slice(topology).expect("can't read and deserialize topology")
    }

    pub fn publisher_topology(topology: &[u8]) -> TopologyDefinition {
        TopologyDefinition {
            channels: vec![],
            ..Self::topology_definition(topology)
        }
    }

    async fn reconnect(&self) -> lapin::Result<()> {
        let channel = backoff::future::retry(ExponentialBackoff::default(), || async {
            let channel = Self::connect(&self.url, &self.topology).await?;
            Ok(channel)
        })
        .await?;

        let mut channel_guard = self.channel.write().await;
        *channel_guard = channel;

        Ok(())
    }

    async fn basic_publish(&self, exchange: &str, routing_key: &str, payload: &[u8]) -> lapin::Result<()> {
        let _ = self
            .channel
            .read()
            .await
            .basic_publish(
                exchange,
                routing_key,
                BasicPublishOptions::default(),
                payload,
                BasicProperties::default(),
            )
            .await?
            .await?;
        Ok(())
    }

    pub async fn purge(&self) -> anyhow::Result<()> {
        let guard = self.channel.read().await;
        guard
            .queue_purge("notifications", lapin::options::QueuePurgeOptions::default())
            .await?;
        guard
            .queue_purge("users", lapin::options::QueuePurgeOptions::default())
            .await?;
        log::debug!("purge queues");
        Ok(())
    }
}
