//! Pub sub behaviour for order sharing.

use libp2p::{
    gossipsub::{Gossipsub, GossipsubConfigBuilder, GossipsubEvent, MessageAuthenticity},
    identity::Keypair,
    swarm::NetworkBehaviourEventProcess,
    NetworkBehaviour,
};

#[derive(NetworkBehaviour)]
pub struct PubSub {
    gossipsub: Gossipsub,
}

impl PubSub {
    pub(crate) fn new(peer_key: Keypair) -> Self {
        // GossipSub
        let gossipsub_config = GossipsubConfigBuilder::new()
            .max_transmit_size(262_144)
            .build();
        let gossipsub = Gossipsub::new(MessageAuthenticity::Signed(peer_key), gossipsub_config);

        Self { gossipsub }
    }
}

impl NetworkBehaviourEventProcess<GossipsubEvent> for PubSub {
    fn inject_event(&mut self, _event: GossipsubEvent) {}
}
