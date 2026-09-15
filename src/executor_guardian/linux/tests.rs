use super::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

fn guarded_shell(script: &str) -> Box<dyn ChildWrapper> {
    let (mut command, channel) = prepare("/bin/sh", &["-c".into(), script.into()]).unwrap();
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    channel.spawn(command).unwrap()
}

async fn descendant_pid(child: &mut dyn ChildWrapper) -> u32 {
    let mut output = BufReader::new(child.stdout().take().unwrap());
    let mut pid = String::new();
    tokio::time::timeout(Duration::from_secs(5), output.read_line(&mut pid))
        .await
        .unwrap()
        .unwrap();
    pid.trim().parse().unwrap()
}

async fn wait(child: &mut dyn ChildWrapper) -> io::Result<ExitStatus> {
    tokio::time::timeout(Duration::from_secs(5), child.wait())
        .await
        .unwrap()
}

fn assert_reaped(pid: u32) {
    assert!(
        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "descendant {pid} was not reaped"
    );
}

#[tokio::test]
async fn guardian_preserves_stdio_and_provider_exit_status() {
    let mut child = guarded_shell("read -r line; printf '%s' \"$line\"; printf error >&2; exit 7");
    child
        .stdin()
        .as_mut()
        .unwrap()
        .write_all(b"provider response\n")
        .await
        .unwrap();
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        Box::into_pin(child.wait_with_output()),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stdout, b"provider response");
    assert_eq!(output.stderr, b"error");
}

#[tokio::test]
async fn guardian_reaps_detached_descendants_after_provider_exit() {
    let mut child =
        guarded_shell("setsid /bin/sh -c 'echo $$; exec sleep 60' & read -r line; exit 0");
    let pid = descendant_pid(child.as_mut()).await;
    child
        .stdin()
        .as_mut()
        .unwrap()
        .write_all(b"exit\n")
        .await
        .unwrap();
    assert!(wait(child.as_mut()).await.unwrap().success());
    assert_reaped(pid);
}

#[tokio::test]
async fn guardian_stop_reaps_detached_descendants() {
    let mut child = guarded_shell("setsid /bin/sh -c 'echo $$; exec sleep 60' & wait");
    let pid = descendant_pid(child.as_mut()).await;
    child.start_kill().unwrap();
    child.start_kill().unwrap();
    assert_eq!(wait(child.as_mut()).await.unwrap().code(), Some(137));
    assert_reaped(pid);
    assert_eq!(child.try_wait().unwrap().unwrap().code(), Some(137));
}

#[tokio::test]
async fn guardian_control_disconnect_reaps_detached_descendants() {
    let mut child = guarded_shell("setsid /bin/sh -c 'echo $$; exec sleep 60' & wait");
    let pid = descendant_pid(child.as_mut()).await;
    // Unwrapping closes the daemon's control endpoint, as daemon death would do.
    let mut raw_child = child.into_inner();
    wait(raw_child.as_mut()).await.unwrap();
    assert_reaped(pid);
}

#[tokio::test]
async fn guardian_missing_receipt_never_establishes_cleanup() {
    let (control, peer) = UnixStream::pair().unwrap();
    control.set_nonblocking(true).unwrap();
    drop(peer);
    let mut child = GuardianChild {
        child: Command::new("/bin/true").spawn().unwrap(),
        control,
        stop_requested: AtomicBool::new(false),
        cleanup_verified: None,
    };
    let error = wait(&mut child).await.unwrap_err();
    assert!(error.to_string().contains("without cleanup evidence"));
    assert!(child.try_wait().is_err());
    assert!(child.wait().await.is_err());
}

#[tokio::test]
async fn guardian_failed_provider_spawn_has_cleanup_evidence() {
    let (command, channel) = prepare("/agenthub-test-missing-provider", &[]).unwrap();
    let mut child = channel.spawn(command).unwrap();
    assert_eq!(wait(child.as_mut()).await.unwrap().code(), Some(125));
}

#[tokio::test]
async fn guardian_never_lends_control_descriptor_to_provider() {
    let (mut command, channel) = prepare("/bin/sh", &["-c".into(), "test -z \"$AGENTHUB_EXECUTOR_GUARDIAN_FD\" && test ! -e /proc/self/fd/\"$EXPECTED_FD\"".into()]).unwrap();
    command.env("EXPECTED_FD", channel.inherited.as_raw_fd().to_string());
    let mut child = channel.spawn(command).unwrap();
    assert!(wait(child.as_mut()).await.unwrap().success());
}
