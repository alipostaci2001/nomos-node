mod command;
mod config;
mod mixnet;
mod swarm;

// std
pub use self::command::{Command, Libp2pInfo};
pub use self::config::Libp2pConfig;
use self::mixnet::MixnetHandler;
use self::swarm::SwarmHandler;

// internal
use super::NetworkBackend;
pub use nomos_libp2p::libp2p::gossipsub::{Message, TopicHash};
// crates
use overwatch_rs::{overwatch::handle::OverwatchHandle, services::state::NoState};
use tokio::sync::{broadcast, mpsc};

pub struct Libp2p {
    events_tx: broadcast::Sender<Event>,
    commands_tx: mpsc::Sender<Command>,
}

#[derive(Debug)]
pub enum EventKind {
    Message,
}

/// Events emitted from [`NomosLibp2p`], which users can subscribe
#[derive(Debug, Clone)]
pub enum Event {
    Message(Message),
}

const BUFFER_SIZE: usize = 64;

#[async_trait::async_trait]
impl NetworkBackend for Libp2p {
    type Settings = Libp2pConfig;
    type State = NoState<Libp2pConfig>;
    type Message = Command;
    type EventKind = EventKind;
    type NetworkEvent = Event;

    fn new(config: Self::Settings, overwatch_handle: OverwatchHandle) -> Self {
        let (commands_tx, commands_rx) = tokio::sync::mpsc::channel(BUFFER_SIZE);
        let (events_tx, _) = tokio::sync::broadcast::channel(BUFFER_SIZE);

        let mut mixnet_handler = MixnetHandler::new(&config, commands_tx.clone());
        overwatch_handle.runtime().spawn(async move {
            mixnet_handler.run().await;
        });

        let mut swarm_handler =
            SwarmHandler::new(&config, commands_tx.clone(), commands_rx, events_tx.clone());
        overwatch_handle.runtime().spawn(async move {
            swarm_handler.run(config.initial_peers).await;
        });

        Self {
            events_tx,
            commands_tx,
        }
    }

    async fn process(&self, msg: Self::Message) {
        if let Err(e) = self.commands_tx.send(msg).await {
            tracing::error!("failed to send command to nomos-libp2p: {e:?}");
        }
    }

    async fn subscribe(
        &mut self,
        kind: Self::EventKind,
    ) -> broadcast::Receiver<Self::NetworkEvent> {
        match kind {
            EventKind::Message => {
                tracing::debug!("processed subscription to incoming messages");
                self.events_tx.subscribe()
            }
        }
    }
}
