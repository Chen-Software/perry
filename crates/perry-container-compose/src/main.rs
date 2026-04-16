#[tokio::main]
async fn main() -> anyhow::Result<()> {
    perry_container_compose::cli::run().await
}
