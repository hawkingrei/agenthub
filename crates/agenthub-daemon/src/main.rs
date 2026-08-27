use std::process::ExitCode;

fn main() -> ExitCode {
    match agenthub_daemon::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            agenthub::report_cli_error(&err);
            ExitCode::FAILURE
        }
    }
}
