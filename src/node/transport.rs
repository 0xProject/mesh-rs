use anyhow::{Context, Result};
use libp2p::{
    core::{muxing::StreamMuxerBox, upgrade, upgrade::SelectUpgrade},
    dns::DnsConfig,
    identity,
    mplex::MplexConfig,
    tcp::TokioTcpConfig,
    websocket::WsConfig,
    yamux::YamuxConfig,
    PeerId, Transport,
};
use libp2p_secio::SecioConfig;
use std::time::Duration;

pub(crate) type Libp2pTransport = libp2p::core::transport::Boxed<(PeerId, StreamMuxerBox)>;

/// Create a transport for TCP/IP and WebSockets over TCP/IP with Secio
/// encryption and either yamux or else mplex multiplexing.
pub(crate) async fn make_transport(peer_id_keys: identity::Keypair) -> Result<Libp2pTransport> {
    // TCP/IP transport using Tokio
    let tcp_transport = TokioTcpConfig::new().nodelay(true);

    // Add DNS support to the TCP transport (to resolve /dns*/ addresses)
    let tcp_dns_transport = DnsConfig::new(tcp_transport).context("Creating /dns/ transport")?;

    // Websocket transport over TCP/IP
    let ws_transport = WsConfig::new(tcp_dns_transport.clone());

    // TODO: wasm_ext transport in the WASM context?

    // Combine transports
    let transport = tcp_dns_transport.or_transport(ws_transport);

    // Use Secio as authentication/encryption layer
    let authenticator = SecioConfig::new(peer_id_keys.clone());

    // Use Yamux as preferred multiplexer, but support mplex too.
    let multiplexer = SelectUpgrade::new(
        YamuxConfig::default(), // Preferred multiplexer
        MplexConfig::default(),
    );

    // Add authentication and multiplexer upgrades
    let transport = transport
        .upgrade(upgrade::Version::V1)
        .authenticate(authenticator)
        .multiplex(multiplexer)
        .timeout(Duration::from_secs(20));

    Ok(transport.boxed())
}
