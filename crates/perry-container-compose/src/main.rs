#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    if let Err(e) = perry_container_compose::cli::run_cli().await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
