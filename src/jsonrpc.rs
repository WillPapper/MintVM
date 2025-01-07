// Exposes a JSON-RPC interface that is compatible with the Ethereum JSON-RPC API
// https://ethereum.org/en/developers/docs/apis/json-rpc/
// Will read data via sqlite.rs. See `src/sqlite.rs` for the current
// implementation.
mod jsonrpc;

use std::net::SocketAddr;
use std::time::Duration;

use hyper::body::Bytes;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::http_client::HttpClient;
use jsonrpsee::server::{RpcModule, Server};
use jsonrpsee::ws_client::WsClientBuilder;
use jsonrpsee::rpc_params;
use tokio::task;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tower_http::LatencyUnit;
use tracing_subscriber::util::SubscriberInitExt;

pub async fn run_server() -> anyhow::Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()?
        .add_directive("jsonrpsee[method_call{name = \"say_hello\"}]=trace".parse()?);
    tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(filter)
        .finish()
        .try_init()?;

    // Run both HTTP and WebSocket servers concurrently
    let http_addr = task::spawn(run_http_server());
    let ws_addr = task::spawn(run_ws_server());

    // Wait for both servers to start and print their addresses
    let http_addr = http_addr.await??;
    let ws_addr = ws_addr.await??;

    tracing::info!("HTTP server running on {}", http_addr);
    tracing::info!("WebSocket server running on {}", ws_addr);

    // Example HTTP client
    let http_client_url = format!("http://{}", http_addr);
    let middleware = tower::ServiceBuilder::new()
        .layer(
            TraceLayer::new_for_http()
                .on_request(|request: &hyper::Request<_>, _span: &tracing::Span| {
                    tracing::info!(request = ?request, "on_request")
                })
                .on_body_chunk(|chunk: &Bytes, latency: Duration, _: &tracing::Span| {
                    tracing::info!(size_bytes = chunk.len(), latency = ?latency, "sending body chunk")
                })
                .make_span_with(DefaultMakeSpan::new().include_headers(true))
                .on_response(DefaultOnResponse::new().include_headers(true).latency_unit(LatencyUnit::Micros)),
        );
    let http_client = HttpClient::builder()
        .set_http_middleware(middleware)
        .build(http_client_url)?;
    let response: Result<String, _> = http_client.request("say_hello", rpc_params![1_u64, 2, 3]).await;
    tracing::info!("HTTP client response: {:?}", response);

    // Example WebSocket client
    let ws_client_url = format!("ws://{}", ws_addr);
    let ws_client = WsClientBuilder::default().build(&ws_client_url).await?;
    let ws_response: String = ws_client.request("say_hello", rpc_params![]).await?;
    tracing::info!("WebSocket client response: {:?}", ws_response);

    Ok(())
}

async fn run_http_server() -> anyhow::Result<SocketAddr> {
    let server = Server::builder().build("127.0.0.1:0".parse::<SocketAddr>()?).await?;
    let mut module = RpcModule::new(());
    module.register_method("say_hello", |_, _, _| "Hello from HTTP!")?;

    let addr = server.local_addr()?;
    let handle = server.start(module);

    // Run HTTP server in the background
    tokio::spawn(handle.stopped());

    Ok(addr)
}

async fn run_ws_server() -> anyhow::Result<SocketAddr> {
    let server = Server::builder().build("127.0.0.1:0".parse::<SocketAddr>()?).await?;
    let mut module = RpcModule::new(());
    module.register_method("say_hello", |_, _, _| "Hello from WebSocket!")?;

    let addr = server.local_addr()?;
    let handle = server.start(module);

    // Run WebSocket server in the background
    tokio::spawn(handle.stopped());

    Ok(addr)
}