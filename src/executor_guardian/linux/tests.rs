use super::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub(super) fn recovery_fixture() -> (
    std::path::PathBuf,
    agenthub_agent_domain::loop_runtime::LoopReservation,
) {
    let directory =
        std::env::temp_dir().join(format!("guardian-recovery-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let reservation = agenthub_agent_domain::loop_runtime::LoopReservation {
        actor_id: "worker".into(),
        team_id: "team".into(),
        activation_id: Some("activation".into()),
        owner_id: "old-daemon".into(),
        generation: 1,
        lease_expires_at: 160,
        lease_seconds: 60,
        renewal_seconds: 15,
        session_id: None,
        created_at: 100,
    };
    (directory, reservation)
}

fn wait_for_witness(
    directory: &std::path::Path,
    reservation: &agenthub_agent_domain::loop_runtime::LoopReservation,
) -> CleanupWitness {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(witness) = CleanupWitness::verify(directory, reservation).unwrap() {
            return witness;
        }
        // Parallel process tests can briefly inherit a CLOEXEC fd between fork and exec.
        assert!(
            std::time::Instant::now() < deadline,
            "cleanup witness stayed locked or invalid"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn guardian_recovery_requires_exclusive_matching_complete_witness() {
    let (directory, reservation) = recovery_fixture();
    assert!(
        CleanupWitness::verify(&directory, &reservation)
            .unwrap()
            .is_none()
    );
    let witness = CleanupWitness::prepare(&directory, &reservation).unwrap();
    assert!(
        CleanupWitness::verify(&directory, &reservation)
            .unwrap()
            .is_none()
    );
    drop(witness);
    let witness = wait_for_witness(&directory, &reservation);
    let mut stale = reservation.clone();
    stale.generation += 1;
    assert!(
        CleanupWitness::verify(&directory, &stale)
            .unwrap()
            .is_none()
    );
    witness.mark_started().unwrap();
    drop(witness);
    // An unlocked file left by a killed guardian cannot prove descendant cleanup.
    assert!(
        CleanupWitness::verify(&directory, &reservation)
            .unwrap()
            .is_none()
    );
    CleanupWitness::retire(&directory, &reservation);
    let witness = CleanupWitness::prepare(&directory, &reservation).unwrap();
    witness.mark_cleaned().unwrap();
    drop(witness);
    drop(wait_for_witness(&directory, &reservation));
    CleanupWitness::retire(&directory, &reservation);
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn guardian_recovery_survives_daemon_disconnect_and_reaps_detached_children() {
    let (directory, reservation) = recovery_fixture();
    let witness = std::sync::Arc::new(CleanupWitness::prepare(&directory, &reservation).unwrap());
    let (mut command, mut channel) = prepare(
        "/bin/sh",
        &[
            "-c".into(),
            "setsid /bin/sh -c 'echo $$; exec sleep 60' & wait".into(),
        ],
    )
    .unwrap();
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    channel.attach_witness(&mut command, witness);
    let mut child = channel.spawn(command).unwrap();
    let pid = descendant_pid(child.as_mut()).await;
    assert!(
        CleanupWitness::verify(&directory, &reservation)
            .unwrap()
            .is_none()
    );
    let mut raw = child.into_inner();
    wait(raw.as_mut()).await.unwrap();
    assert_reaped(pid);
    let proof = wait_for_witness(&directory, &reservation);
    assert!(
        CleanupWitness::verify(&directory, &reservation)
            .unwrap()
            .is_none()
    );
    drop(proof);
    drop(wait_for_witness(&directory, &reservation));
    std::fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
async fn guardian_recovery_witness_descriptor_never_reaches_provider() {
    let (directory, reservation) = recovery_fixture();
    let witness = std::sync::Arc::new(CleanupWitness::prepare(&directory, &reservation).unwrap());
    let fd = witness.descriptor();
    let (mut command, mut channel) = prepare(
        "/bin/sh",
        &[
            "-c".into(),
            "test -z \"$AGENTHUB_EXECUTOR_WITNESS_FD\" && test ! -e /proc/self/fd/\"$EXPECTED_FD\""
                .into(),
        ],
    )
    .unwrap();
    command.env("EXPECTED_FD", fd.to_string());
    channel.attach_witness(&mut command, witness);
    let mut child = channel.spawn(command).unwrap();
    assert!(wait(child.as_mut()).await.unwrap().success());
    drop(wait_for_witness(&directory, &reservation));
    std::fs::remove_dir_all(directory).unwrap();
}

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
