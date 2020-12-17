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
    #[serde(rename_all = "camelCase")]
    V1 {
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
#[serde(rename_all = "camelCase")]
pub enum ResponseMetadata {
    #[serde(rename = "/pagination-with-filter/version/0")]
    V0 {
        #[serde(rename = "snapshotID")]
        snapshot_id: String,

        page: i64,
    },
    #[serde(rename = "/pagination-with-filter/version/1")]
    #[serde(rename_all = "camelCase")]
    V1 { next_min_order_hash: String },
}

/// See <https://github.com/0xProject/0x-mesh/blob/b2a12fdb186fb56eb7d99dc449b9773d0943ee8e/zeroex/order.go#L538>
#[derive(Clone, PartialEq, Eq, Default, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    #[serde(rename = "chainId")]
    chain_id:                i64,
    exchange_address:        String,
    maker_address:           String,
    maker_asset_data:        String,
    maker_fee_asset_data:    String,
    maker_asset_amount:      String,
    maker_fee:               String,
    taker_address:           String,
    taker_asset_data:        String,
    taker_fee_asset_data:    String,
    taker_asset_amount:      String,
    taker_fee:               String,
    sender_address:          String,
    fee_recipient_address:   String,
    expiration_time_seconds: String,
    salt:                    String,
    signature:               String,
}

/// See <https://github.com/0xProject/0x-mesh/blob/b2a12fdb186fb56eb7d99dc449b9773d0943ee8e/orderfilter/shared.go#L144>
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderFilter {
    custom_order_schema: String,

    #[serde(rename = "chainID")]
    chain_id: i64,

    exchange_address: String,
}

impl Default for OrderFilter {
    fn default() -> Self {
        OrderFilter {
            chain_id:            i64::default(),
            custom_order_schema: "{}".into(),
            exchange_address:    "0x0000000000000000000000000000000000000000".into(),
        }
    }
}

impl OrderFilter {
    pub fn mainnet_v3() -> Self {
        OrderFilter {
            chain_id: 1,
            exchange_address: "0x61935cbdd02287b511119ddb11aeb42f1593b7ef".into(),
            ..Self::default()
        }
    }

    pub fn mainnet_v2() -> Self {
        OrderFilter {
            chain_id: 1,
            exchange_address: "0x080bf510fcbf18b91105470639e9561022937712".into(),
            ..Self::default()
        }
    }
}

impl Default for Request {
    fn default() -> Self {
        Request::from(OrderFilter::default())
    }
}

impl From<OrderFilter> for Request {
    fn from(order_filter: OrderFilter) -> Self {
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
                        order_filter:   order_filter.clone(),
                    },
                    RequestMetadata::V0 {
                        snapshot_id: String::default(),
                        page: 0,
                        order_filter,
                    },
                ],
            },
        }
    }
}

pub fn new(config: Config) -> Protocol {
    let protocols = iter::once((Version(), ProtocolSupport::Full));
    RequestResponse::new(Codec(), protocols, config)
}

impl ProtocolName for Version {
    fn protocol_name(&self) -> &[u8] {
        b"/0x-mesh/order-sync/version/0"
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
                            "orderfilter": {
                                "chainID": 0,
                                "customOrderSchema": "{}",
                                "exchangeAddress": "0x0000000000000000000000000000000000000000",
                            },
                        },
                        {
                            "page": 0,
                            "snapshotID": "",
                            "orderfilter": {
                                "chainID": 0,
                                "customOrderSchema": "{}",
                                "exchangeAddress": "0x0000000000000000000000000000000000000000",
                            },
                        }
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
        let order_filter = OrderFilter {
            chain_id:            4,
            custom_order_schema: "{}".into(),
            exchange_address:    "0x198805e9682fceec29413059b68550f92868c129".into(),
        };
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
            orders:   vec![], // TODO: Add content
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

    #[test]
    fn test_parse_response() {
        let response = include_str!("../../test/response.json");
        let message = serde_json::from_str::<Message>(response).unwrap();
    }
}
