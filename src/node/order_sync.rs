//! TODO: Add Throttling: https://docs.rs/libp2p/0.32.2/libp2p/request_response/struct.Throttled.html

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
use std::{io, iter};

/// Maximum message size
const MAX_SIZE: usize = 1024;

pub type Config = RequestResponseConfig;
pub type Protocol = RequestResponse<Codec>;
pub type Event = RequestResponseEvent<Request, Response>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Version {
    V0,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Request {
    None,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Response {
    None,
}

#[derive(Clone)]
pub struct Codec();

pub fn new(version: Version, config: Config) -> Protocol {
    let protocols = iter::once((version, ProtocolSupport::Full));
    RequestResponse::new(Codec(), protocols, config)
}

impl ProtocolName for Version {
    fn protocol_name(&self) -> &[u8] {
        match *self {
            Version::V0 => b"/0x-mesh/order-sync/version/0",
        }
    }
}

async fn read_msg<T>(socket: &mut T) -> io::Result<Vec<u8>>
where
    T: Unpin + AsyncRead,
{
    read_one(socket, MAX_SIZE)
        .map(|res| {
            match res {
                Err(e) => Err(io::Error::new(io::ErrorKind::InvalidData, e)),
                Ok(vec) if vec.is_empty() => Err(io::ErrorKind::UnexpectedEof.into()),
                Ok(vec) => Ok(vec),
            }
        })
        .await
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
        let msg = read_msg(io).await?;
        info!("Req: {:?}", msg);
        Ok(Request::None)
    }

    async fn read_response<T>(
        &mut self,
        _protocol: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let msg = read_msg(io).await?;
        info!("Req: {:?}", msg);
        Ok(Response::None)
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
        write_one(io, b"Test").await
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
        write_one(io, b"Test").await
    }
}
