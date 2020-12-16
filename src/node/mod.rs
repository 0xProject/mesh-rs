mod behaviour;
mod order_sync;
mod transport;

use self::{behaviour::MyBehaviour, transport::make_transport};
use crate::prelude::*;
use libp2p::{identity, swarm::SwarmBuilder, PeerId, Swarm};

pub async fn run() -> Result<()> {
    // Generate peer id
    let peer_id_keys = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(peer_id_keys.public());
    info!("Peer Id: {}", peer_id.clone());

    // Create a transport
    let (transport, bandwidth_monitor) = make_transport(peer_id_keys.clone())
        .await
        .context("Creating libp2p transport")?;

    // Create node behaviour
    let behaviour = MyBehaviour::new(peer_id_keys)
        .await
        .context("Creating node behaviour")?;

    // Executor for connection background tasks.
    let executor = Box::new(|future| {
        debug!("Spawning background task");
        tokio::spawn(future);
    });

    // Create a Swarm to manage peers and events.
    let mut swarm: Swarm<MyBehaviour> = SwarmBuilder::new(transport, behaviour, peer_id)
        .executor(executor)
        .build();

    // Listen on all interfaces and whatever port the OS assigns
    Swarm::listen_on(
        &mut swarm,
        "/ip4/0.0.0.0/tcp/0"
            .parse()
            .context("Parsing listening address")?,
    )
    .context("Starting to listen")?;

    // Do something interesting
    swarm.search_random_peer();

    // Catch SIGTERM so the container can shutdown without an init process.
    let sigterm = tokio::signal::ctrl_c();
    tokio::pin!(sigterm);

    // Kick it off
    loop {
        tokio::select! {
            event = swarm.next() => {
                info!("New Event: {:?}", event);
            },
            _ = &mut sigterm => {
                info!("SIGTERM received, shutting down");
                // TODO: Shut down swarm?
                break;
            }
        }
    }
    info!("Done.");
    info!("Known peers: {:?}", swarm.known_peers());
    info!(
        "Bandwidth: {} inbound, {} outbound",
        bandwidth_monitor.total_inbound(),
        bandwidth_monitor.total_outbound()
    );
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use float_eq::assert_float_eq;
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
