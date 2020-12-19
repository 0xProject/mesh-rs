//! ## To do
//!
//! * De-stringify types such as Hashes, etc.

use crate::prelude::*;

/// The OrderSync protocol uses the same internally tagged JSON object
/// for request and response.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Message {
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
        Self {
            chain_id:            i64::default(),
            custom_order_schema: "{}".into(),
            exchange_address:    "0x0000000000000000000000000000000000000000".into(),
        }
    }
}

impl OrderFilter {
    #[allow(dead_code)]
    pub fn mainnet_v3() -> Self {
        Self {
            chain_id: 1,
            exchange_address: "0x61935cbdd02287b511119ddb11aeb42f1593b7ef".into(),
            ..Self::default()
        }
    }

    #[allow(dead_code)]
    pub fn mainnet_v2() -> Self {
        Self {
            chain_id: 1,
            exchange_address: "0x080bf510fcbf18b91105470639e9561022937712".into(),
            ..Self::default()
        }
    }
}

impl Default for Request {
    fn default() -> Self {
        Self::from(OrderFilter::default())
    }
}

impl Default for Response {
    fn default() -> Self {
        Self {
            complete: true,
            orders:   vec![],
            metadata: ResponseMetadata::V0 {
                page:        0,
                snapshot_id: "".into(),
            },
        }
    }
}

impl Response {
    pub fn next_request(&self) -> Option<Request> {
        if self.complete { None } else {
            Some(self.metadata.next_request_metadata().into())
        }
    }
}


impl From<OrderFilter> for Request {
    fn from(order_filter: OrderFilter) -> Self {
        Self {
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

impl ResponseMetadata {
    fn next_request_metadata(&self) -> RequestMetadata {
        match self {
            ResponseMetadata::V0 { page, snapshot_id } => {
                RequestMetadata::V0 {
                    page: page + 1,
                    snapshot_id: snapshot_id.clone(),
                    order_filter: OrderFilter::default(),
                }
            }
            ResponseMetadata::V1 {
                next_min_order_hash,
            } => {
                RequestMetadata::V1 {
                    min_order_hash: next_min_order_hash.clone(),
                    order_filter:   OrderFilter::default(),
                }
            }
        }
    }
}

impl From<RequestMetadata> for Request {
    fn from(metadata: RequestMetadata) -> Self {
        Self {
            subprotocols: smallvec![metadata.sub_protocol_name().into()],
            metadata: RequestMetadataContainer {
                metadata: smallvec![metadata]
            }
        }
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
                            page: 0,
                            snapshot_id: "".into(),
                            order_filter,
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
        let response = include_str!("../../../../test/response.json");
        let _message = serde_json::from_str::<Message>(response).unwrap();
    }
}
