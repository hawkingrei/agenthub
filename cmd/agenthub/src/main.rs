use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    match agenthub::run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            agenthub::report_cli_error(&err);
            ExitCode::FAILURE
        }
    }
}
