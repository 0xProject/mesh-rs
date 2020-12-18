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
//! * Observed addresses protocol: https://docs.rs/libp2p-observed-address/0.12.0/libp2p_observed_address/

use crate::prelude::*;
use humantime::Duration as HumanDuration;
use libp2p::{
    identify::{Identify, IdentifyEvent, IdentifyInfo},
    identity::Keypair,
    kad::{
        record::store::MemoryStore, Kademlia, KademliaBucketInserts, KademliaConfig, KademliaEvent,
        QueryId, QueryResult,
    },
    mdns::{Mdns, MdnsEvent},
    ping::{Ping, PingConfig, PingEvent},
    swarm::NetworkBehaviourEventProcess,
    Multiaddr, NetworkBehaviour, PeerId,
};
use std::{collections::HashMap, time::Duration};

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
#[derive(Clone, Debug)]
pub struct PeerInfo {
    peer_id: PeerId,

    /// Latest Identify info
    identify: Option<IdentifyInfo>,

    /// Latest ping time with this node.
    ping: Option<Duration>,
}

impl PeerInfo {
    pub fn new(peer_id: PeerId) -> Self {
        Self {
            peer_id,
            identify: None,
            ping: None,
        }
    }
}

#[derive(NetworkBehaviour)]
pub struct Discovery {
    mdns:     Mdns,
    kademlia: Kademlia<MemoryStore>,
    identify: Identify,
    ping:     Ping,

    #[behaviour(ignore)]
    bootstrap_query_id: Option<QueryId>,

    /// Information that we know about all nodes.
    #[behaviour(ignore)]
    nodes_info: HashMap<PeerId, PeerInfo>,
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
            bootstrap_query_id: None,
            nodes_info: HashMap::new(),
        })
    }

    pub fn start(&mut self) -> Result<()> {
        // Join DHT
        let query_id = self.kademlia.bootstrap().context("Joining Kademlia DHT")?;
        info!("Kademlia Bootstrap started {:?}", &query_id);
        self.bootstrap_query_id = Some(query_id);

        // Start searching for random nodes
        // TODO: self.swarm.search_random_peer();

        Ok(())
    }
}

impl NetworkBehaviourEventProcess<MdnsEvent> for Discovery {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(iter) => for (peer_id, multiaddr) in iter {
                debug!("Discovered {} at {} on LAN.", peer_id, multiaddr);
            },
            MdnsEvent::Expired(iter) => for (peer_id, multiaddr) in iter {
                debug!("Expired {} at {} from LAN.", peer_id, multiaddr);
            },
        }
    }
}

impl NetworkBehaviourEventProcess<KademliaEvent> for Discovery {
    fn inject_event(&mut self, event: KademliaEvent) {
        match event {
            // A query has produced a result.
            KademliaEvent::QueryResult { id, stats, result } => {
                info!("Query {:?} finished with {:?}", &id, stats);
                match result {
                    QueryResult::Bootstrap(result) => {
                        if Some(id) != self.bootstrap_query_id {
                            error!("Received bootstrap result for unknown query id.");
                        }
                        let done = match result {
                            Ok(ok) => {
                                info!("Bootstrap succeeded with {:?}", ok);
                                ok.num_remaining == 0
                            }
                            Err(err) => {
                                error!("Bootstrap failed with {:?}", err);
                                true
                            }
                        };
                        if Some(id) == self.bootstrap_query_id && done {
                            self.bootstrap_query_id = None;
                        }
                    }
                    QueryResult::GetClosestPeers(result) => {
                        // TODO: track query_id
                        match result {
                            Ok(ok) => {
                                info!("Peer query succeeded with {:?}", ok);
                            }
                            Err(err) => {
                                error!("Peer query failed with {:?}", err);
                            }
                        }
                    }
                    result => {
                        error!("Received query result for unsupported query: {:?}", result);
                    }
                }
            }

            // The routing table has been updated with a new peer and / or address, thereby possibly
            // evicting another peer.
            KademliaEvent::RoutingUpdated {
                peer,
                addresses,
                old_peer,
            } => {
                if let Some(old_peer) = old_peer {
                    debug!("Peer {} evicted from routing table", old_peer);
                }
                debug!("Peer {} at {:?} added to routing table", peer, addresses);
            }

            // A peer has connected for whom no listen address is known.
            KademliaEvent::UnroutablePeer { peer } => {
                warn!("Connected peer {} has no routable addresses", peer);
            }

            // A connection to a peer has been established for whom a listen address is known but
            // the peer has not been added to the routing table either because
            // KademliaBucketInserts::Manual is configured or because the corresponding bucket is
            // full.
            KademliaEvent::RoutablePeer { peer, address } => {
                warn!(
                    "Connected peer {} at {} not added to routing table (bucket full)",
                    peer, address
                );
            }

            KademliaEvent::PendingRoutablePeer { peer, address } => {
                debug!(
                    "Peer {} at {} pending inserting in routing table",
                    peer, address
                );
            }
        }
    }
}

impl NetworkBehaviourEventProcess<IdentifyEvent> for Discovery {
    fn inject_event(&mut self, event: IdentifyEvent) {
        match event {
            IdentifyEvent::Received {
                peer_id,
                info,
                observed_addr,
            } => {
                debug!(
                    "Learned about {} at {}: {:?}",
                    &peer_id, observed_addr, &info
                );
                let entry = self
                    .nodes_info
                    .entry(peer_id.clone())
                    .or_insert(PeerInfo::new(peer_id));
                entry.identify = Some(info);
            }
            IdentifyEvent::Sent { peer_id } => {
                debug!("Sent identify info to {}", peer_id);
            }
            IdentifyEvent::Error { peer_id, error } => {
                warn!(
                    "Error in identify protocol from peer {}: {}",
                    peer_id, error
                );
            }
        }
    }
}

impl NetworkBehaviourEventProcess<PingEvent> for Discovery {
    fn inject_event(&mut self, event: PingEvent) {
        match event.result {
            Ok(libp2p::ping::PingSuccess::Ping { rtt }) => {
                debug!(
                    "Pinged {} with round trip time {}",
                    &event.peer,
                    HumanDuration::from(rtt)
                );
                let entry = self
                    .nodes_info
                    .entry(event.peer.clone())
                    .or_insert(PeerInfo::new(event.peer));
                entry.ping = Some(rtt);
            }
            Ok(libp2p::ping::PingSuccess::Pong) => {
                debug!("Sent pong to {}", event.peer);
            }
            Err(err) => {
                error!("Ping failed for {}: {:?}", event.peer, err);
            }
        }
    }
}
