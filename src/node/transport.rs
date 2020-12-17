//! Compose the transport stack for LibP2P
//!
//! TODO: Testnet memory transport
//! TODO: pnet private network for testing

use crate::prelude::*;
use libp2p::{
    bandwidth::BandwidthSinks,
    core::{
        either::EitherOutput, muxing::StreamMuxerBox, upgrade, upgrade::SelectUpgrade, UpgradeInfo,
    },
    dns::DnsConfig,
    identity, mplex, noise,
    tcp::TokioTcpConfig,
    websocket::WsConfig,
    yamux, PeerId, Transport, TransportExt,
};
use libp2p_secio as secio;
use std::{sync::Arc, time::Duration};

use upgrade::{MapInboundUpgrade, MapOutboundUpgrade};

pub(crate) type Libp2pTransport = libp2p::core::transport::Boxed<(PeerId, StreamMuxerBox)>;

/// Create a transport for TCP/IP and WebSockets over TCP/IP with Secio
/// encryption and either yamux or else mplex multiplexing.
pub(crate) fn make_transport(
    peer_id_keys: identity::Keypair,
) -> Result<(Libp2pTransport, Arc<BandwidthSinks>)> {
    // Create transport with TCP, DNS and WS
    // TODO: WASM support
    // TODO: Circuit-relay (waiting for upstream PR)
    let transport = {
        // TCP/IP transport using Tokio
        let tcp_transport = TokioTcpConfig::new().nodelay(true);

        // Add DNS support to the TCP transport (to resolve /dns*/ addresses)
        let tcp_dns_transport =
            DnsConfig::new(tcp_transport).context("Creating /dns/ transport")?;

        // Websocket transport over TCP/IP
        // TODO: Secure websocket.
        let ws_transport = WsConfig::new(tcp_dns_transport.clone());

        // Combine transports
        tcp_dns_transport.or_transport(ws_transport)
    };

    // Add bandwidth monitoring
    let (transport, bandwidth_logger) = transport.with_bandwidth_logging();

    // Create authenticator with Noise and Secio
    let authenticator = {
        // Noise legacy
        let mut noise_legacy = noise::LegacyConfig::default();
        noise_legacy.send_legacy_handshake = false;
        noise_legacy.recv_legacy_handshake = true;

        // Noise (with legacy)
        let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
            .into_authentic(&peer_id_keys)
            .context("Noise key generation")?;
        let mut noise_xx_config = noise::NoiseConfig::xx(noise_keys);
        noise_xx_config.set_legacy_config(noise_legacy);
        let noise = noise_xx_config.into_authenticated();

        // Secio
        // The Go version of 0x-mesh only supports Secio.
        let secio = secio::SecioConfig::new(peer_id_keys.clone());

        // We need to do some monad stack shuffling:
        // `Either<(A, B), (A, C)>` to `(A, Either<B, C>)`
        let upgrade = SelectUpgrade::new(noise, secio);
        let upgrade = MapInboundUpgrade::new(upgrade, |out| {
            match out {
                EitherOutput::First((peer_id, out)) => (peer_id, EitherOutput::First(out)),
                EitherOutput::Second((peer_id, out)) => (peer_id, EitherOutput::Second(out)),
            }
        });
        let upgrade = MapOutboundUpgrade::new(upgrade, |out| {
            match out {
                EitherOutput::First((peer_id, out)) => (peer_id, EitherOutput::First(out)),
                EitherOutput::Second((peer_id, out)) => (peer_id, EitherOutput::Second(out)),
            }
        });
        upgrade
    };
    info!("Authenticator: {:?}", authenticator.protocol_info());

    // Create multiplexer with yamux and mplex
    let multiplexer = {
        let mut yamux_config = yamux::YamuxConfig::default();
        // Update windows when data is consumed.
        yamux_config.set_window_update_mode(yamux::WindowUpdateMode::on_read());

        let mut mplex_config = mplex::MplexConfig::default();
        mplex_config.set_max_buffer_behaviour(mplex::MaxBufferBehaviour::Block);

        SelectUpgrade::new(yamux_config, mplex_config)
    };
    info!("Mutiplexer: {:?}", multiplexer.protocol_info());

    // TODO: Log the connection paths used

    let transport = transport
        .upgrade(upgrade::Version::V1)
        .authenticate(authenticator)
        .multiplex(multiplexer)
        .timeout(Duration::from_secs(20))
        .boxed();

    Ok((transport, bandwidth_logger))
}
