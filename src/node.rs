use anyhow::{Context, Result};
use futures::{Future, FutureExt as _};
use libp2p::{tcp::TcpConfig, Multiaddr, Transport};
use log::info;
use std::{convert::Infallible, net::SocketAddr};
use tokio;
use tokio_compat_02::FutureExt as _;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

pub async fn async_main() -> Result<()> {
    // Generate peer id
    let keys = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(keys.public());
    info!("Peer Id: {}", peer_id.clone());

    let (response_sender, mut response_rcv) = mpsc::unbounded_channel();

    let auth_keys = Keypair::<X25519Spec>::new()
        .into_authentic(&KEYS)
        .expect("can create auth keys");

    // Create a transport
    let transport = TokioTcpConfig::new()
        .upgrade(upgrade::Version::V1)
        .authenticate(NoiseConfig::xx(auth_keys).into_authenticated())
        .multiplex(mplex::MplexConfig::new())
        .boxed();

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use float_eq::assert_float_eq;
    use futures::stream::{self, StreamExt, TryStreamExt};
    use hyper::{
        body::{to_bytes, HttpBody},
        Request,
    };
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;
}

#[cfg(feature = "bench")]
pub(crate) mod bench {
    use super::*;
    use criterion::{black_box, Criterion};
    use futures::executor::block_on;
    use hyper::body::to_bytes;

    pub(crate) fn group(c: &mut Criterion) {}
}
