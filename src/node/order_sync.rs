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
//!
//! TODO: De-stringify types such as Hashes, etc.

use crate::prelude::*;
use async_trait::async_trait;
use libp2p::{
    core::ProtocolName,
    request_response::{
        ProtocolSupport, RequestResponse, RequestResponseCodec, RequestResponseConfig,
        RequestResponseEvent,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io,
    io::{Error, ErrorKind, Result},
    iter,
};

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
    metadata:     RequestMetadataContainer,
}

/// Redundant wrapper for metadata
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct RequestMetadataContainer {
    metadata: Vec<RequestMetadata>,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestMetadata {
    V0 {
        #[serde(rename = "snapshotID")]
        snapshot_id: String,

        page: i64,

        #[serde(rename = "orderfilter")]
        order_filter: OrderFilter,
    },
    V1 {
        #[serde(rename = "minOrderHash")]
        min_order_hash: String,

        #[serde(rename = "orderfilter")]
        order_filter: OrderFilter,
    },
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Response {
    orders: Vec<Order>,

    complete: bool,

    #[serde(flatten)]
    metadata: ResponseMetadata,
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
        next_min_order_hash: String,
    },
}

#[derive(Clone, PartialEq, Eq, Default, Debug, Serialize, Deserialize)]
pub struct Order(HashMap<String, String>);

#[derive(Clone, PartialEq, Eq, Default, Debug, Serialize, Deserialize)]
pub struct OrderFilter(HashMap<String, serde_json::value::Value>);

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
                "/pagination-with-filter/version/1".into(),
                "/pagination-with-filter/version/0".into(),
            ],
            metadata:     RequestMetadataContainer {
                metadata: vec![
                    RequestMetadata::V1 {
                        min_order_hash:
                            "0x0000000000000000000000000000000000000000000000000000000000000000"
                                .into(),
                        order_filter:   OrderFilter::default(),
                    },
                    RequestMetadata::V0 {
                        snapshot_id:  String::default(),
                        page:         0,
                        order_filter: OrderFilter::default(),
                    },
                ],
            },
        }
    }
}

/// Read Serde-JSON from an AsyncRead.
///
/// This is difficult because there is no framing other than JSON succeeding to
/// parse. All we can do, it seems, is to repeatedly try parsing and wait for
/// more content to arrive if it fails.
///
/// TODO: Maximum size
///
/// TODO: Remove once Serde gains async support.
/// See <https://github.com/serde-rs/json/issues/316>
async fn read_json<R, T>(io: &mut R) -> io::Result<T>
where
    R: AsyncRead + Unpin + Send,
    T: for<'a> Deserialize<'a> + std::fmt::Debug,
{
    warn!("Attempting to read JSON from socket");
    let mut buffer = Vec::new();
    loop {
        // Read another (partial) block
        let mut block = [0u8; 1024];
        let n = match io.read(&mut block).await {
            Ok(0) => {
                Err(Error::new(
                    ErrorKind::UnexpectedEof,
                    "Unexpected EOF while reading JSON.",
                ))
            }
            r => r,
        }?;
        buffer.extend(&block[..n]);
        info!("Read {} more bytes, total {} in buffer", n, buffer.len());

        // Try to parse
        let result = serde_json::de::from_slice::<T>(&buffer);
        match result {
            Err(e) if e.is_eof() => {
                // Read some more
                continue;
            }
            _ => {}
        }

        warn!("Parse result {:?}", result);
        if result.is_err() {
            warn!("Buffer {}", String::from_utf8_lossy(&buffer));
        }
        return Ok(result?);
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
        warn!("OrderSync received request");
        let message = read_json::<_, Message>(io).await?;
        warn!("OrderSync received request: {:?}", &message);
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
        warn!("OrderSync received response");
        let message = read_json::<_, Message>(io).await?;
        warn!("OrderSync received response: {:?}", &message);
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
        let message = Message::Request(req);
        warn!("OrderSync send request: {:?}", &message);
        io.write_all(serde_json::to_vec(&message)?.as_slice()).await
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
        let message = Message::Response(res);
        warn!("OrderSync send response: {:?}", &message);
        io.write_all(serde_json::to_vec(&message)?.as_slice()).await
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn test_default_request_json() {
        let message = Message::Request(Request::default());
        assert_eq!(
            serde_json::to_value(&message).unwrap(),
            json!({
                "type": "Request",
                "subprotocols": [
                    "/pagination-with-filter/version/1",
                    "/pagination-with-filter/version/0",
                ],
                "metadata": {
                    "metadata": [
                        {
                            "minOrderHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                            "orderfilter": {},
                        },
                        {
                            "page": 0,
                            "snapshotID": "",
                            "orderfilter": {},
                        }
                    ],
                }
            })
        );
    }

    #[test]
    fn test_filter_request_json() {
        let message = Message::Request(Request {
            subprotocols: vec!["/pagination-with-filter/version/1".into()],
            metadata:     RequestMetadataContainer {
                metadata: vec![RequestMetadata::V1 {
                    min_order_hash:
                        "0x0000000000000000000000000000000000000000000000000000000000000000".into(),
                    order_filter:   OrderFilter(
                        [("a".to_string(), json!("b")), ("b".to_string(), json!(5))]
                            .iter()
                            .cloned()
                            .collect(),
                    ),
                }],
            },
        });
        assert_eq!(
            serde_json::to_value(&message).unwrap(),
            json!({
                "type": "Request",
                "subprotocols": [
                    "/pagination-with-filter/version/1",
                ],
                "metadata": {
                    "metadata": [
                        {
                            "minOrderHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                            "orderfilter": {
                                "a":"b",
                                "b":5,
                            },
                        },
                    ],
                }
            })
        );
    }

    #[test]
    fn test_request_parse() {
        let request = r#"{
            "type": "Request",
            "subprotocols": [
                "/pagination-with-filter/version/1",
                "/pagination-with-filter/version/0"
            ],
            "metadata": {
                "metadata": [
                {
                    "minOrderHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                    "orderfilter": {
                        "chainID": 4,
                        "customOrderSchema": "{}",
                        "exchangeAddress": "0x198805e9682fceec29413059b68550f92868c129"
                    }
                },
                {
                    "page": 0,
                    "snapshotID": "",
                    "orderfilter": {
                        "chainID": 4,
                        "customOrderSchema": "{}",
                        "exchangeAddress": "0x198805e9682fceec29413059b68550f92868c129"
                    }
                }
                ]
            }
        }"#;
        let message = serde_json::from_str::<Message>(request).unwrap();
        let order_filter = OrderFilter(
            [
                ("chainID".to_string(), json!(4)),
                ("customOrderSchema".to_string(), json!("{}")),
                (
                    "exchangeAddress".to_string(),
                    json!("0x198805e9682fceec29413059b68550f92868c129"),
                ),
            ]
            .iter()
            .cloned()
            .collect(),
        );
        assert_eq!(
            message,
            Message::Request(Request {
                subprotocols: vec![
                    "/pagination-with-filter/version/1".into(),
                    "/pagination-with-filter/version/0".into(),
                ],
                metadata:     RequestMetadataContainer {
                    metadata: vec![
                        RequestMetadata::V1 {
                            min_order_hash:
                                "0x0000000000000000000000000000000000000000000000000000000000000000"
                                    .into(),
                            order_filter:   order_filter.clone(),
                        },
                        RequestMetadata::V0 {
                            page:         0,
                            snapshot_id:  "".into(),
                            order_filter: order_filter.clone(),
                        },
                    ],
                },
            })
        );
    }

    #[test]
    fn test_response_json() {
        let message = Message::Response(Response {
            complete: false,
            metadata: ResponseMetadata::V0 {
                page:        0,
                snapshot_id: "0x172b4c50e71cb73ed3ac8d191a6ddaf683d70757c848b62f6b33b3845bcbecbd"
                    .into(),
            },
            orders:   vec![Order(
                [
                    ("chainId", "1"),
                    (
                        "exchangeAddress",
                        "0x61935cbdd02287b511119ddb11aeb42f1593b7ef",
                    ),
                ]
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
            )],
        });
        assert_eq!(
            serde_json::to_value(&message).unwrap(),
            json!({
                "type": "Response",
                "subprotocol":"/pagination-with-filter/version/0",
                "orders":[{
                    "chainId": "1",
                    "exchangeAddress": "0x61935cbdd02287b511119ddb11aeb42f1593b7ef",
                }],
                "complete":false,
                "metadata": {
                    "page":0,"snapshotID":"0x172b4c50e71cb73ed3ac8d191a6ddaf683d70757c848b62f6b33b3845bcbecbd"
                },
            })
        );
    }
}
