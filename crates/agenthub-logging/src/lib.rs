use std::fs::OpenOptions;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, Utc};
use tracing_subscriber::EnvFilter;

pub type LogSpec = (PathBuf, String);

pub struct LogGuards {
    _writer: tracing_appender::non_blocking::WorkerGuard,
}

pub struct ActiveHourlyLogWriter {
    directory: PathBuf,
    file_name: String,
    current_path: PathBuf,
    current_hour_slot: i64,
    file: std::fs::File,
    now_utc: Box<dyn Fn() -> DateTime<Utc> + Send + Sync>,
}

impl ActiveHourlyLogWriter {
    const ROTATION_CANDIDATE_MAX_ATTEMPTS: usize = 1_000;

    pub fn new(directory: PathBuf, file_name: String) -> std::io::Result<Self> {
        Self::new_with_clock(directory, file_name, Box::new(Utc::now))
    }

    pub fn new_with_clock(
        directory: PathBuf,
        file_name: String,
        now_utc: Box<dyn Fn() -> DateTime<Utc> + Send + Sync>,
    ) -> std::io::Result<Self> {
        std::fs::create_dir_all(&directory)?;
        let now = now_utc();
        let current_hour_slot = Self::hour_slot(now);
        let current_path = directory.join(&file_name);
        let file = Self::open_append_file(&current_path)?;
        Ok(Self {
            directory,
            file_name,
            current_path,
            current_hour_slot,
            file,
            now_utc,
        })
    }

    fn open_append_file(path: &Path) -> std::io::Result<std::fs::File> {
        OpenOptions::new().create(true).append(true).open(path)
    }

    fn hour_slot(now: DateTime<Utc>) -> i64 {
        now.timestamp().div_euclid(3600)
    }

    fn slot_to_suffix(hour_slot: i64) -> String {
        let slot_epoch = hour_slot.saturating_mul(3600);
        let ts = Utc
            .timestamp_opt(slot_epoch, 0)
            .single()
            .unwrap_or_else(Utc::now);
        ts.format("%Y-%m-%d-%H").to_string()
    }

    fn rotate_destination_with_limit(
        &self,
        hour_slot: i64,
        max_attempts: usize,
    ) -> std::io::Result<PathBuf> {
        let suffix = Self::slot_to_suffix(hour_slot);
        let base = self
            .directory
            .join(format!("{}.{}", self.file_name, suffix));
        if !base.exists() {
            return Ok(base);
        }
        for idx in 1..=max_attempts {
            let candidate = self
                .directory
                .join(format!("{}.{}.{}", self.file_name, suffix, idx));
            if !candidate.exists() {
                return Ok(candidate);
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "failed to allocate rotated log path under {} after {} attempts",
                self.directory.display(),
                max_attempts
            ),
        ))
    }

    fn rotate_destination(&self, hour_slot: i64) -> std::io::Result<PathBuf> {
        self.rotate_destination_with_limit(hour_slot, Self::ROTATION_CANDIDATE_MAX_ATTEMPTS)
    }

    fn rotate_if_needed(&mut self) -> std::io::Result<()> {
        let now_slot = Self::hour_slot((self.now_utc)());
        if now_slot == self.current_hour_slot {
            return Ok(());
        }

        self.file.flush()?;
        let replacement = Self::open_append_file(&self.current_path)?;
        let previous_file = std::mem::replace(&mut self.file, replacement);
        drop(previous_file);

        if self.current_path.exists() {
            let rotated_path = self.rotate_destination(self.current_hour_slot)?;
            std::fs::rename(&self.current_path, rotated_path)?;
        }

        self.file = Self::open_append_file(&self.current_path)?;
        self.current_hour_slot = now_slot;
        Ok(())
    }
}

impl std::io::Write for ActiveHourlyLogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.rotate_if_needed()?;
        self.file.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

pub fn split_log_path(path: &str) -> LogSpec {
    let path_buf = Path::new(path);
    if path_buf.extension().is_some() {
        let file_name = path_buf
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("agenthub.log")
            .to_string();
        let dir = path_buf.parent().unwrap_or_else(|| Path::new("."));
        return (dir.to_path_buf(), file_name);
    }
    (path_buf.to_path_buf(), "agenthub.log".to_string())
}

pub fn init_tracing(
    filter: EnvFilter,
    log_spec: Option<&LogSpec>,
) -> anyhow::Result<Option<LogGuards>> {
    if let Some((dir, file_name)) = log_spec {
        let appender = ActiveHourlyLogWriter::new(dir.clone(), file_name.clone())?;
        let (writer, guard) = tracing_appender::non_blocking(appender);
        let _ = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(writer)
            .with_ansi(false)
            .with_target(true)
            .try_init();
        return Ok(Some(LogGuards { _writer: guard }));
    }
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .try_init();
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use std::io::Write;
    use std::sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    };
    use tracing_subscriber::EnvFilter;

    #[test]
    fn split_log_path_detects_file_and_directory_inputs() {
        let (dir, file) = split_log_path("/tmp/agenthub/service.log");
        assert!(dir.ends_with("/tmp/agenthub"));
        assert_eq!(file, "service.log");

        let (dir, file) = split_log_path("/tmp/agenthub/logs");
        assert!(dir.ends_with("/tmp/agenthub/logs"));
        assert_eq!(file, "agenthub.log");
    }

    #[test]
    fn init_tracing_supports_stdout_and_file_targets() {
        let stdout_guard =
            init_tracing(EnvFilter::new("info"), None).expect("init tracing for stdout");
        assert!(stdout_guard.is_none());

        let unique = format!(
            "agenthub-app-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("duration since epoch")
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        let spec = (dir.clone(), "agenthub.log".to_string());
        let file_guard =
            init_tracing(EnvFilter::new("info"), Some(&spec)).expect("init tracing for file");
        assert!(file_guard.is_some());
        assert!(dir.exists());
        assert!(dir.join("agenthub.log").exists());
    }

    #[test]
    fn active_log_writer_keeps_latest_plain_and_rotates_with_suffix() {
        let unique = format!(
            "agenthub-log-rotate-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("duration since epoch")
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).expect("create temp log dir");

        let hour_slot_val = Arc::new(AtomicI64::new(48_000));
        let hour_slot_for_clock = Arc::clone(&hour_slot_val);
        let clock = Box::new(move || {
            let slot = hour_slot_for_clock.load(Ordering::Relaxed);
            Utc.timestamp_opt(slot.saturating_mul(3600), 0)
                .single()
                .expect("valid slot timestamp")
        });
        let mut writer =
            ActiveHourlyLogWriter::new_with_clock(dir.clone(), "agenthub.log".to_string(), clock)
                .expect("create writer");

        writer.write_all(b"line-1\n").expect("write first log line");

        let current_log = dir.join("agenthub.log");
        assert!(current_log.exists());
        let first_content = std::fs::read_to_string(&current_log).expect("read current log");
        assert_eq!(first_content, "line-1\n");

        let old_slot = hour_slot_val.load(Ordering::Relaxed);
        hour_slot_val.store(old_slot + 1, Ordering::Relaxed);
        writer
            .write_all(b"line-2\n")
            .expect("write second log line with rotation");

        let rotated = dir.join(format!(
            "agenthub.log.{}",
            ActiveHourlyLogWriter::slot_to_suffix(old_slot)
        ));
        assert!(rotated.exists());
        let rotated_content = std::fs::read_to_string(rotated).expect("read rotated log");
        assert_eq!(rotated_content, "line-1\n");

        let current_content = std::fs::read_to_string(current_log).expect("read current log");
        assert_eq!(current_content, "line-2\n");
    }

    #[test]
    fn rotate_destination_with_limit_returns_error_after_exhaustion() {
        let unique = format!(
            "agenthub-log-rotate-limit-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("duration since epoch")
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).expect("create temp log dir");

        let hour_slot_val = 48_001;
        let suffix = ActiveHourlyLogWriter::slot_to_suffix(hour_slot_val);
        let base = dir.join(format!("agenthub.log.{}", suffix));
        std::fs::write(&base, "x").expect("write base collision file");
        std::fs::write(dir.join(format!("agenthub.log.{}.1", suffix)), "x")
            .expect("write first collision file");
        std::fs::write(dir.join(format!("agenthub.log.{}.2", suffix)), "x")
            .expect("write second collision file");

        let writer = ActiveHourlyLogWriter::new(dir.clone(), "agenthub.log".to_string())
            .expect("create writer");
        let err = writer
            .rotate_destination_with_limit(hour_slot_val, 2)
            .expect_err("rotate destination should fail after hitting max attempts");
        assert_eq!(err.kind(), std::io::ErrorKind::AlreadyExists);
    }
}
