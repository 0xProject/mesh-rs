//! OrderSync v0 protocol
//!
//! Implemented using RequestResponse.
//!
//! A async rpc interface is implemented similar to [substrate][sub].
//!
//! [sub]: https://github.com/paritytech/substrate/blob/6b600cdeb4043e512bc5f342eb02a5a17d26797a/client/network/src/request_responses.rs#L59
//!
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
pub mod messages;

use self::{
    json_codec::JsonCodec,
    messages::{Message, Request, Response},
};
use crate::prelude::*;
use futures::channel::oneshot;
use libp2p::{
    core::ProtocolName,
    request_response::{
        OutboundFailure, ProtocolSupport, RequestId, RequestResponse, RequestResponseConfig,
        RequestResponseEvent, RequestResponseMessage,
    },
    swarm::NetworkBehaviourEventProcess,
    NetworkBehaviour, PeerId,
};
use std::{collections::HashMap, iter};

/// Maximum message size
const MAX_SIZE: usize = 1024;

#[derive(Clone, Debug)]
pub struct Version();

pub type Config = RequestResponseConfig;
pub type Event = RequestResponseEvent<Message, Message>;
pub type Codec = JsonCodec<Version, Message, Message>;
pub type Result = std::result::Result<Response, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Expected a Response message, but received a Request.")]
    UnexpectedRequest,

    #[error("OrderSync got dropped before request was handeled.")]
    Cancelled,

    #[error("Failure during request: {0:?}")]
    OutboundFailure(OutboundFailure),
}

#[derive(NetworkBehaviour)]
pub struct OrderSync {
    request_response: RequestResponse<Codec>,

    #[behaviour(ignore)]
    pending_requests: HashMap<RequestId, oneshot::Sender<Result>>,
}

impl OrderSync {
    pub fn new(config: Config) -> Self {
        let protocols = iter::once((Version(), ProtocolSupport::Full));
        let codec = JsonCodec::default();
        Self {
            request_response: RequestResponse::new(codec, protocols, config),
            pending_requests: HashMap::new(),
        }
    }
}

impl ProtocolName for Version {
    fn protocol_name(&self) -> &[u8] {
        b"/0x-mesh/order-sync/version/0"
    }
}

impl NetworkBehaviourEventProcess<Event> for OrderSync {
    fn inject_event(&mut self, event: Event) {
        match event {
            // Receive incoming request.
            RequestResponseEvent::Message {
                peer,
                message:
                    RequestResponseMessage::Request {
                        request_id,
                        request,
                        channel,
                    },
            } => {
                warn!("Incoming requests are not handled (unimplemented).");
            }

            // Receive incoming response.
            RequestResponseEvent::Message {
                peer,
                message:
                    RequestResponseMessage::Response {
                        request_id,
                        response,
                    },
            } => {
                if let Some(sender) = self.pending_requests.remove(&request_id) {
                    sender.send(match response {
                        Message::Request(_) => Err(Error::UnexpectedRequest),
                        Message::Response(res) => Ok(res),
                    });
                } else {
                    error!(
                        "Received response for unexpected request id {} from peer {}",
                        request_id, peer
                    );
                }
            }

            // A request we initiated failed.
            RequestResponseEvent::OutboundFailure {
                peer,
                request_id,
                error,
            } => {
                if let Some(sender) = self.pending_requests.remove(&request_id) {
                    sender.send(Err(Error::OutboundFailure(error)));
                } else {
                    error!(
                        "Failure for unexpected outbound request id {} from peer {}: {:?}",
                        request_id, peer, error
                    );
                }
            }

            // A request remote initiated failed. (Either during reading the request or sending the
            // response).
            RequestResponseEvent::InboundFailure {
                peer,
                request_id,
                error,
            } => {
                error!(
                    "Failure for inbound request id {} from peer {}: {:?}",
                    request_id, peer, error
                );
            }

            // A response to an inbound request has been sent.
            RequestResponseEvent::ResponseSent { request_id, peer } => {}
        }
    }
}
