use anyhow::{Context, Result};
use libp2p::{
    gossipsub::{Gossipsub, GossipsubConfigBuilder, GossipsubEvent, MessageAuthenticity, Topic},
    identify::{Identify, IdentifyEvent},
    identity::Keypair,
    kad::{
        record::{
            store::{MemoryStore, RecordStore},
            Record,
        },
        Kademlia, KademliaBucketInserts, KademliaConfig, KademliaEvent,
    },
    mdns::{Mdns, MdnsEvent},
    ping::{Ping, PingConfig, PingEvent},
    swarm::NetworkBehaviourEventProcess,
    Multiaddr, NetworkBehaviour, PeerId,
};
use log::{debug, info};
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

// protocols: ["/ipfs/id/1.0.0", "/ipfs/id/push/1.0.0", "/p2p/id/delta/1.0.0",
// "/ipfs/ping/1.0.0", "/libp2p/circuit/relay/0.1.0", "/0x-mesh-dht/version/1",
// "/libp2p/autonat/1.0.0"]

// We create a custom network behaviour that combines floodsub and mDNS.
// The derive generates a delegating `NetworkBehaviour` impl which in turn
// requires the implementations of `NetworkBehaviourEventProcess` for
// the events of each behaviour.
#[derive(NetworkBehaviour)]
pub(crate) struct MyBehaviour {
    mdns:     Mdns,
    kademlia: Kademlia<MemoryStore>,
    identify: Identify,
    ping:     Ping,
    pubsub:   Gossipsub,
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
        debug!("Kademlia: {:?}", event);
    }
}

impl NetworkBehaviourEventProcess<IdentifyEvent> for MyBehaviour {
    /// Called when `identify` produces and event.
    fn inject_event(&mut self, event: IdentifyEvent) {
        debug!("Identify: {:?}", event);
    }
}

impl NetworkBehaviourEventProcess<PingEvent> for MyBehaviour {
    /// Called when `identify` produces and event.
    fn inject_event(&mut self, event: PingEvent) {
        debug!("Ping: {:?}", event);
    }
}

impl NetworkBehaviourEventProcess<GossipsubEvent> for MyBehaviour {
    /// Called when `identify` produces and event.
    fn inject_event(&mut self, event: GossipsubEvent) {
        match event {
            GossipsubEvent::Message(peer_id, id, message) => {
                info!(
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

        // Pin protocol
        let ping = Ping::new(PingConfig::new());

        // GossipSub
        let topic = Topic::new(TOPIC.into());
        let gossipsub_config = GossipsubConfigBuilder::new()
            .max_transmit_size(262144)
            .build();
        let mut pubsub = Gossipsub::new(
            MessageAuthenticity::Signed(peer_key.clone()),
            gossipsub_config,
        );
        pubsub.subscribe(topic);

        let mut behaviour = MyBehaviour {
            mdns,
            kademlia,
            identify,
            ping,
            pubsub,
        };
        Ok(behaviour)
    }

    pub(crate) fn search_random_peer(&mut self) {
        let query: PeerId = Keypair::generate_ed25519().public().into();
        // It's not the query that matters, it's the friends we make along the way.
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
}
