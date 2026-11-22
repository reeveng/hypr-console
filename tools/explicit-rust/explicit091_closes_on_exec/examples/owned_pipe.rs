// run-rustfix
// compile-flags: --test
use rustix::io::{Errno, FdFlags, fcntl_getfd};

fn local_test_channel() -> Result<(), Errno> {
    let (read, write) = rustix::pipe::pipe()?;
    assert!(fcntl_getfd(&read)?.contains(FdFlags::CLOEXEC));
    assert!(fcntl_getfd(&write)?.contains(FdFlags::CLOEXEC));
    Ok(())
}

fn main() { local_test_channel().unwrap(); }

#[cfg(test)]
mod tests {
    #[test]
    fn receives_a_local_pipe() -> Result<(), rustix::io::Errno> {
        let (read, _write) = rustix::pipe::pipe()?;
        assert!(rustix::io::fcntl_getfd(&read)?.contains(rustix::io::FdFlags::CLOEXEC));
        Ok(())
    }
}
