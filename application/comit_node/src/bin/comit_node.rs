#![warn(unused_extern_crates, missing_debug_implementations, rust_2018_idioms)]
#![deny(unsafe_code)]

#[macro_use]
extern crate log;

use comit_node::{
    comit_client,
    comit_server::ComitServer,
    http_api::route_factory,
    ledger_query_service::DefaultLedgerQueryServiceApiClient,
    logging,
    seed::Seed,
    settings::ComitNodeSettings,
    swap_protocols::{
        rfc003::{self, state_store::InMemoryStateStore},
        InMemoryMetadataStore, SwapId,
    },
};
use ethereum_support::*;
use futures::sync::{
    mpsc::{self, UnboundedSender},
    oneshot,
};
use std::{env::var, net::SocketAddr, sync::Arc, time::Duration};

// TODO: Make a nice command line interface here (using StructOpt f.e.) see #298
fn main() -> Result<(), failure::Error> {
    logging::set_up_logging();
    let settings = load_settings()?;

    // TODO: Maybe not print settings because of private keys?
    info!("Starting up with {:#?}", settings);

    let seed = settings.comit.secret_seed;
    let metadata_store = Arc::new(InMemoryMetadataStore::default());
    let state_store = Arc::new(InMemoryStateStore::default());
    let lqs_client = create_ledger_query_service_api_client(&settings);
    let comit_client_factory = Arc::new(comit_client::bam::BamClientPool::default());

    let mut runtime = tokio::runtime::Runtime::new()?;

    spawn_warp_instance(
        &settings,
        Arc::clone(&metadata_store),
        Arc::clone(&state_store),
        Arc::clone(&lqs_client),
        comit_client_factory,
        seed,
        &mut runtime,
    );

    let sender = spawn_bob_swap_request_handler_for_rfc003(
        Arc::clone(&metadata_store),
        Arc::clone(&state_store),
        Arc::clone(&lqs_client),
        seed,
        settings.ledger_query_service.bitcoin.poll_interval_secs,
        settings.ledger_query_service.ethereum.poll_interval_secs,
        &mut runtime,
    );

    spawn_comit_server(&settings, sender, &mut runtime);

    // Block the current thread.
    ::std::thread::park();
    Ok(())
}

fn load_settings() -> Result<ComitNodeSettings, config::ConfigError> {
    let comit_config_path = var_or_default("COMIT_NODE_CONFIG_PATH", "~/.config/comit_node".into());
    let run_mode_config = var_or_default("RUN_MODE", "development".into());
    let default_config = format!("{}/{}", comit_config_path.trim(), "default");
    let run_mode_config = format!("{}/{}", comit_config_path.trim(), run_mode_config);

    let settings = ComitNodeSettings::create(default_config, run_mode_config)?;
    Ok(settings)
}

fn create_ledger_query_service_api_client(
    settings: &ComitNodeSettings,
) -> Arc<DefaultLedgerQueryServiceApiClient> {
    Arc::new(DefaultLedgerQueryServiceApiClient::new(
        &settings.ledger_query_service.url,
    ))
}

fn spawn_warp_instance(
    settings: &ComitNodeSettings,
    metadata_store: Arc<InMemoryMetadataStore<SwapId>>,
    state_store: Arc<InMemoryStateStore<SwapId>>,
    lqs_client: Arc<DefaultLedgerQueryServiceApiClient>,
    comit_client_factory: Arc<comit_client::bam::BamClientPool>,
    seed: Seed,
    runtime: &mut tokio::runtime::Runtime,
) {
    use comit_node::http_api::rfc003::alice_spawner::AliceSpawner;
    let alice_spawner = AliceSpawner::new(
        settings.comit.remote_comit_node_url,
        lqs_client,
        comit_client_factory,
        Arc::new(seed),
        settings.ledger_query_service.bitcoin.poll_interval_secs,
        settings.ledger_query_service.ethereum.poll_interval_secs,
    );
    let routes = route_factory::create(metadata_store, state_store, Arc::new(alice_spawner), seed);

    let http_socket_address = SocketAddr::new(settings.http_api.address, settings.http_api.port);

    let server = warp::serve(routes).bind(http_socket_address);

    runtime.spawn(server);
}

#[allow(clippy::too_many_arguments)]
fn spawn_bob_swap_request_handler_for_rfc003(
    metadata_store: Arc<InMemoryMetadataStore<SwapId>>,
    state_store: Arc<InMemoryStateStore<SwapId>>,
    lqs_api_client: Arc<DefaultLedgerQueryServiceApiClient>,
    seed: Seed,
    bitcoin_poll_interval: Duration,
    ethereum_poll_interval: Duration,
    runtime: &mut tokio::runtime::Runtime,
) -> UnboundedSender<(
    SwapId,
    rfc003::bob::SwapRequestKind,
    oneshot::Sender<rfc003::bob::SwapResponseKind>,
)> {
    let (sender, receiver) = mpsc::unbounded();

    let bob_swap_request_handler = rfc003::bob::SwapRequestHandler {
        receiver,
        metadata_store,
        state_store,
        lqs_api_client,
        seed,
        bitcoin_poll_interval,
        ethereum_poll_interval,
    };

    runtime.spawn(bob_swap_request_handler.start());

    sender
}

fn spawn_comit_server(
    settings: &ComitNodeSettings,
    sender: UnboundedSender<(
        SwapId,
        rfc003::bob::SwapRequestKind,
        oneshot::Sender<rfc003::bob::SwapResponseKind>,
    )>,

    runtime: &mut tokio::runtime::Runtime,
) {
    let server = ComitServer::new(sender);

    runtime.spawn(server.listen(settings.comit.comit_listen).map_err(|e| {
        error!("ComitServer shutdown: {:?}", e);
    }));
}

fn var_or_default(name: &str, default: String) -> String {
    match var(name) {
        Ok(value) => {
            info!("Set {}={}", name, value);
            value
        }
        Err(_) => {
            eprintln!(
                "{} is not set, falling back to default: '{}' ",
                name, default
            );
            default
        }
    }
}
