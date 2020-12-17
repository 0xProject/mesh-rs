mod behaviour;
mod transport;

use self::{behaviour::Behaviour, transport::make_transport};
use crate::prelude::*;
use libp2p::{
    bandwidth::BandwidthSinks, core::network::NetworkInfo, gossipsub::Topic, identity,
    swarm::SwarmBuilder, Multiaddr, PeerId, Swarm,
};
use std::{sync::Arc};

const TOPIC: &str = "/0x-orders/version/3/chain/1/schema/e30=";

pub struct Node {
    bandwidth_monitor: Arc<BandwidthSinks>,
    swarm:             Swarm<Behaviour>,
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

        Ok(Self {
            bandwidth_monitor,
            swarm,
        })
    }

    pub fn start(&mut self) -> Result<()> {
        // Join DHT
        // let bootstrap = self
        //     .swarm
        //     .kademlia
        //     .bootstrap()
        //     .context("Joining Kademlia DHT")?;
        // info!("Kademlia Bootstrap query {:?}", bootstrap);

        // Subscribe to orders
        let _topic = Topic::new(TOPIC.into());
        // self.swarm.pubsub.subscribe(topic);

        // Listen on all interfaces and whatever port the OS assigns
        Swarm::listen_on(
            &mut self.swarm,
            "/ip4/0.0.0.0/tcp/0"
                .parse()
                .context("Parsing listening address")?,
        )
        .context("Starting to listen")?;

        // Start searching for random nodes
        // TODO: self.swarm.search_random_peer();

        Ok(())
    }

    /// Drive the event loop forward
    pub async fn run(&mut self) -> Result<()> {
        tokio::select! {
            _ = self.swarm.next() => Ok(()),
        }
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

    // Catch SIGTERM so the container can shutdown without an init process.
    let sigterm = tokio::signal::ctrl_c();
    tokio::pin!(sigterm);

    // Kick it off
    loop {
        tokio::select! {
            _ = node.run() => {
            },
            _ = &mut sigterm => {
                info!("SIGTERM received, shutting down");
                // TODO: Shut down swarm?
                break;
            }
        }
    }
    info!("Network: {:?}", node.network_info());
    info!("Done.");
    info!(
        "Bandwidth: {} inbound, {} outbound",
        node.total_inbound(),
        node.total_outbound()
    );
    info!("Orders saved");
    Ok(())
}
