//! Generic RequestResponseCodec for Serde types using raw JSON.
//!
//! **Note.** Do not use for new protocols.
//!
//! Raw JSON does not include a length prefix, so the solve the framing problem
//! we repeatedly try parsing and read more content to the buffer until it
//! succeeds.
//!
//! ## To do
//!
//! * Implement maximum buffer size.

use crate::{prelude::*, utils::read_json};
use libp2p::{core::ProtocolName, request_response::RequestResponseCodec};
use std::marker::PhantomData;

#[derive(Clone, Debug)]
pub struct JsonCodec<Protocol, Request, Response>
where
    Protocol: Clone + Send + Sync + ProtocolName,
    Request: Send + Sync + Serialize + for<'a> Deserialize<'a>,
    Response: Send + Sync + Serialize + for<'a> Deserialize<'a>,
{
    protocol: PhantomData<Protocol>,
    request:  PhantomData<Request>,
    response: PhantomData<Response>,
}

impl<Protocol, Request, Response> Default for JsonCodec<Protocol, Request, Response>
where
    Protocol: Clone + Send + Sync + ProtocolName,
    Request: Send + Sync + Serialize + for<'a> Deserialize<'a>,
    Response: Send + Sync + Serialize + for<'a> Deserialize<'a>,
{
    fn default() -> Self {
        Self {
            protocol: PhantomData,
            request:  PhantomData,
            response: PhantomData,
        }
    }
}

#[async_trait]
impl<Protocol, Request, Response> RequestResponseCodec for JsonCodec<Protocol, Request, Response>
where
    Protocol: Clone + Send + Sync + ProtocolName,
    Request: Send + Sync + Serialize + for<'a> Deserialize<'a>,
    Response: Send + Sync + Serialize + for<'a> Deserialize<'a>,
{
    type Protocol = Protocol;
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
        read_json::<_, Request>(io).await
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_json::<_, Response>(io).await
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
        io.write_all(serde_json::to_vec(&req)?.as_slice()).await
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
        io.write_all(serde_json::to_vec(&res)?.as_slice()).await
    }
}
