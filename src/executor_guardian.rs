//! Local executor cleanup evidence. The guardian owns descendants independently of process groups.

use std::process::ExitCode;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::{GuardianChannel, prepare};

const INTERNAL_COMMAND: &str = "internal-executor-guardian";

/// Dispatch before creating a runtime: the subreaper must remain single-threaded.
#[doc(hidden)]
pub fn run_executor_guardian_if_requested() -> Option<ExitCode> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(INTERNAL_COMMAND)) {
        return None;
    }
    #[cfg(target_os = "linux")]
    let result = linux::run(args.collect());
    #[cfg(not(target_os = "linux"))]
    let result: std::io::Result<u8> = Err(std::io::Error::other(
        "executor guardians currently require Linux",
    ));
    Some(match result {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("executor guardian failed: {error}");
            ExitCode::from(125)
        }
    })
}
