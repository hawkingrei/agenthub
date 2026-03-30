use std::io::{self, Write};

pub fn write_cli_error<W: Write>(mut writer: W, err: &anyhow::Error) -> io::Result<()> {
    writeln!(writer, "Error: {err}")?;
    let mut causes = err.chain().skip(1).peekable();
    if causes.peek().is_none() {
        return Ok(());
    }

    writeln!(writer)?;
    writeln!(writer, "Caused by:")?;
    for (idx, cause) in causes.enumerate() {
        writeln!(writer, "  {}: {}", idx + 1, cause)?;
    }
    Ok(())
}

pub fn report_cli_error(err: &anyhow::Error) {
    if let Some(clap_err) = err.downcast_ref::<clap::Error>() {
        let _ = clap_err.print();
        return;
    }
    let stderr = io::stderr();
    let mut handle = stderr.lock();
    let _ = write_cli_error(&mut handle, err);
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;

    use super::write_cli_error;

    #[test]
    fn write_cli_error_formats_single_error_without_cause_chain() {
        let err = anyhow!("team_id is required (flag or env fallback)");
        let mut output = Vec::new();

        write_cli_error(&mut output, &err).expect("render single error");

        let rendered = String::from_utf8(output).expect("utf8 output");
        assert_eq!(
            rendered,
            "Error: team_id is required (flag or env fallback)\n"
        );
        assert!(!rendered.contains("Caused by:"));
    }

    #[test]
    fn write_cli_error_formats_cause_chain() {
        let err = anyhow!("grpc unavailable").context("actor permission review control failed");
        let mut output = Vec::new();

        write_cli_error(&mut output, &err).expect("render chained error");

        let rendered = String::from_utf8(output).expect("utf8 output");
        assert!(rendered.contains("Error: actor permission review control failed"));
        assert!(rendered.contains("Caused by:\n  1: grpc unavailable"));
    }
}
