use anyhow::{Context, Result};
use futures::prelude::*;
use libp2p::{
    core::{
        either::{EitherError, EitherOutput},
        muxing::StreamMuxerBox,
        upgrade,
        upgrade::SelectUpgrade,
    },
    dns::DnsConfig,
    identity, mplex, noise,
    tcp::TokioTcpConfig,
    websocket::WsConfig,
    yamux, InboundUpgradeExt, OutboundUpgradeExt, PeerId, Transport,
};
use libp2p_secio as secio;
use std::time::Duration;

pub(crate) type Libp2pTransport = libp2p::core::transport::Boxed<(PeerId, StreamMuxerBox)>;

/// Create a transport for TCP/IP and WebSockets over TCP/IP with Secio
/// encryption and either yamux or else mplex multiplexing.
pub(crate) async fn make_transport(peer_id_keys: identity::Keypair) -> Result<Libp2pTransport> {
    // Create transport with TCP, DNS and WS
    let transport = {
        // TCP/IP transport using Tokio
        let tcp_transport = TokioTcpConfig::new().nodelay(true);

        // Add DNS support to the TCP transport (to resolve /dns*/ addresses)
        let tcp_dns_transport =
            DnsConfig::new(tcp_transport).context("Creating /dns/ transport")?;

        // Websocket transport over TCP/IP
        let ws_transport = WsConfig::new(tcp_dns_transport.clone());

        // TODO: wasm_ext transport in the WASM context?
        // Combine transports
        tcp_dns_transport.or_transport(ws_transport)
    };

    // Create authenticator with Noise and Secio
    let transport = {
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
        let secio = secio::SecioConfig::new(peer_id_keys.clone());

        // We need to make the outputs of Noise and Secio match for SelectUpgrade
        // to work here. See for example Parity Substrate:
        // https://github.com/paritytech/substrate/blob/7b22fbfc6ba567af8ac0a693ef6593e179f46a53/client/network/src/transport.rs#L97
        // TODO: Make it work using upgrade builder
        let upgrade = SelectUpgrade::new(noise, secio);
        transport.and_then(move |stream, endpoint| {
            upgrade::apply(stream, upgrade, endpoint, upgrade::Version::V1).map(|out| {
                match out? {
                    // We negotiated noise
                    EitherOutput::First((remote_id, out)) => {
                        if false {
                            // TODO: This is just here to give the Err a type.
                            return Err(upgrade::UpgradeError::Apply(EitherError::A(
                                noise::NoiseError::InvalidKey,
                            )));
                        };
                        Ok((EitherOutput::First(out), remote_id))
                    }
                    // We negotiated secio
                    EitherOutput::Second((remote_id, out)) => {
                        Ok((EitherOutput::Second(out), remote_id))
                    }
                }
            })
        })
    };

    // Create multiplexer with yamux and mplex
    let transport = {
        let mut yamux_config = yamux::YamuxConfig::default();
        // Update windows when data is consumed.
        yamux_config.set_window_update_mode(yamux::WindowUpdateMode::on_read());

        let mut mplex_config = mplex::MplexConfig::default();
        mplex_config.set_max_buffer_behaviour(mplex::MaxBufferBehaviour::Block);

        transport.and_then(move |(stream, peer_id), endpoint| {
            let peer_id2 = peer_id.clone();
            let upgrade = SelectUpgrade::new(yamux_config, mplex_config)
                .map_inbound(move |muxer| (peer_id, muxer))
                .map_outbound(move |muxer| (peer_id2, muxer));
            upgrade::apply(stream, upgrade, endpoint, upgrade::Version::V1)
                .map_ok(|(id, muxer)| (id, StreamMuxerBox::new(muxer)))
        })
    };

    // TODO: Log the connection paths used

    Ok(transport.boxed())
}
