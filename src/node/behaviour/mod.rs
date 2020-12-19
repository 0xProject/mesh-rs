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

pub mod discovery;
pub mod order_sync;
pub mod pubsub;

use self::{discovery::{Discovery, PeerInfo}, order_sync::OrderSync, pubsub::PubSub};
use crate::prelude::*;
use futures::channel::oneshot;
use libp2p::{
    identity::Keypair, request_response, swarm::NetworkBehaviourEventProcess, NetworkBehaviour,
    PeerId,
};
use std::sync::{Arc, RwLock};
use std::collections::HashMap;

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
        let order_sync = OrderSync::new();

        Ok(Self {
            discovery,
            pubsub,
            order_sync,
        })
    }

    pub fn start(&mut self) -> Result<()> {
        self.discovery.start()?;
        self.pubsub.start();
        Ok(())
    }

    pub fn order_sync_send(
        &mut self,
        peer_id: &PeerId,
        request: order_sync::messages::Request,
        sender: oneshot::Sender<order_sync::Result>,
    ) {
        self.order_sync.send(peer_id, request, sender);
    }


    pub fn known_peers(&self) -> Arc<RwLock<HashMap<PeerId, PeerInfo>>> {
        self.discovery.known_peers()
    }
}

impl NetworkBehaviourEventProcess<()> for Behaviour {
    fn inject_event(&mut self, _event: ()) {}
}
