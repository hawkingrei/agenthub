#[cfg(target_os = "linux")]
use libc::{c_char, c_int, c_long, c_uint};

// V8 release archives used by Codex currently reference memfd_create directly.
// Older GNU cross sysroots used by release/prebuild jobs can miss that libc
// wrapper even though the kernel syscall exists, so export a small compatibility
// symbol from the final binary and forward it to the raw syscall.
#[cfg(target_os = "linux")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memfd_create(name: *const c_char, flags: c_uint) -> c_int {
    unsafe { libc::syscall(libc::SYS_memfd_create as c_long, name, flags) as c_int }
}
