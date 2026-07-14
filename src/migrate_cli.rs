//! `agenthub migrate` — one-shot maintenance commands.
//!
//! Currently this stages existing `team_conversation_messages` bodies into the tiered message body
//! store while retaining the SQLite compatibility copy. It also restores any legacy sentinel rows
//! written by the earlier move-out implementation. It is safe to re-run from its durable checkpoint.

use clap::{CommandFactory, Parser, error::ErrorKind};

#[derive(Debug, Parser)]
#[command(
    name = "migrate",
    bin_name = "agenthub migrate",
    about = "Backfill existing conversation message bodies into the tiered body store.",
    disable_help_subcommand = true
)]
struct MigrateCli {
    /// Number of rows to stage per transaction.
    #[arg(long, value_name = "N", default_value_t = 500)]
    batch_size: usize,
    /// Report how many rows need restoration or staging without changing anything.
    #[arg(long)]
    dry_run: bool,
}

fn render_help() -> anyhow::Result<()> {
    let mut command = MigrateCli::command();
    command.print_long_help()?;
    Ok(())
}

pub(crate) async fn run_from_args(args: &[String]) -> anyhow::Result<()> {
    if matches!(args, [arg] if arg.trim().eq_ignore_ascii_case("help")) {
        return render_help();
    }

    let argv = std::iter::once("migrate".to_string()).chain(args.iter().cloned());
    let cli = match MigrateCli::try_parse_from(argv) {
        Ok(cli) => cli,
        Err(err) if err.kind() == ErrorKind::DisplayHelp => {
            err.print()?;
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };

    let (config, _info) = agenthub_config::AppConfig::load_with_info()?;
    let pool = agenthub_db::init_db().await?;

    let remaining = crate::team::count_pending_conversation_body_migration(&pool).await?;
    if cli.dry_run {
        println!(
            "[dry-run] {remaining} conversation message body/bodies would be restored or staged"
        );
        return Ok(());
    }

    if remaining == 0 {
        println!("no conversation message bodies need restoration or staging");
        return Ok(());
    }

    let Some(store) = crate::message_body_store::init_body_store(&config) else {
        anyhow::bail!(
            "the message body store is not available, so there is nowhere to migrate bodies to. \
             Build with the `rocksdb` feature and keep `message_body_store` enabled in config, then \
             re-run `agenthub migrate`. Nothing was changed."
        );
    };

    let staged_at = chrono::Utc::now().timestamp();
    // Foreground maintenance command: run flat-out, no inter-batch throttle.
    let report = crate::team::migrate_conversation_bodies_into_store(
        &pool,
        store.as_ref(),
        cli.batch_size,
        staged_at,
        std::time::Duration::ZERO,
    )
    .await?;

    println!(
        "restored {} and staged {} conversation message body/bodies ({} confirmed in the body store)",
        report.restored, report.staged, report.drained
    );
    Ok(())
}
