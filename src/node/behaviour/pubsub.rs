//! Pub sub behaviour for order sharing.

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
    request_response::{
        InboundFailure, OutboundFailure, RequestId, RequestResponseEvent, RequestResponseMessage,
    },
    swarm::NetworkBehaviourEventProcess,
    Multiaddr, NetworkBehaviour, PeerId,
};
use std::{collections::HashMap, time::Duration};

#[derive(NetworkBehaviour)]
pub(crate) struct PubSub {
    gossipsub: Gossipsub,
}

impl PubSub {
    pub(crate) fn new(peer_key: Keypair) -> Self {
        // GossipSub
        let gossipsub_config = GossipsubConfigBuilder::new()
            .max_transmit_size(262144)
            .build();
        let mut gossipsub = Gossipsub::new(MessageAuthenticity::Signed(peer_key), gossipsub_config);

        Self { gossipsub }
    }
}

impl NetworkBehaviourEventProcess<GossipsubEvent> for PubSub {
    fn inject_event(&mut self, event: GossipsubEvent) {}
}
