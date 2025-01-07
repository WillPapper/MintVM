mod jsonrpc;

#[tokio::main]
async fn main() {
    println!("MintVM started");
    jsonrpc::run_server().await.expect("Failed to start JSON-RPC server");
}
