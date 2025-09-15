use libc::*;
use libc::{F_GETFL, F_SETFL, O_NONBLOCK};
use std::{io, os::fd::RawFd};

/// Level-triggered epoll.
pub const READ_FLAGS: u32 = (EPOLLIN) as u32;
pub const WRITE_FLAGS: u32 = (EPOLLOUT) as u32;

pub struct Epoll { fd: RawFd }

impl Epoll {
    pub fn new() -> io::Result<Self> {
        let fd = unsafe { epoll_create1(EPOLL_CLOEXEC) };
        if fd < 0 { return Err(io::Error::last_os_error()); }
        Ok(Self { fd })
    }

    pub fn add(&self, fd: RawFd, flags: u32, data: u64) -> io::Result<()> {
        let mut ev = epoll_event { events: flags, u64: data };
        let r = unsafe { epoll_ctl(self.fd, EPOLL_CTL_ADD, fd, &mut ev) };
        if r < 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
    }

    pub fn modf(&self, fd: RawFd, flags: u32, data: u64) -> io::Result<()> {
        let mut ev = epoll_event { events: flags, u64: data };
        let r = unsafe { epoll_ctl(self.fd, EPOLL_CTL_MOD, fd, &mut ev) };
        if r < 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
    }

    pub fn del(&self, fd: RawFd) -> io::Result<()> {
        let r = unsafe { epoll_ctl(self.fd, EPOLL_CTL_DEL, fd, std::ptr::null_mut()) };
        if r < 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
    }

    pub fn wait(&self, events: &mut [epoll_event], timeout_ms: isize) -> io::Result<usize> {
        let n = unsafe { epoll_wait(self.fd, events.as_mut_ptr(), events.len() as i32, timeout_ms as i32) };
        if n < 0 { Err(io::Error::last_os_error()) } else { Ok(n as usize) }
    }

    pub fn fd(&self) -> RawFd { self.fd }
}

impl Drop for Epoll { fn drop(&mut self) { unsafe { close(self.fd) }; } }

pub fn set_nonblocking(fd: RawFd) -> io::Result<()> {
    unsafe {
        let flags = fcntl(fd, F_GETFL);
        if flags < 0 { return Err(io::Error::last_os_error()); }
        if fcntl(fd, F_SETFL, flags | O_NONBLOCK) < 0 { return Err(io::Error::last_os_error()); }
        Ok(())
    }
}
