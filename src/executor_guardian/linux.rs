use std::ffi::OsString;
use std::future::Future;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::pin::Pin;
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use nix::errno::Errno;
use nix::fcntl::{FcntlArg, FdFlag, fcntl};
use nix::sys::prctl::set_child_subreaper;
use nix::sys::signal::{Signal, kill};
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::Pid;
use process_wrap::tokio::ChildWrapper;
use tokio::process::{Child, Command};

const CONTROL_FD_ENV: &str = "AGENTHUB_EXECUTOR_GUARDIAN_FD";
const CLEANED: u8 = 1;
const STOP: u8 = 2;
const POLL_INTERVAL: Duration = Duration::from_millis(20);

#[derive(Debug)]
pub(crate) struct GuardianChannel {
    control: UnixStream,
    inherited: UnixStream,
}

pub(crate) fn prepare(program: &str, args: &[String]) -> io::Result<(Command, GuardianChannel)> {
    let executable = guardian_executable()?;
    let (control, inherited) = UnixStream::pair()?;
    control.set_nonblocking(true)?;
    inherited.set_nonblocking(true)?;
    let fd = inherited.as_raw_fd();
    let mut command = Command::new(executable);
    command
        .arg(super::INTERNAL_COMMAND)
        .arg(program)
        .args(args)
        .env(CONTROL_FD_ENV, fd.to_string())
        .process_group(0);
    // SAFETY: fcntl is async-signal-safe; the channel owns this fd through spawn.
    unsafe {
        command.pre_exec(move || {
            fcntl(
                BorrowedFd::borrow_raw(fd),
                FcntlArg::F_SETFD(FdFlag::empty()),
            )?;
            Ok(())
        });
    }
    Ok((command, GuardianChannel { control, inherited }))
}

fn guardian_executable() -> io::Result<std::path::PathBuf> {
    let executable = std::env::current_exe()?;
    if matches!(
        executable.file_stem().and_then(|name| name.to_str()),
        Some("agenthub" | "agenthubd")
    ) {
        return Ok(executable);
    }
    #[cfg(test)]
    if let Some(executable) = crate::agenthub_binary::resolve_agenthub_binary_path() {
        return Ok(executable);
    }
    Err(io::Error::other(
        "could not resolve the executor guardian binary",
    ))
}

impl GuardianChannel {
    pub(crate) fn spawn(self, mut command: Command) -> io::Result<Box<dyn ChildWrapper>> {
        // Closing the control connection requests cleanup, including during daemon death.
        // kill_on_drop would kill the subreaper before it can collect detached descendants.
        command.kill_on_drop(false);
        let child = command.spawn()?;
        drop(self.inherited);
        Ok(Box::new(GuardianChild {
            child,
            control: self.control,
            stop_requested: AtomicBool::new(false),
            cleanup_verified: None,
        }))
    }
}

#[derive(Debug)]
struct GuardianChild {
    child: Child,
    control: UnixStream,
    stop_requested: AtomicBool,
    cleanup_verified: Option<bool>,
}

impl GuardianChild {
    fn request_stop(&self) -> io::Result<()> {
        if self.stop_requested.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        match (&self.control).write_all(&[STOP]) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.stop_requested.store(false, Ordering::Release);
                Err(error)
            }
        }
    }

    fn verify_cleanup(&mut self) -> io::Result<()> {
        if self.cleanup_verified.is_none() {
            let mut receipt = [0];
            self.cleanup_verified =
                Some(matches!(self.control.read(&mut receipt), Ok(1)) && receipt[0] == CLEANED);
        }
        if self.cleanup_verified == Some(true) {
            Ok(())
        } else {
            Err(io::Error::other(
                "executor guardian exited without cleanup evidence",
            ))
        }
    }
}

impl ChildWrapper for GuardianChild {
    fn inner(&self) -> &dyn ChildWrapper {
        &self.child
    }

    fn inner_mut(&mut self) -> &mut dyn ChildWrapper {
        &mut self.child
    }

    fn into_inner(self: Box<Self>) -> Box<dyn ChildWrapper> {
        Box::new(self.child)
    }

    fn start_kill(&mut self) -> io::Result<()> {
        self.request_stop()
    }

    fn signal(&self, _signal: i32) -> io::Result<()> {
        self.request_stop()
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        let status = self.child.try_wait()?;
        if status.is_some() {
            self.verify_cleanup()?;
        }
        Ok(status)
    }

    fn wait(&mut self) -> Pin<Box<dyn Future<Output = io::Result<ExitStatus>> + Send + '_>> {
        Box::pin(async move {
            let status = self.child.wait().await?;
            self.verify_cleanup()?;
            Ok(status)
        })
    }
}

pub(super) fn run(args: Vec<OsString>) -> io::Result<u8> {
    let fd: i32 = std::env::var(CONTROL_FD_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|fd| *fd >= 3)
        .ok_or_else(|| io::Error::other("missing guardian control descriptor"))?;
    // SAFETY: this internal entrypoint takes sole ownership of the inherited descriptor.
    // Validate it before constructing the owner, and prevent provider inheritance.
    fcntl(unsafe { BorrowedFd::borrow_raw(fd) }, FcntlArg::F_GETFD)?;
    let mut control = unsafe { UnixStream::from_raw_fd(fd) };
    fcntl(&control, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC))?;
    control.set_nonblocking(true)?;
    set_child_subreaper(true)?;
    let Some((program, args)) = args.split_first() else {
        return Err(io::Error::other("missing executor command"));
    };
    let mut command = std::process::Command::new(program);
    command
        .args(args)
        .env_remove(CONTROL_FD_ENV)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .process_group(0);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(_) => {
            // No provider process exists, so startup failure can be safely retried.
            control.write_all(&[CLEANED])?;
            return Ok(125);
        }
    };
    let code = loop {
        if let Some(status) = child.try_wait()? {
            break status
                .code()
                .unwrap_or_else(|| 128 + status.signal().unwrap_or(1)) as u8;
        }
        let mut request = [0];
        match control.read(&mut request) {
            Ok(_) => break 137,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => break 137,
        }
        std::thread::sleep(POLL_INTERVAL);
    };
    reap_descendants()?;
    // A disconnected daemon still gets cleanup; only a live receiver needs the receipt.
    match control.write_all(&[CLEANED]) {
        Ok(()) => {}
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
            ) => {}
        Err(error) => return Err(error),
    }
    Ok(code)
}

fn reap_descendants() -> io::Result<()> {
    let children_path = format!("/proc/self/task/{}/children", std::process::id());
    loop {
        let children = std::fs::read_to_string(&children_path)?;
        // Each PID remains our unreaped child until the wait loop below. This avoids
        // numeric PID/process-group reuse while recursively adopting escaped descendants.
        for pid in children.split_whitespace() {
            let pid = pid.parse::<i32>().map_err(io::Error::other)?;
            match kill(Pid::from_raw(pid), Signal::SIGKILL) {
                Ok(()) | Err(Errno::ESRCH) => {}
                Err(error) => return Err(error.into()),
            }
        }
        loop {
            match waitpid(Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::StillAlive) => break,
                Ok(_) | Err(Errno::EINTR) => {}
                Err(Errno::ECHILD) => return Ok(()),
                Err(error) => return Err(error.into()),
            }
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[cfg(test)]
mod tests;
