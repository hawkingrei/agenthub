use std::io::IsTerminal;

use agenthub_acp_adapter::{Cli, run_with_cli, shutdown};
use clap::Parser;
use tokio::signal;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let result = tokio::select! {
        result = run_with_cli(cli) => result,
        _ = signal::ctrl_c() => {
            eprintln!("Received SIGINT, shutting down...");
            Ok(())
        }
        _ = async {
            #[cfg(unix)]
            {
                let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
                    .expect("failed to register SIGTERM handler");
                sigterm.recv().await;
            }
            #[cfg(not(unix))]
            {
                std::future::pending::<()>().await;
            }
        } => {
            eprintln!("Received SIGTERM, shutting down...");
            Ok(())
        }
    };

    shutdown();

    if let Err(err) = result {
        eprintln!("Error: {err}");

        if std::io::stdin().is_terminal() {
            eprintln!("\nFor debugging, run with --diagnostic to log to a file.");
            eprintln!("Or use -v/-vv/-vvv for more verbose logging.");
        }

        std::process::exit(1);
    }

    Ok(())
}
