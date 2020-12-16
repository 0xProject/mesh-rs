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

use crate::prelude::*;
use async_trait::async_trait;
use libp2p::{
    core::{
        upgrade::{read_one, write_one},
        ProtocolName,
    },
    request_response::{
        ProtocolSupport, RequestResponse, RequestResponseCodec, RequestResponseConfig,
        RequestResponseEvent,
    },
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io, iter};

/// Maximum message size
const MAX_SIZE: usize = 1024;

#[derive(Clone, Debug)]
pub struct Version();

#[derive(Clone, Debug)]
pub struct Codec();

pub type Config = RequestResponseConfig;
pub type Protocol = RequestResponse<Codec>;
pub type Event = RequestResponseEvent<Request, Response>;

/// The OrderSync protocol uses the same internally tagged JSON object
/// for request and response.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum Message {
    Request(Request),
    Response(Response),
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Request {
    subprotocols: Vec<String>,
    metadata:     Option<RequestMetadata>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum RequestMetadata {
    V0 {
        #[serde(rename = "snapshotID")]
        snapshot_id: String,

        page: i64,

        #[serde(rename = "orderfilter")]
        order_filter: Option<OrderFilter>,
    },
    V1 {
        #[serde(rename = "minOrderHash")]
        min_order_hash: Hash,

        #[serde(rename = "orderfilter")]
        order_filter: OrderFilter,
    },
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Response {
    orders: Vec<Order>,

    complete: bool,

    // TODO: Is this field really optional?
    #[serde(flatten)]
    metadata: Option<ResponseMetadata>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "subprotocol", content = "metadata")]
pub enum ResponseMetadata {
    #[serde(rename = "/pagination-with-filter/version/0")]
    V0 {
        #[serde(rename = "snapshotID")]
        snapshot_id: String,

        page: i64,
    },
    #[serde(rename = "/pagination-with-filter/version/1")]
    V1 {
        #[serde(rename = "nextMinOrderHash")]
        next_min_order_hash: Hash,
    },
}

#[derive(Clone, PartialEq, Eq, Default, Debug, Serialize, Deserialize)]
pub struct Order(HashMap<String, String>);

#[derive(Clone, PartialEq, Eq, Default, Debug, Serialize, Deserialize)]
pub struct OrderFilter(HashMap<String, String>);

// TODO: This may not be correct
#[derive(Clone, PartialEq, Eq, Default, Debug, Serialize, Deserialize)]
pub struct Hash(HashMap<String, String>);

pub fn new(config: Config) -> Protocol {
    let protocols = iter::once((Version(), ProtocolSupport::Full));
    RequestResponse::new(Codec(), protocols, config)
}

impl ProtocolName for Version {
    fn protocol_name(&self) -> &[u8] {
        b"/0x-mesh/order-sync/version/0"
    }
}

impl Default for Request {
    fn default() -> Self {
        Request {
            subprotocols: vec![
                "/pagination-with-filter/version/0".into(),
                "/pagination-with-filter/version/1".into(),
            ],
            metadata:     None,
        }
    }
}

#[async_trait]
impl RequestResponseCodec for Codec {
    type Protocol = Version;
    type Request = Request;
    type Response = Response;

    async fn read_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        // OPT: Streaming read
        let mut buffer = Vec::new();
        io.read_to_end(&mut buffer).await?;
        let message = serde_json::de::from_slice::<Message>(&buffer)?;
        match message {
            Message::Request(obj) => Ok(obj),
            _ => {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Expected Request object",
                ))
            }
        }
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        // OPT: Streaming read
        let mut buffer = Vec::new();
        io.read_to_end(&mut buffer).await?;
        let message = serde_json::de::from_slice::<Message>(&buffer)?;
        match message {
            Message::Response(obj) => Ok(obj),
            _ => {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Expected Response object",
                ))
            }
        }
    }

    async fn write_request<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        // OPT: Streaming write
        io.write_all(serde_json::to_vec(&Message::Request(req))?.as_slice())
            .await
    }

    async fn write_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        // OPT: Streaming write
        io.write_all(serde_json::to_vec(&Message::Response(res))?.as_slice())
            .await
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn test_request_json() {
        let message = Message::Request(Request::default());
        assert_eq!(
            serde_json::to_value(&message).unwrap(),
            json!({
                "type": "Request",
                "subprotocols": [
                    "/pagination-with-filter/version/0",
                    "/pagination-with-filter/version/1",
                ],
                "metadata": null
            })
        );
    }

    #[test]
    fn test_response_json() {
        let message = Message::Response(Response {
            complete: false,
            metadata: Some(ResponseMetadata::V0 {
                page:        0,
                snapshot_id: "0x172b4c50e71cb73ed3ac8d191a6ddaf683d70757c848b62f6b33b3845bcbecbd"
                    .into(),
            }),
            orders:   vec![],
        });
        assert_eq!(
            serde_json::to_value(&message).unwrap(),
            json!({
                "type": "Response",
                "subprotocol":"/pagination-with-filter/version/0",
                "orders":[],
                "complete":false,
                "metadata": {
                    "page":0,"snapshotID":"0x172b4c50e71cb73ed3ac8d191a6ddaf683d70757c848b62f6b33b3845bcbecbd"
                },
            })
        );
    }
}
