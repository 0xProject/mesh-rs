//! # To do
//!
//! * Move OrderSync channel stuff to it's behaviour.

// How to handle external and internal events in parallel?
// See https://github.com/libp2p/rust-libp2p/issues/1876

// How to emit out events:
// See https://github.com/libp2p/rust-libp2p/issues/983
// See https://github.com/libp2p/rust-libp2p/issues/1021

mod behaviour;
mod transport;

use self::{
    behaviour::{order_sync, Behaviour},
    transport::make_transport,
};
use crate::prelude::*;
use futures::channel::{mpsc, oneshot};
use humansize::{file_size_opts::DECIMAL, FileSize};
use libp2p::{
    bandwidth::BandwidthSinks, core::network::NetworkInfo, gossipsub::Topic, identity,
    swarm::SwarmBuilder, Multiaddr, PeerId, Swarm,
};
use std::sync::Arc;

type OrderSyncRequest = (
    PeerId,
    order_sync::messages::Request,
    oneshot::Sender<order_sync::Result>,
);

/// TODO: Impl Debug
pub struct Node {
    bandwidth_monitor: Arc<BandwidthSinks>,
    swarm:             Swarm<Behaviour>,

    order_sync_sender:   mpsc::Sender<OrderSyncRequest>,
    order_sync_receiver: mpsc::Receiver<OrderSyncRequest>,
}

#[derive(Clone)]
pub struct OrderSyncRpc {
    sender: mpsc::Sender<OrderSyncRequest>,
}

impl OrderSyncRpc {
    pub async fn call(
        &mut self,
        peer_id: PeerId,
        request: order_sync::messages::Request,
    ) -> order_sync::Result {
        let (sender, receiver) = oneshot::channel();
        self.sender.send((peer_id, request, sender)).await?;
        receiver.await?
    }
}

impl Node {
    pub async fn new(peer_id_keys: identity::Keypair) -> Result<Self> {
        // Generate peer id
        let peer_id = PeerId::from(peer_id_keys.public());
        info!("Peer Id: {}", peer_id.clone());

        // Create a transport
        let (transport, bandwidth_monitor) =
            make_transport(peer_id_keys.clone()).context("Creating libp2p transport")?;

        // Create node behaviour
        let behaviour = Behaviour::new(peer_id_keys)
            .await
            .context("Creating node behaviour")?;

        // Executor for connection background tasks.
        let executor = Box::new(|future| {
            trace!("Spawning background task");
            tokio::spawn(future);
        });

        // Create a Swarm to manage peers and events.
        let swarm: Swarm<Behaviour> = SwarmBuilder::new(transport, behaviour, peer_id)
            .executor(executor)
            .build();

        // Create a channel for OrderSync requests
        let request_buffer_size = 16;
        let (order_sync_sender, order_sync_receiver) = mpsc::channel(request_buffer_size);

        Ok(Self {
            bandwidth_monitor,
            swarm,
            order_sync_sender,
            order_sync_receiver,
        })
    }

    pub fn start(&mut self) -> Result<()> {
        // Start behaviours
        self.swarm.start()?;

        // Listen on all interfaces and whatever port the OS assigns
        Swarm::listen_on(
            &mut self.swarm,
            "/ip4/0.0.0.0/tcp/0"
                .parse()
                .context("Parsing listening address")?,
        )
        .context("Starting to listen")?;

        Ok(())
    }

    /// Create a Send + Sync handle to the OrderSync RPC interface.
    pub fn order_sync_rpc(&self) -> OrderSyncRpc {
        OrderSyncRpc {
            sender: self.order_sync_sender.clone(),
        }
    }

    /// Drive the event loop forward
    pub async fn run(&mut self) -> Result<()> {
        let order_sync_request = tokio::select! {
            _ = self.swarm.next() => None,
            r = self.order_sync_receiver.next() => r,
        };
        if let Some((peer_id, request, sender)) = order_sync_request {
            self.swarm.order_sync_send(&peer_id, request, sender);
        }
        Ok(())
    }
}

// Pass-through accessors
impl Node {
    pub fn local_peer_id<'a>(&'a self) -> &'a PeerId {
        Swarm::local_peer_id(&self.swarm)
    }

    pub fn listeners<'a>(&'a self) -> impl Iterator<Item = &'a Multiaddr> {
        Swarm::listeners(&self.swarm)
    }

    pub fn network_info(&self) -> NetworkInfo {
        Swarm::network_info(&self.swarm)
    }

    pub fn total_inbound(&self) -> u64 {
        self.bandwidth_monitor.total_inbound()
    }

    pub fn total_outbound(&self) -> u64 {
        self.bandwidth_monitor.total_outbound()
    }
}

pub async fn run() -> Result<()> {
    let peer_id_keys = identity::Keypair::generate_ed25519();
    let mut node = Node::new(peer_id_keys).await.context("Creating node")?;
    node.start()?;

    let mut order_sync_rpc = node.order_sync_rpc();

    // Catch SIGTERM so the container can shutdown without an init process.
    let sigterm = tokio::signal::ctrl_c();
    tokio::pin!(sigterm);

    // Fetch orders from node
    let fetch = async {
        let peer_id = "16Uiu2HAkzEEu7Qpv8WK2XwpfiyrAdnJYDJWqd1Qz67RvLRe9veYv".parse()?;
        // TODO: We need to find and query this peer first.

        let request = order_sync::messages::Request::default();
        let response = order_sync_rpc.call(peer_id, request).await?;
        info!("Received response: {:#?}", response);

        anyhow::Result::<()>::Ok(())
    }
    .fuse();
    tokio::pin!(fetch);

    // Kick it off
    loop {
        tokio::select! {
            _ = node.run() => {
            },
            result = &mut fetch  => match result {
                Err(err) => error!("OrderSync fetch failed: {}", err),
                Ok(()) => info!("OrderSync fetch finished succesfully.")
            },
            _ = &mut sigterm => {
                info!("SIGTERM received, shutting down");
                // TODO: Shut down swarm?
                break;
            }
        }
    }

    // Log final stats
    info!("Network: {:?}", node.network_info());
    info!("Listening on: {:?}", node.listeners().collect::<Vec<_>>());
    info!(
        "Bandwidth: {} inbound, {} outbound",
        node.total_inbound().file_size(DECIMAL).unwrap(),
        node.total_outbound().file_size(DECIMAL).unwrap()
    );
    Ok(())
}
