//! Pub sub behaviour for order sharing.

use libp2p::{
    gossipsub::{Gossipsub, GossipsubConfigBuilder, GossipsubEvent, MessageAuthenticity, Topic},
    identity::Keypair,
    swarm::NetworkBehaviourEventProcess,
    NetworkBehaviour,
};

/// Topic for all mainnet v3 orders (unfiltered)
const TOPIC: &str = "/0x-orders/version/3/chain/1/schema/e30=";

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

    pub fn start(&mut self) {
        // Subscribe to orders
        let topic = Topic::new(TOPIC.into());
        self.gossipsub.subscribe(topic);
    }
}

impl NetworkBehaviourEventProcess<GossipsubEvent> for PubSub {
    fn inject_event(&mut self, _event: GossipsubEvent) {}
}
