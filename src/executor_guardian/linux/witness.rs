//! A locked, durable witness for one reservation. Absence is never cleanup evidence.

use std::fs::{DirBuilder, File, OpenOptions, TryLockError};
use std::io;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd};
use std::os::unix::fs::{DirBuilderExt, FileExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use agenthub_agent_domain::loop_runtime::LoopReservation;
use agenthub_db::native_sessions::NativeExecutionOwner;
use nix::fcntl::{FcntlArg, FdFlag, fcntl};
use sha2::{Digest, Sha256};

pub(super) const FD_ENV: &str = "AGENTHUB_EXECUTOR_WITNESS_FD";
const PREPARED: u8 = b'P';
const STARTED: u8 = b'S';
const CLEANED: u8 = b'C';
const LENGTH: usize = 33;

#[derive(Debug)]
pub(crate) struct CleanupWitness {
    file: File,
}

fn identity(reservation: &LoopReservation) -> [u8; 32] {
    let key = serde_json::to_vec(&(
        "guardian-witness-v1",
        &reservation.team_id,
        &reservation.actor_id,
        &reservation.activation_id,
        &reservation.owner_id,
        reservation.generation,
    ))
    .expect("reservation identity is serializable");
    Sha256::digest(key).into()
}

fn path(base: &Path, reservation: &LoopReservation) -> PathBuf {
    identity_path(base, &identity(reservation))
}

fn standalone_identity(owner: &NativeExecutionOwner) -> [u8; 32] {
    Sha256::digest(
        serde_json::to_vec(&(
            "guardian-standalone-v1",
            &owner.agent_id,
            &owner.local_session_id,
            &owner.owner_id,
            owner.generation,
        ))
        .expect("native owner identity is serializable"),
    )
    .into()
}

fn identity_path(base: &Path, identity: &[u8; 32]) -> PathBuf {
    use std::fmt::Write;
    let mut name = String::with_capacity(64);
    for byte in identity {
        write!(&mut name, "{byte:02x}").expect("String write");
    }
    base.join(".executor-recovery").join(name)
}

impl CleanupWitness {
    pub(crate) fn prepare(base: &Path, reservation: &LoopReservation) -> io::Result<Self> {
        Self::prepare_identity(base, &identity(reservation))
    }

    pub(crate) fn prepare_standalone(
        base: &Path,
        owner: &NativeExecutionOwner,
    ) -> io::Result<Self> {
        Self::prepare_identity(base, &standalone_identity(owner))
    }

    fn prepare_identity(base: &Path, identity: &[u8; 32]) -> io::Result<Self> {
        // Event databases are created lazily; the first guarded launch may precede all output.
        std::fs::create_dir_all(base)?;
        let path = identity_path(base, identity);
        let directory = path.parent().expect("witness directory");
        match DirBuilder::new().mode(0o700).create(directory) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let metadata = std::fs::symlink_metadata(directory)?;
        if !metadata.is_dir() || metadata.mode() & 0o077 != 0 {
            return Err(io::Error::other(
                "executor recovery directory must be private",
            ));
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(&path)?;
        file.try_lock().map_err(io::Error::other)?;
        let mut contents = [PREPARED; LENGTH];
        contents[1..].copy_from_slice(identity);
        file.write_all_at(&contents, 0)?;
        file.sync_all()?;
        File::open(directory)?.sync_all()?;
        Ok(Self { file })
    }

    pub(super) fn descriptor(&self) -> i32 {
        self.file.as_raw_fd()
    }

    pub(super) fn inherit() -> io::Result<Option<Self>> {
        let Some(value) = std::env::var_os(FD_ENV) else {
            return Ok(None);
        };
        let fd = value
            .to_str()
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|fd| *fd >= 3)
            .ok_or_else(|| io::Error::other("invalid witness descriptor"))?;
        // SAFETY: the internal guardian entrypoint owns the inherited descriptor exclusively.
        fcntl(unsafe { BorrowedFd::borrow_raw(fd) }, FcntlArg::F_GETFD)?;
        let file = unsafe { File::from_raw_fd(fd) };
        fcntl(&file, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC))?;
        Ok(Some(Self { file }))
    }

    pub(super) fn mark_started(&self) -> io::Result<()> {
        self.file.write_all_at(&[STARTED], 0)?;
        self.file.sync_data()
    }

    pub(super) fn mark_cleaned(&self) -> io::Result<()> {
        self.file.write_all_at(&[CLEANED], 0)?;
        self.file.sync_data()
    }

    /// Keep this lock alive through the database CAS. A launcher or guardian still owning the
    /// original open-file description prevents recovery, including the pre-exec window.
    pub(crate) fn verify(base: &Path, reservation: &LoopReservation) -> io::Result<Option<Self>> {
        Self::verify_identity(base, &identity(reservation))
    }

    pub(crate) fn verify_standalone(
        base: &Path,
        owner: &NativeExecutionOwner,
    ) -> io::Result<Option<Self>> {
        Self::verify_identity(base, &standalone_identity(owner))
    }

    fn verify_identity(base: &Path, identity: &[u8; 32]) -> io::Result<Option<Self>> {
        let file = match OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(identity_path(base, identity))
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.mode() & 0o077 != 0 || metadata.len() != LENGTH as u64 {
            return Ok(None);
        }
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Ok(None),
            Err(TryLockError::Error(error)) => return Err(error),
        }
        let mut contents = [0; LENGTH];
        file.read_exact_at(&mut contents, 0)?;
        if !matches!(contents[0], PREPARED | CLEANED) || contents[1..] != *identity {
            return Ok(None);
        }
        Ok(Some(Self { file }))
    }

    pub(crate) fn retire(base: &Path, reservation: &LoopReservation) {
        Self::retire_path(path(base, reservation));
    }

    pub(crate) fn retire_standalone(base: &Path, owner: &NativeExecutionOwner) {
        Self::retire_path(identity_path(base, &standalone_identity(owner)));
    }

    fn retire_path(path: PathBuf) {
        if let Err(error) = std::fs::remove_file(path)
            && error.kind() != io::ErrorKind::NotFound
        {
            tracing::warn!(%error, "could not retire executor cleanup witness");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn standalone_witness_fences_every_owner_dimension_and_loop_namespace() {
        let (directory, reservation) = super::super::tests::recovery_fixture();
        let owner = NativeExecutionOwner {
            agent_id: reservation.actor_id.clone(),
            local_session_id: "local".into(),
            owner_id: reservation.owner_id.clone(),
            generation: reservation.generation,
        };
        let witness = CleanupWitness::prepare_standalone(&directory, &owner).unwrap();
        assert!(
            CleanupWitness::verify_standalone(&directory, &owner)
                .unwrap()
                .is_none()
        );
        assert!(
            CleanupWitness::verify(&directory, &reservation)
                .unwrap()
                .is_none()
        );
        for field in 0..4 {
            let mut other = owner.clone();
            match field {
                0 => other.agent_id = "other".into(),
                1 => other.local_session_id = "other".into(),
                2 => other.owner_id = "other".into(),
                _ => other.generation += 1,
            }
            assert!(
                CleanupWitness::verify_standalone(&directory, &other)
                    .unwrap()
                    .is_none()
            );
        }
        witness.mark_started().unwrap();
        drop(witness);
        assert!(
            CleanupWitness::verify_standalone(&directory, &owner)
                .unwrap()
                .is_none()
        );
        CleanupWitness::retire_standalone(&directory, &owner);
        let witness = CleanupWitness::prepare_standalone(&directory, &owner).unwrap();
        witness.mark_cleaned().unwrap();
        drop(witness);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let proof = loop {
            if let Some(proof) = CleanupWitness::verify_standalone(&directory, &owner).unwrap() {
                break proof;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert!(
            CleanupWitness::verify_standalone(&directory, &owner)
                .unwrap()
                .is_none()
        );
        CleanupWitness::retire_standalone(&directory, &owner);
        drop(proof);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn guardian_recovery_rejects_corruption_identity_replacement_and_symlinks() {
        let (directory, reservation) = super::super::tests::recovery_fixture();
        let witness = CleanupWitness::prepare(&directory, &reservation).unwrap();
        drop(witness);
        let path = path(&directory, &reservation);
        let original = std::fs::read(&path).unwrap();
        for contents in [
            vec![],
            vec![b'C'],
            {
                let mut replaced = original.clone();
                replaced[1] ^= 1;
                replaced
            },
            {
                let mut extended = original.clone();
                extended.push(0);
                extended
            },
        ] {
            std::fs::write(&path, contents).unwrap();
            assert!(
                CleanupWitness::verify(&directory, &reservation)
                    .unwrap()
                    .is_none()
            );
        }
        std::fs::write(&path, &original).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            CleanupWitness::verify(&directory, &reservation)
                .unwrap()
                .is_none()
        );
        std::fs::remove_file(&path).unwrap();
        let replacement = directory.join("replacement");
        std::fs::write(&replacement, original).unwrap();
        symlink(&replacement, &path).unwrap();
        assert!(CleanupWitness::verify(&directory, &reservation).is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }
}
