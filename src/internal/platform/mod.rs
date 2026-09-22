//! Private platform backends: raw syscalls, no libc, no std.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub(crate) mod linux_x86;
