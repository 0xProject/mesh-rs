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
use smallvec::{smallvec, SmallVec};
use std::{
    io,
    io::{Error, ErrorKind},
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
    pub subprotocols: SmallVec<[String; 2]>,
    pub metadata:     RequestMetadataContainer,
}

/// Redundant wrapper for metadata
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct RequestMetadataContainer {
    pub metadata: SmallVec<[RequestMetadata; 2]>,
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
    pub orders: Vec<Order>,

    pub complete: bool,

    #[serde(flatten)]
    pub metadata: ResponseMetadata,
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
    pub chain_id:                i64, // Note: Unlike elsewhere, not renamed to chainID
    pub exchange_address:        String,
    pub maker_address:           String,
    pub maker_asset_data:        String,
    pub maker_fee_asset_data:    String,
    pub maker_asset_amount:      String,
    pub maker_fee:               String,
    pub taker_address:           String,
    pub taker_asset_data:        String,
    pub taker_fee_asset_data:    String,
    pub taker_asset_amount:      String,
    pub taker_fee:               String,
    pub sender_address:          String,
    pub fee_recipient_address:   String,
    pub expiration_time_seconds: String,
    pub salt:                    String,
    pub signature:               String,
}

/// See <https://github.com/0xProject/0x-mesh/blob/b2a12fdb186fb56eb7d99dc449b9773d0943ee8e/orderfilter/shared.go#L144>
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderFilter {
    pub custom_order_schema: String,

    #[serde(rename = "chainID")]
    pub chain_id: i64,

    pub exchange_address: String,
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
    #[allow(dead_code)]
    pub fn mainnet_v3() -> Self {
        OrderFilter {
            chain_id: 1,
            exchange_address: "0x61935cbdd02287b511119ddb11aeb42f1593b7ef".into(),
            ..OrderFilter::default()
        }
    }

    #[allow(dead_code)]
    pub fn mainnet_v2() -> Self {
        OrderFilter {
            chain_id: 1,
            exchange_address: "0x080bf510fcbf18b91105470639e9561022937712".into(),
            ..OrderFilter::default()
        }
    }
}

impl Default for Request {
    fn default() -> Self {
        Request::from(OrderFilter::default())
    }
}

impl Default for Response {
    fn default() -> Self {
        Response {
            complete: true,
            orders:   vec![],
            metadata: ResponseMetadata::V0 {
                page:        0,
                snapshot_id: "".into(),
            },
        }
    }
}

impl From<OrderFilter> for Request {
    fn from(order_filter: OrderFilter) -> Self {
        Request {
            subprotocols: smallvec![
                "/pagination-with-filter/version/1".into(),
                "/pagination-with-filter/version/0".into(),
            ],
            metadata:     RequestMetadataContainer {
                metadata: smallvec![
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

impl RequestMetadata {
    pub fn sub_protocol_name(&self) -> &str {
        match self {
            Self::V0 { .. } => "/pagination-with-filter/version/0",
            Self::V1 { .. } => "/pagination-with-filter/version/1",
        }
    }

    pub fn order_filter_ref<'a>(&'a self) -> &'a OrderFilter {
        match self {
            Self::V0 { order_filter, .. } => order_filter,
            Self::V1 { order_filter, .. } => order_filter,
        }
    }

    pub fn order_filter_mut<'a>(&'a mut self) -> &'a mut OrderFilter {
        match self {
            Self::V0 { order_filter, .. } => order_filter,
            Self::V1 { order_filter, .. } => order_filter,
        }
    }
}

impl From<ResponseMetadata> for RequestMetadata {
    fn from(response: ResponseMetadata) -> Self {
        match response {
            ResponseMetadata::V0 { page, snapshot_id } => {
                RequestMetadata::V0 {
                    page: page + 1,
                    snapshot_id,
                    order_filter: OrderFilter::default(),
                }
            }
            ResponseMetadata::V1 {
                next_min_order_hash,
            } => {
                RequestMetadata::V1 {
                    min_order_hash: next_min_order_hash,
                    order_filter:   OrderFilter::default(),
                }
            }
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
    trace!("Attempting to read JSON from socket");
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
        trace!("Read {} more bytes, total {} in buffer", n, buffer.len());

        // Try to parse
        let result = serde_json::de::from_slice::<T>(&buffer);
        match result {
            Err(e) if e.is_eof() => {
                // Read some more
                continue;
            }
            _ => {}
        }

        trace!("Parse result {:?}", result);
        if result.is_err() {
            error!("Could not parse: {}", String::from_utf8_lossy(&buffer));
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
        debug!("OrderSync receiving request");
        let message = read_json::<_, Message>(io).await?;
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
        debug!("OrderSync receiving response");
        let message = read_json::<_, Message>(io).await?;
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
        debug!("OrderSync send request: {:?}", &message);
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
        debug!("OrderSync send response: {:?}", &message);
        io.write_all(serde_json::to_vec(&message)?.as_slice()).await
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test::prelude::assert_eq;
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
                subprotocols: smallvec![
                    "/pagination-with-filter/version/1".into(),
                    "/pagination-with-filter/version/0".into(),
                ],
                metadata:     RequestMetadataContainer {
                    metadata: smallvec![
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
