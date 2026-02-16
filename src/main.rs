#[tokio::main]
async fn main() -> anyhow::Result<()> {
    agenthub::run().await
}
