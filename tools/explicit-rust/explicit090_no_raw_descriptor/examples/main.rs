use std::fs::File;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};

//~v WARN EXPLICIT090_NO_RAW_DESCRIPTOR
fn kept(held: &File) -> RawFd {
    //~v WARN EXPLICIT090_NO_RAW_DESCRIPTOR
    held.as_raw_fd()
}

fn given_up(held: File) -> i32 {
    //~v WARN EXPLICIT090_NO_RAW_DESCRIPTOR
    held.into_raw_fd()
}

fn declared(number: i32) -> File {
    // SAFETY: the case is about the call, and nothing here is run.
    //~v WARN EXPLICIT090_NO_RAW_DESCRIPTOR
    unsafe { File::from_raw_fd(number) }
}

fn lent(held: &File) -> BorrowedFd<'_> {
    held.as_fd()
}

fn owned(held: File) -> OwnedFd {
    OwnedFd::from(held)
}

fn main() {
    let _ = File::open("somewhere").map(|held| kept(&held));
    let _ = File::open("somewhere").map(given_up);
    let _ = File::open("somewhere").map(|held| lent(&held).as_fd().try_clone_to_owned());
    let _ = File::open("somewhere").map(owned);
    let _ = declared;
}
