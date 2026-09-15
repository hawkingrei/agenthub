use std::process::ExitCode;

fn main() -> ExitCode {
    if let Some(code) = agenthub::run_executor_guardian_if_requested() {
        return code;
    }
    run()
}

#[tokio::main]
async fn run() -> ExitCode {
    match agenthub::run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            agenthub::report_cli_error(&err);
            ExitCode::FAILURE
        }
    }
}
