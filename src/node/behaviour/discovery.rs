//! Behaviour for learning about other nodes
//!
//! Compare with the [`Discovery`][sub:discovery] and
//! [`PeerInfo`][sub:peer_info] behvaiours in Parity Substrate.
//
//! [sub:discovery]: https://github.com/paritytech/substrate/blob/6b600cdeb4043e512bc5f342eb02a5a17d26797a/client/network/src/discovery.rs
//! [sub:peer_info]: https://github.com/paritytech/substrate/blob/6b600cdeb4043e512bc5f342eb02a5a17d26797a/client/network/src/peer_info.rs
//!
//! ## To do
//!
//! * Accessor methods for known peers.
//! * Periodically initiate random Kademlia searches.
//! * Persistently store known peers for quick restart.
//! * Distinguish between local and global addresses, only feed global ones to
//!   DHT.

use crate::prelude::*;
use libp2p::{
    identify::{Identify, IdentifyEvent},
    identity::Keypair,
    kad::{
        record::store::MemoryStore, Kademlia, KademliaBucketInserts, KademliaConfig, KademliaEvent,
    },
    mdns::{Mdns, MdnsEvent},
    ping::{Ping, PingConfig, PingEvent},
    swarm::NetworkBehaviourEventProcess,
    Multiaddr, NetworkBehaviour, PeerId,
};
use std::time::Duration;

const DHT_PROTOCOL_ID: &[u8] = b"/0x-mesh-dht/version/1";
const BOOTNODES: &[(&str, &str)] = &[
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

pub struct DiscoveryConfig {
    peer_key:          Keypair,
    dht_protocol_name: String,
    bootnodes:         Vec<(PeerId, Multiaddr)>,
}

#[derive(NetworkBehaviour)]
pub struct Discovery {
    mdns:     Mdns,
    kademlia: Kademlia<MemoryStore>,
    identify: Identify,
    ping:     Ping,
}

impl Discovery {
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

        // Identify protocol
        let identify = Identify::new("/ipfs/0.1.0".into(), "mesh-rs".into(), public_key);

        // Ping protocol
        let ping = Ping::new(PingConfig::new());

        Ok(Self {
            mdns,
            kademlia,
            identify,
            ping,
        })
    }
}

impl NetworkBehaviourEventProcess<MdnsEvent> for Discovery {
    fn inject_event(&mut self, _event: MdnsEvent) {}
}

impl NetworkBehaviourEventProcess<KademliaEvent> for Discovery {
    fn inject_event(&mut self, _event: KademliaEvent) {}
}

impl NetworkBehaviourEventProcess<IdentifyEvent> for Discovery {
    fn inject_event(&mut self, _event: IdentifyEvent) {}
}

impl NetworkBehaviourEventProcess<PingEvent> for Discovery {
    fn inject_event(&mut self, _event: PingEvent) {}
}
