//! TODO: Add Throttling: https://docs.rs/libp2p/0.32.2/libp2p/request_response/struct.Throttled.html
//!
//! TODO:
//!
//! This protocol implements set reconciliation, but does so in a rather
//! inefficient way (bulk transfer of all the orders). There more efficient
//! reconciliation algorithms out there that efficiently compute the set
//! difference first. For an academic overview see
//!
//! * Ivo Kubjas (2014). "Set Reconciliation Master Thesis". [pdf](https://comserv.cs.ut.ee/home/files/kubjas_cybersecurity_2014.pdf?study=ATILoputoo&reference=E731444824814AE27FE0D91FA073B5F3FE61038D)
//!
//! There is a crate for Minisketch that should allow prototyping something:
//!
//! <https://docs.rs/minisketch-rs/0.1.9/minisketch_rs/>

mod json_codec;
mod messages;

use self::{json_codec::JsonCodec, messages::Message};

use libp2p::{
    core::ProtocolName,
    request_response::{
        ProtocolSupport, RequestResponse, RequestResponseConfig, RequestResponseEvent,
    },
    swarm::NetworkBehaviourEventProcess,
    NetworkBehaviour,
};

use std::iter;

/// Maximum message size
const MAX_SIZE: usize = 1024;

#[derive(Clone, Debug)]
pub struct Version();

pub type Config = RequestResponseConfig;
pub type Event = RequestResponseEvent<Message, Message>;
pub type Codec = JsonCodec<Version, Message, Message>;

#[derive(NetworkBehaviour)]
pub struct OrderSync {
    #[behaviour(event_process = false)]
    request_response: RequestResponse<Codec>,
}

impl OrderSync {
    pub fn new(config: Config) -> Self {
        let protocols = iter::once((Version(), ProtocolSupport::Full));
        let codec = JsonCodec::default();
        let request_response = RequestResponse::new(codec, protocols, config);
        Self { request_response }
    }
}

impl ProtocolName for Version {
    fn protocol_name(&self) -> &[u8] {
        b"/0x-mesh/order-sync/version/0"
    }
}

impl NetworkBehaviourEventProcess<Event> for OrderSync {
    fn inject_event(&mut self, _event: Event) {}
}
