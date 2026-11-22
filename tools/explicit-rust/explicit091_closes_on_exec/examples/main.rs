use rustix::fd::OwnedFd;
use rustix::fs::{Mode, OFlags};
use rustix::io::Errno;
use rustix::pipe::PipeFlags;

fn inherited() -> Result<(OwnedFd, OwnedFd), Errno> {
    //~v WARN EXPLICIT091_CLOSES_ON_EXEC
    rustix::pipe::pipe()
}

fn flagged_with_something_else() -> Result<(OwnedFd, OwnedFd), Errno> {
    //~v WARN EXPLICIT091_CLOSES_ON_EXEC
    rustix::pipe::pipe_with(PipeFlags::NONBLOCK)
}

fn opened_for_everybody() -> Result<OwnedFd, Errno> {
    //~v WARN EXPLICIT091_CLOSES_ON_EXEC
    rustix::fs::open("somewhere", OFlags::RDONLY, Mode::empty())
}

fn kept() -> Result<(OwnedFd, OwnedFd), Errno> {
    rustix::pipe::pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK)
}

fn opened() -> Result<OwnedFd, Errno> {
    rustix::fs::open("somewhere", OFlags::RDONLY | OFlags::CLOEXEC, Mode::empty())
}

#[cfg(target_os = "linux")]
fn guarded_descriptors() -> Result<(), Errno> {
    use rustix::fs::{MemfdFlags, memfd_create};
    use rustix::process::{PidfdFlags, getpid, pidfd_open};
    let required = MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING;
    let flags = required;
    let memory = memfd_create("console-draw-surface", flags)?;
    let process = pidfd_open(getpid(), PidfdFlags::empty())?;
    for descriptor in [memory, process] {
        assert!(rustix::io::fcntl_getfd(descriptor)?.contains(rustix::io::FdFlags::CLOEXEC));
    }
    Ok(())
}

fn misleading() -> Result<OwnedFd, Errno> {
    let mut flags = OFlags::CLOEXEC;
    flags.remove(OFlags::CLOEXEC);
    //~v WARN EXPLICIT091_CLOSES_ON_EXEC
    rustix::fs::open("name-with-CLOEXEC", flags, Mode::empty())
}

fn cleared() -> Result<(OwnedFd, OwnedFd), Errno> {
    let flags = PipeFlags::NONBLOCK & PipeFlags::CLOEXEC;
    //~v WARN EXPLICIT091_CLOSES_ON_EXEC
    rustix::pipe::pipe_with(flags)
}

fn main() {
    let _ = inherited();
    let _ = flagged_with_something_else();
    let _ = opened_for_everybody();
    let _ = kept();
    let _ = opened();
    let _ = (misleading, cleared);
    #[cfg(target_os = "linux")]
    guarded_descriptors().unwrap();
}
