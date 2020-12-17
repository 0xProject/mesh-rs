//! Stack of implemented protocols for the node.
//!
//! Implemented protocols:
//!
//! * `/ipfs/id/1.0.0`
//! * `/meshsub/1.0.0` (aka gossipsub)
//! * `/0x-mesh-dht/version/1` (aka kademlia)
//! * `/0x-mesh/order-sync/version/0`
//!
//! Missing protocols:
//!
//! * `/ipfs/id/push/1.0.0`
//! * `/p2p/id/delta/1.0.0`
//! * `/libp2p/circuit/relay/0.1.0
//! * `/floodsub/1.0.0`
//!
//! TODO: https://docs.rs/libp2p-observed-address/0.12.0/libp2p_observed_address/

use super::order_sync;
use crate::prelude::*;
use libp2p::{
    core::ProtocolName,
    gossipsub::{Gossipsub, GossipsubConfigBuilder, GossipsubEvent, MessageAuthenticity, Topic},
    identify::{Identify, IdentifyEvent, IdentifyInfo},
    identity::Keypair,
    kad::{
        record::store::MemoryStore, Kademlia, KademliaBucketInserts, KademliaConfig, KademliaEvent,
    },
    mdns::{Mdns, MdnsEvent},
    ping::{Ping, PingConfig, PingEvent},
    swarm::{NetworkBehaviour, NetworkBehaviourEventProcess},
    Multiaddr, NetworkBehaviour, PeerId,
};
use std::time::Duration;

const DHT_PROTOCOL_ID: &[u8] = b"/0x-mesh-dht/version/1";
const TOPIC: &str = "/0x-orders/version/3/chain/1/schema/e30=";
const BOOTNODES: &'static [(&str, &str)] = &[
    (
        "16Uiu2HAmGx8Z6gdq5T5AQE54GMtqDhDFhizywTy1o28NJbAMMumF",
        "/dns4/bootstrap-0.mesh.0x.org/tcp/60558",
    ),
    (
        "16Uiu2HAkwsDZk4LzXy2rnWANRsyBjB4fhjnsNeJmjgsBqxPGTL32",
        "/dns4/bootstrap-1.mesh.0x.org/tcp/60558",
    ),
    (
        "16Uiu2HAkykwoBxwyvoEbaEkuKMeKrmJDPZ2uKFPUKtqd2JbGHUNH",
        "/dns4/bootstrap-2.mesh.0x.org/tcp/60558",
    ),
];

#[derive(NetworkBehaviour)]
pub(crate) struct MyBehaviour {
    mdns:       Mdns,
    kademlia:   Kademlia<MemoryStore>,
    identify:   Identify,
    ping:       Ping,
    pubsub:     Gossipsub,
    order_sync: order_sync::Protocol,

    #[behaviour(ignore)]
    requesting: bool,
}

impl NetworkBehaviourEventProcess<MdnsEvent> for MyBehaviour {
    // Called when `mdns` produces an event.
    fn inject_event(&mut self, event: MdnsEvent) {
        debug!("Mdns: {:?}", event);
    }
}

impl NetworkBehaviourEventProcess<KademliaEvent> for MyBehaviour {
    /// Called when `kademlia` produces and event.
    fn inject_event(&mut self, event: KademliaEvent) {
        use KademliaEvent::*;
        debug!("Kademlia: {:?}", event);
        match event {
            QueryResult { .. } => {
                // Search another peer
                self.search_random_peer();
            }
            _ => {}
        }
    }
}

impl NetworkBehaviourEventProcess<IdentifyEvent> for MyBehaviour {
    /// Called when `identify` produces and event.
    fn inject_event(&mut self, event: IdentifyEvent) {
        use IdentifyEvent::*;
        match event {
            Received { info, .. } => {
                self.upsert_peer_info(info);
            }
            Sent { peer_id } => {
                trace!("Identifying information sent to {}", peer_id);
            }
            Error { peer_id, error } => {
                error!("Identify error on peer {}: {}", peer_id, error);
            }
        }
    }
}

impl NetworkBehaviourEventProcess<PingEvent> for MyBehaviour {
    /// Called when `ping` produces and event.
    fn inject_event(&mut self, event: PingEvent) {
        trace!("Ping: {:?}", event);
    }
}

impl NetworkBehaviourEventProcess<GossipsubEvent> for MyBehaviour {
    /// Called when `gossipsub` produces and event.
    fn inject_event(&mut self, event: GossipsubEvent) {
        match event {
            GossipsubEvent::Message(peer_id, id, message) => {
                trace!(
                    "Got message: {} with id: {} from peer: {:?}",
                    String::from_utf8_lossy(&message.data),
                    id,
                    peer_id
                );
            }
            event => debug!("Gossipsub: {:?}", event),
        }
    }
}

impl NetworkBehaviourEventProcess<order_sync::Event> for MyBehaviour {
    /// Called when `identify` produces and event.
    fn inject_event(&mut self, event: order_sync::Event) {
        warn!("OrderSync event: {:?}", event);
    }
}

impl MyBehaviour {
    pub(crate) async fn new(peer_key: Keypair) -> Result<Self> {
        let public_key = peer_key.public();
        let peer_id = PeerId::from_public_key(public_key.clone());

        // Mdns LAN node discovery
        let mdns = Mdns::new()
            .await
            .context("Creating mDNS node discovery behaviour")?;

        // Kademlia for 0x Mesh peer discovery
        let mut kad_config = KademliaConfig::default();
        kad_config.set_protocol_name(DHT_PROTOCOL_ID);
        kad_config.set_kbucket_inserts(KademliaBucketInserts::OnConnected);
        kad_config.set_query_timeout(Duration::from_secs(5));
        debug!("Kademlia config: {:?}", &kad_config);
        let kad_store = MemoryStore::new(peer_id.clone());
        let mut kademlia = Kademlia::with_config(peer_id.clone(), kad_store, kad_config);

        // Add bootnodes
        for (peer_id, multiaddr) in BOOTNODES {
            let peer_id = peer_id.parse().context("Parsing bootnode peer id")?;
            let multiaddr = multiaddr.parse().context("Parsing bootnode address")?;
            kademlia.add_address(&peer_id, multiaddr);
        }

        // Join DHT
        let bootstrap = kademlia.bootstrap().context("Joining Kademlia DHT")?;
        info!("Kademlia Bootstrap query {:?}", bootstrap);

        // Identify protocol
        let identify = Identify::new("/ipfs/0.1.0".into(), "mesh-rs".into(), public_key);

        // Ping protocol
        let ping = Ping::new(PingConfig::new());

        // GossipSub
        let gossipsub_config = GossipsubConfigBuilder::new()
            .max_transmit_size(262144)
            .build();
        let mut pubsub = Gossipsub::new(
            MessageAuthenticity::Signed(peer_key.clone()),
            gossipsub_config,
        );

        // Subscribe to orders
        let topic = Topic::new(TOPIC.into());
        pubsub.subscribe(topic);

        // OrderSync protocol versions
        let order_sync_config = order_sync::Config::default();
        let order_sync = order_sync::new(order_sync_config);

        let mut behaviour = MyBehaviour {
            mdns,
            kademlia,
            identify,
            ping,
            pubsub,
            order_sync,
            requesting: false,
        };
        Ok(behaviour)
    }

    fn upsert_peer_info(&mut self, peer_info: IdentifyInfo) {
        info!("Learned about peer {:?}", peer_info);
        let peer_id = peer_info.public_key.into_peer_id();
        if peer_info
            .protocols
            .contains(&String::from_utf8_lossy(order_sync::Version().protocol_name()).to_string())
        {
            // Node supports order sync protocol
            if !self.requesting {
                // Request only once, and from the first peer we see.
                self.requesting = true;
                self.get_orders(peer_id).unwrap();
            }
        }
        // TODO: Store
    }

    pub(crate) fn search_random_peer(&mut self) {
        // It's not the query that matters, it's the friends we make along the way.
        let query: PeerId = Keypair::generate_ed25519().public().into();
        info!("Searching for random peer {:?} query", &query);
        let query_id = self.kademlia.get_closest_peers(query.clone());
        debug!("Query {:?} {:?}", query_id, query);
    }

    pub(crate) fn known_peers(&mut self) -> Vec<(PeerId, Vec<Multiaddr>)> {
        let mut result = Vec::default();
        for bucket in self.kademlia.kbuckets() {
            for entry in bucket.iter() {
                let peer_id = entry.node.key.preimage();
                let addresses = entry.node.value.iter().cloned().collect::<Vec<_>>();
                result.push((peer_id.clone(), addresses));
            }
        }
        result
    }

    /// GetOrders iterates through every peer the node is currently connected to
    /// and attempts to perform the ordersync protocol. It keeps trying until
    /// ordersync has been completed with minPeers, using an exponential backoff
    /// strategy between retries.
    pub(crate) fn get_orders(&mut self, peer: PeerId) -> Result<()> {
        let request = order_sync::Request::from(order_sync::OrderFilter::mainnet_v2());
        let id = self.order_sync.send_request(&peer, request);
        info!("Req({})", id);
        Ok(())
    }

    pub(crate) async fn get_identity(&mut self) -> Result<()> {
        Ok(())
    }
}
