use anyhow::Context;
use async_trait::async_trait;
use backoff::{future::retry_notify, ExponentialBackoff};

use futures::prelude::*;
use lapin::{
    message::Delivery,
    options::{BasicCancelOptions, BasicNackOptions},
    topology::{RestoredTopology, TopologyDefinition},
    types::DeliveryTag,
    Channel, Connection, ConnectionProperties, Consumer, ConsumerState,
};

use stream_cancel::{StreamExt, Trigger, Tripwire};

pub trait CancelConsume {
    fn cancel(self);
}

pub trait MessageConsumer<MsgProcessor> {
    type Cancellation: CancelConsume;
    fn try_connect_and_consume(
        url: &str,
        topology_definition: TopologyDefinition,
        processor: MsgProcessor,
        consumer_tag: &'static str,
    ) -> Self::Cancellation;
}

#[async_trait]
pub trait MessageProcessor {
    async fn process_message(&self, delivery: &Delivery, channel: &Channel) -> anyhow::Result<()>;
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
        consumer_tag: &'static str,
    ) -> Self::Cancellation {
        let (trigger, tripwire) = Tripwire::new();

        RabbitMessageConsumer {
            url: url.to_owned(),
            topology_definition,
            processor,
            tripwire,
        }
        .try_connect_and_consume_core(consumer_tag);

        Self::Cancellation { trigger }
    }
}

impl<MsgProcessor: MessageProcessor + Clone + Send + Sync + 'static> RabbitMessageConsumer<MsgProcessor> {
    fn try_connect_and_consume_core(self, consumer_tag: &'static str) {
        tokio::spawn(async move {
            log::trace!("try connect and consume");
            let retry_status = retry_notify(
                ExponentialBackoff::default(),
                || async { self.clone().connect_and_consume(consumer_tag).await.map_err(Into::into) },
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

    async fn connect_and_consume(self, consumer_tag: &str) -> anyhow::Result<()> {
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
            log::trace!("received message {}", delivery.delivery_tag);

            // actual message handler should send ack/nack
            if let Err(err) = processor.process_message(&delivery, &channel).await {
                // here we will send nack for failed message processing (e.g. can't deserialize, can't send through tx,
                // etc)
                log::warn!("Failed to process message: {err}");
                delivery
                    .nack(BasicNackOptions::default())
                    .await
                    .context("Failed to nask rabbitmq msg")?;
            }
        }

        // Consumer will be cancelled on error, otherwise cancellation trigger
        // has been fired and it has to be cancelled by hand
        let channel = Self::channel(&topology);
        if Self::consumer(&topology).state() != ConsumerState::Canceled {
            channel
                .basic_cancel(consumer_tag, BasicCancelOptions::default())
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
