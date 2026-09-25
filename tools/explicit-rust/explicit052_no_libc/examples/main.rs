// UI test for EXPLICIT052 — the kernel is asked through rustix.

// BAD EXPLICIT052 — a signal sent the C library's way.
fn stops(pid: i32) -> i32 {
    //~v EXPLICIT052_NO_LIBC
    unsafe { libc::kill(pid, 15) }
}

// BAD EXPLICIT052 — a type out of libc is libc too.
//~v EXPLICIT052_NO_LIBC
fn width(number: libc::c_int) -> i32 {
    number
}

// GOOD — allowed, and the reason says why.
#[allow(explicit052_no_libc, reason = "a number read out of libc to compare against, and nothing called")]
fn installs() -> i32 {
    libc::SIGTERM
}

fn main() {
    let _ = stops(0);
    let _ = width(0);
    let _ = installs();
}
