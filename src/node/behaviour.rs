use anyhow::{Context, Result};
use libp2p::{
    gossipsub::{Gossipsub, GossipsubConfigBuilder, GossipsubEvent, MessageAuthenticity},
    identify::{Identify, IdentifyEvent},
    identity::Keypair,
    kad::{record::store::MemoryStore, Kademlia, KademliaConfig, KademliaEvent},
    mdns::{Mdns, MdnsEvent},
    ping::{Ping, PingConfig, PingEvent},
    swarm::NetworkBehaviourEventProcess,
    NetworkBehaviour, PeerId,
};
use log::{debug, info};
use std::time::Duration;

const DHT_PROTOCOL_ID: &[u8] = b"/0x-mesh-dht/version/1";

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
    // TODO: Ping
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
        debug!("Gossipsub: {:?}", event);
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
        kad_config.set_query_timeout(Duration::from_secs(5));
        debug!("Kademlia config: {:?}", &kad_config);
        let kad_store = MemoryStore::new(peer_id.clone());
        let kademlia = Kademlia::with_config(peer_id.clone(), kad_store, kad_config);

        // Identify protocol
        let identify = Identify::new("/ipfs/0.1.0".into(), "mesh-rs".into(), public_key);

        // Pin protocol
        let ping = Ping::new(PingConfig::new());

        // GossipSub
        let gossipsub_config = GossipsubConfigBuilder::new()
            .max_transmit_size(262144)
            .build();
        let pubsub = Gossipsub::new(
            MessageAuthenticity::Signed(peer_key.clone()),
            gossipsub_config,
        );

        let mut behaviour = MyBehaviour {
            mdns,
            kademlia,
            identify,
            ping,
            pubsub,
        };
        Ok(behaviour)
    }
}
