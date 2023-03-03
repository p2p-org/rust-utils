use anyhow::Context;
use async_trait::async_trait;
use backoff::ExponentialBackoff;
use lapin::{
    options::BasicPublishOptions, topology::TopologyDefinition, BasicProperties, Channel, Connection,
    ConnectionProperties,
};
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(feature = "telemetry")]
use lapin::types::FieldTable;
use serde::Serialize;
#[cfg(feature = "telemetry")]
use std::collections::BTreeMap;
#[cfg(feature = "telemetry")]
use tracing_opentelemetry::OpenTelemetrySpanExt;

#[async_trait]
pub trait MessagePublisher {
    async fn publish_payload(&self, exchange: &str, routing_key: &str, payload: &[u8]) -> anyhow::Result<()>;

    async fn publish<T>(&self, exchange: &str, routing_key: &str, message: &T) -> anyhow::Result<()>
    where
        T: Serialize + Sync,
    {
        self.publish_payload(exchange, routing_key, serde_json::to_vec(message)?.as_ref())
            .await
    }
}

#[derive(Clone)]
pub struct RabbitMessagePublisher {
    url: String,
    channel: Arc<RwLock<Channel>>,
    topology: TopologyDefinition,
}

#[cfg(not(feature = "telemetry"))]
#[async_trait]
impl MessagePublisher for RabbitMessagePublisher {
    async fn publish_payload(&self, exchange: &str, routing_key: &str, payload: &[u8]) -> anyhow::Result<()> {
        while self.basic_publish(exchange, routing_key, payload).await.is_err() {
            self.reconnect().await?;
        }
        Ok(())
    }
}

#[cfg(feature = "telemetry")]
#[async_trait]
impl MessagePublisher for RabbitMessagePublisher {
    #[tracing::instrument(skip(self))]
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

    #[cfg(not(feature = "telemetry"))]
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

    #[cfg(feature = "telemetry")]
    #[tracing::instrument(skip(self))]
    async fn basic_publish(&self, exchange: &str, routing_key: &str, payload: &[u8]) -> lapin::Result<()> {
        let mut amqp_headers = BTreeMap::new();

        // retrieve the current span
        let span = tracing::Span::current();
        // retrieve the current context
        let cx = span.context();
        // inject the current context through the amqp headers
        opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&cx, &mut AmqpClientCarrier::new(&mut amqp_headers))
        });

        let _ = self
            .channel
            .read()
            .await
            .basic_publish(
                exchange,
                routing_key,
                BasicPublishOptions::default(),
                payload,
                BasicProperties::default().with_headers(FieldTable::from(amqp_headers)),
            )
            .await?
            .await?;
        Ok(())
    }

    pub async fn purge(&self, queue: &str) -> anyhow::Result<()> {
        let guard = self.channel.read().await;
        guard
            .queue_purge(queue, lapin::options::QueuePurgeOptions::default())
            .await?;
        log::debug!("purge queues");
        Ok(())
    }
}

#[cfg(feature = "telemetry")]
mod telemetry {
    use lapin::types::{AMQPValue, ShortString};
    use opentelemetry::propagation::Injector;
    use std::collections::BTreeMap;

    pub(crate) struct AmqpClientCarrier<'a> {
        properties: &'a mut BTreeMap<ShortString, AMQPValue>,
    }

    impl<'a> AmqpClientCarrier<'a> {
        pub(crate) fn new(properties: &'a mut BTreeMap<ShortString, AMQPValue>) -> Self {
            Self { properties }
        }
    }

    impl<'a> Injector for AmqpClientCarrier<'a> {
        fn set(&mut self, key: &str, value: String) {
            self.properties.insert(key.into(), AMQPValue::LongString(value.into()));
        }
    }
}

#[cfg(feature = "telemetry")]
use telemetry::*;
