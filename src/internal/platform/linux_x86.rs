//! Raw Linux x86_64 syscalls (syscall instruction, System V).
//! Clean wrappers: no libc, no std, only `core` + `alloc` at call sites.

use core::arch::asm;

// Write-path: reserved for `save()`. Not wired up yet.
#[allow(dead_code)]
const SYS_WRITE: usize = 1;
const SYS_CLOSE: usize = 3;
const SYS_FSTAT: usize = 5;
const SYS_MMAP: usize = 9;
const SYS_MUNMAP: usize = 11;
const SYS_OPENAT: usize = 257;

pub const O_RDONLY: i32 = 0;
// Write-path: reserved for `save()`.
#[allow(dead_code)]
pub const O_WRONLY: i32 = 1;
#[allow(dead_code)]
pub const O_CREAT: i32 = 0o100;
#[allow(dead_code)]
pub const O_TRUNC: i32 = 0o1000;

pub const PROT_READ: i32 = 0x1;
pub const MAP_PRIVATE: i32 = 0x02;
pub const MAP_FAILED: usize = usize::MAX;

#[inline(always)]
fn syscall3(n: usize, a: usize, b: usize, c: usize) -> isize {
    let r: isize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") n => r,
            in("rdi") a, in("rsi") b, in("rdx") c,
            out("rcx") _, out("r11") _,
            options(nostack, preserves_flags)
        );
    }
    r
}

#[inline(always)]
fn syscall4(n: usize, a: usize, b: usize, c: usize, d: usize) -> isize {
    let r: isize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") n => r,
            in("rdi") a, in("rsi") b, in("rdx") c, in("r10") d,
            out("rcx") _, out("r11") _,
            options(nostack, preserves_flags)
        );
    }
    r
}

#[inline(always)]
fn syscall6(
    n: usize, a: usize, b: usize, c: usize, d: usize, e: usize, f: usize,
) -> isize {
    let r: isize;
    unsafe {
        asm!(
            "syscall",
            inlateout("rax") n => r,
            in("rdi") a, in("rsi") b, in("rdx") c,
            in("r10") d, in("r8") e, in("r9") f,
            out("rcx") _, out("r11") _,
            options(nostack, preserves_flags)
        );
    }
    r
}

fn errno(r: isize) -> Result<usize, i32> {
    if r < 0 && r >= -4095 {
        Err(-r as i32)
    } else {
        Ok(r as usize)
    }
}

pub fn open(path: *const u8, flags: i32, mode: u32) -> Result<i32, i32> {
    // openat(AT_FDCWD = -100, path, flags, mode)
    let r = syscall4(
        SYS_OPENAT,
        (-100isize) as usize,
        path as usize,
        flags as usize,
        mode as usize,
    );
    errno(r).map(|v| v as i32)
}

pub fn close(fd: i32) -> Result<(), i32> {
    let r = syscall3(SYS_CLOSE, fd as usize, 0, 0);
    errno(r).map(|_| ())
}

// Write-path: reserved for `save()`.
#[allow(dead_code)]
pub fn write(fd: i32, buf: *const u8, len: usize) -> Result<usize, i32> {
    let r = syscall3(SYS_WRITE, fd as usize, buf as usize, len);
    errno(r)
}

/// fstat -> file size via st_size (offset 48 on x86_64).
pub fn file_size(fd: i32) -> Result<usize, i32> {
    let mut stat = [0u8; 144];
    let r = syscall3(SYS_FSTAT, fd as usize, stat.as_mut_ptr() as usize, 0);
    errno(r)?;
    Ok(usize::from_ne_bytes(stat[48..56].try_into().unwrap_or([0; 8])))
}

pub fn mmap(
    len: usize, prot: i32, flags: i32, fd: i32, offset: u64,
) -> Result<*mut u8, i32> {
    let r = syscall6(
        SYS_MMAP, 0, len, prot as usize, flags as usize, fd as usize,
        offset as usize,
    );
    if (r as usize) == MAP_FAILED || r < 0 {
        return Err(-r as i32);
    }
    errno(r).map(|v| v as *mut u8)
}

pub fn munmap(addr: *mut u8, len: usize) -> Result<(), i32> {
    let r = syscall3(SYS_MUNMAP, addr as usize, len, 0);
    errno(r).map(|_| ())
}

/// Write whole buffer, looping on partial writes.
// Write-path: reserved for `save()`.
#[allow(dead_code)]
pub fn write_all(fd: i32, mut buf: &[u8]) -> Result<(), i32> {
    while !buf.is_empty() {
        let n = write(fd, buf.as_ptr(), buf.len())?;
        if n == 0 {
            return Err(5); // EIO
        }
        buf = &buf[n..];
    }
    Ok(())
}
