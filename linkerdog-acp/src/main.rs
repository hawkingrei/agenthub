use anyhow::Result;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    linkerdog_acp::run_from_args(std::env::args_os()).await
}
