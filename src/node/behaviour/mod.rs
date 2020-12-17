//! Stack of behaviours for the node.
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

mod discovery;
mod order_sync;
mod pubsub;

use self::{
    discovery::{Discovery},
    order_sync::OrderSync,
    pubsub::PubSub,
};
use crate::prelude::*;
use libp2p::{identity::Keypair, swarm::NetworkBehaviourEventProcess, NetworkBehaviour};

#[derive(NetworkBehaviour)]
pub struct Behaviour {
    discovery:  Discovery,
    pubsub:     PubSub,
    order_sync: OrderSync,
}

impl Behaviour {
    pub async fn new(peer_key: Keypair) -> Result<Self> {
        let discovery = Discovery::new(peer_key.clone()).await?;
        let pubsub = PubSub::new(peer_key);
        let order_sync = OrderSync::new(order_sync::Config::default());

        Ok(Self {
            discovery,
            pubsub,
            order_sync,
        })
    }
}

impl NetworkBehaviourEventProcess<()> for Behaviour {
    fn inject_event(&mut self, _event: ()) {}
}
