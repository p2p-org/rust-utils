use anyhow::Context;
use async_trait::async_trait;
use backoff::{future::retry_notify, ExponentialBackoff};

use futures::prelude::*;
use lapin::{
    message::Delivery,
    options::BasicCancelOptions,
    topology::{RestoredTopology, TopologyDefinition},
    types::DeliveryTag,
    Channel, Connection, ConnectionProperties, Consumer, ConsumerState,
};
use serde::de::DeserializeOwned;

use stream_cancel::{StreamExt, Trigger, Tripwire};
#[cfg(feature = "telemetry")]
use tracing::Instrument;

#[derive(Debug, Clone, Copy)]
pub struct PermanentError;

pub type AutoAck = bool;

impl std::fmt::Display for PermanentError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("PermanentError")
    }
}

pub trait CancelConsume {
    fn cancel(self);
}

pub trait MessageConsumer<MsgProcessor> {
    type Cancellation: CancelConsume;
    fn try_connect_and_consume(
        url: &str,
        topology_definition: TopologyDefinition,
        processor: MsgProcessor,
    ) -> Self::Cancellation;
}

#[async_trait]
pub trait MessageProcessor {
    async fn process_message(&self, delivery: &Delivery, channel: &Channel) -> anyhow::Result<AutoAck>;
}

#[async_trait]
pub trait MessageHandler {
    type Message;
    const ROUTING_KEY: Option<&'static str> = None;
    async fn handle_message(&self, message: Self::Message) -> anyhow::Result<()>;
}

#[cfg(not(feature = "telemetry"))]
macro_rules! tagged_warn {
    (tag = $tag:expr; $($arg:tt)*) => {
        log::warn!($($arg)*)
    };
}
#[cfg(feature = "telemetry")]
macro_rules! tagged_warn {
    (tag = $tag:expr; $($arg:tt)*) => {
        tracing::warn!(delivery_tag = %$tag, $($arg)*)
    };
}

#[async_trait]
impl<T> MessageProcessor for T
where
    T: MessageHandler + Send + Sync + 'static,
    T::Message: DeserializeOwned + Send + Sync + 'static,
{
    async fn process_message(&self, delivery: &Delivery, _channel: &Channel) -> anyhow::Result<AutoAck> {
        if let Some(routing_key) = Self::ROUTING_KEY {
            if delivery.routing_key.as_str() != routing_key {
                tagged_warn!(tag = delivery.delivery_tag; "Unsupported routing key {}", delivery.routing_key);
                return Ok(true);
            }
        }

        let message = serde_json::from_slice::<T::Message>(delivery.data.as_ref()).map_err(|error| {
            tagged_warn!(tag = delivery.delivery_tag; "Failed to deserialize message: {error:?}");
            error
        })?;
        self.handle_message(message).await.map_err(|error| {
            tagged_warn!(tag = delivery.delivery_tag; "Failed to handle message: {error:?}");
            error
        })?;
        Ok(true)
    }
}

pub struct RabbitConsumerCancellation {
    trigger: Trigger,
}

impl CancelConsume for RabbitConsumerCancellation {
    fn cancel(self) {
        self.trigger.cancel();
    }
}

#[derive(Clone)]
pub struct RabbitMessageConsumer<MsgProcessor> {
    url: String,
    topology_definition: TopologyDefinition,
    processor: MsgProcessor,
    tripwire: Tripwire,
}

impl<MsgProcessor: MessageProcessor + Clone + Send + Sync + 'static> MessageConsumer<MsgProcessor>
    for RabbitMessageConsumer<MsgProcessor>
{
    type Cancellation = RabbitConsumerCancellation;

    fn try_connect_and_consume(
        url: &str,
        topology_definition: TopologyDefinition,
        processor: MsgProcessor,
    ) -> Self::Cancellation {
        let (trigger, tripwire) = Tripwire::new();

        RabbitMessageConsumer {
            url: url.to_owned(),
            topology_definition,
            processor,
            tripwire,
        }
        .try_connect_and_consume_core();

        Self::Cancellation { trigger }
    }
}

impl<MsgProcessor: MessageProcessor + Clone + Send + Sync + 'static> RabbitMessageConsumer<MsgProcessor> {
    fn try_connect_and_consume_core(self) {
        tokio::spawn(async move {
            log::trace!("try connect and consume");
            let retry_status = retry_notify(
                ExponentialBackoff::default(),
                || async { self.clone().connect_and_consume().await.map_err(Into::into) },
                |err, duration| {
                    log::warn!("failed to connect and consume: {err:?}, retrying in {duration:?}");
                },
            )
            .await;
            if let Err(err) = retry_status {
                log::warn!("Reconnect logic failed: {err}");
            }
        });
    }

    async fn connect_and_consume(self) -> anyhow::Result<()> {
        let RabbitMessageConsumer {
            url,
            topology_definition,
            processor,
            tripwire,
        } = self;

        let options = ConnectionProperties::default()
            // Use tokio executor and reactor.
            // At the moment the reactor is only available for unix.
            .with_executor(tokio_executor_trait::Tokio::current());

        #[cfg(unix)]
        let options = options.with_reactor(tokio_reactor_trait::Tokio);

        let connection = Connection::connect(&url, options)
            .await
            .context("Failed to connect to rabbitmq")?;
        log::trace!("Connected to rabbitmq");

        let topology = connection
            .restore(topology_definition)
            .await
            .context("Failed to restore topology")?;

        let mut consumer = Self::consumer(&topology).take_until_if(tripwire);
        let channel = Self::channel(&topology);

        while let Some(delivery) = consumer.next().await {
            let delivery = delivery.context("Failed to receive message from consumer")?;

            #[cfg(feature = "telemetry")]
            let (delivery, span) = {
                let span = tracing::info_span!("process_message", delivery = %delivery.delivery_tag);
                (span.in_scope(|| correlate_trace_from_delivery(delivery)), span)
            };

            #[cfg(not(feature = "telemetry"))]
            let ack = {
                log::trace!("received message {}", delivery.delivery_tag);

                // actual message handler should return non-permanent error if it wants to nack message
                match processor.process_message(&delivery, &channel).await {
                    Ok(true) => true,
                    Ok(false) => continue,
                    Err(error) => {
                        // here we will send nack for failed message processing (e.g. can't deserialize, can't send
                        // through tx, etc)
                        log::warn!("Failed to process message: {error}");
                        error.is::<PermanentError>()
                    },
                }
            };

            #[cfg(feature = "telemetry")]
            let ack = {
                // actual message handler should return non-permanent error if it wants to nack message
                match processor
                    .process_message(&delivery, &channel)
                    .instrument(span.clone())
                    .await
                {
                    Ok(true) => true,
                    Ok(false) => continue,
                    Err(error) => {
                        // here we will send nack for failed message processing (e.g. can't deserialize, can't send
                        // through tx, etc)
                        tracing::warn!(parent: &span, error = ?error, delivery_tag = %delivery.delivery_tag, "Failed to process message");
                        error.is::<PermanentError>()
                    },
                }
            };

            if ack {
                delivery
                    .ack(Default::default())
                    .await
                    .context("Failed to ack rabbitmq msg")?;
            } else {
                delivery
                    .nack(Default::default())
                    .await
                    .context("Failed to nack rabbitmq msg")?;
            }
        }

        // Consumer will be cancelled on error, otherwise cancellation trigger
        // has been fired and it has to be cancelled by hand
        let channel = Self::channel(&topology);
        let consumer = Self::consumer(&topology);
        if consumer.state() != ConsumerState::Canceled {
            channel
                .basic_cancel(consumer.tag().as_str(), BasicCancelOptions::default())
                .await
                .context("Failed to cancel rabbitmq consumer")?;
        }

        log::info!("Have received close request (cancellation trigger)");

        Ok(())
    }

    fn consumer(topology: &RestoredTopology) -> Consumer {
        topology.channel(0).consumer(0)
    }

    fn channel(topology: &RestoredTopology) -> Channel {
        topology.channel(0).into_inner()
    }
}

#[derive(Debug)]
pub struct Ackable {
    delivery_tag: DeliveryTag,
    channel: Channel,
}

#[async_trait]
pub trait ManualAck {
    async fn ack(&self) -> anyhow::Result<()>;
    async fn nack(&self) -> anyhow::Result<()>;
}

impl Ackable {
    pub fn new(delivery: &Delivery, channel: Channel) -> Self {
        Self {
            delivery_tag: delivery.delivery_tag,
            channel,
        }
    }
}

#[async_trait]
impl ManualAck for Ackable {
    async fn ack(&self) -> anyhow::Result<()> {
        self.channel.basic_ack(self.delivery_tag, Default::default()).await?;
        Ok(())
    }

    async fn nack(&self) -> anyhow::Result<()> {
        self.channel.basic_nack(self.delivery_tag, Default::default()).await?;
        Ok(())
    }
}

#[async_trait]
impl ManualAck for Option<Ackable> {
    async fn ack(&self) -> anyhow::Result<()> {
        match self {
            Some(ackable) => ackable.ack().await,
            None => Ok(()),
        }
    }

    async fn nack(&self) -> anyhow::Result<()> {
        match self {
            Some(ackable) => ackable.nack().await,
            None => Ok(()),
        }
    }
}

#[cfg(feature = "telemetry")]
mod telemetry {
    use lapin::{
        message::Delivery,
        types::{AMQPValue, ShortString},
    };
    use opentelemetry::propagation::Extractor;
    use std::collections::BTreeMap;
    use tracing::Span;
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    pub(crate) struct AmqpHeaderCarrier<'a> {
        headers: &'a BTreeMap<ShortString, AMQPValue>,
    }

    impl<'a> AmqpHeaderCarrier<'a> {
        pub(crate) fn new(headers: &'a BTreeMap<ShortString, AMQPValue>) -> Self {
            Self { headers }
        }
    }

    impl<'a> Extractor for AmqpHeaderCarrier<'a> {
        fn get(&self, key: &str) -> Option<&str> {
            self.headers.get(key).and_then(|header_value| {
                if let AMQPValue::LongString(header_value) = header_value {
                    std::str::from_utf8(header_value.as_bytes())
                        .map_err(|e| tracing::error!("Error decoding header value {:?}", e))
                        .ok()
                } else {
                    tracing::warn!("Missing amqp tracing context propagation");
                    None
                }
            })
        }

        fn keys(&self) -> Vec<&str> {
            self.headers.keys().map(|header| header.as_str()).collect()
        }
    }

    pub fn correlate_trace_from_delivery(delivery: Delivery) -> Delivery {
        let span = Span::current();

        let headers = &delivery
            .properties
            .headers()
            .clone()
            .unwrap_or_default()
            .inner()
            .clone();
        let parent_cx = opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.extract(&AmqpHeaderCarrier::new(headers))
        });

        span.set_parent(parent_cx);

        delivery
    }
}

#[cfg(feature = "telemetry")]
use telemetry::*;
