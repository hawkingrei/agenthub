pub async fn run_from_args(args: &[String]) -> anyhow::Result<()> {
    agenthub_doctor_cli::run_from_args(args).await
}
