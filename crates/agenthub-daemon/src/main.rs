use std::process::ExitCode;

fn main() -> ExitCode {
    if let Some(code) = agenthub::run_executor_guardian_if_requested() {
        return code;
    }
    match agenthub_daemon::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            agenthub::report_cli_error(&err);
            ExitCode::FAILURE
        }
    }
}
