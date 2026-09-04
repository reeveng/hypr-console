//! What time it is, for a program that is frozen and thawed.
//!
//! The daemon guards against acting on what piled up while nobody was
//! listening: a turn that arrives far later than the one before it is a turn
//! the machine was not running for, and what queued in the gap is thrown away
//! rather than acted on. `turning::AWAY_SECONDS` is that guard and the comment
//! on it is the reason it exists.
//!
//! It was blind to the largest gap there is. `std::time::Instant` is
//! `CLOCK_MONOTONIC` on Linux, and `CLOCK_MONOTONIC` stops while the machine
//! is suspended. Measured on the device after a day and a half, it stood
//! eleven hours and forty-six minutes behind the time that had really passed.
//! So a handheld shut in a bag overnight woke, asked how long it had been, was
//! told twenty milliseconds, and acted on every button pressed the evening
//! before -- in order, in one instant, against a desktop that had moved on.
//! What that did on this machine is in the journal: a burst of panels opening
//! and closing, and then Legion left, which leaves the desktop for Game Mode
//! and takes the session down with it.
//!
//! `CLOCK_BOOTTIME` is the same clock with the sleeping counted. It is the
//! only difference between the two, and it is the whole of what was wrong.


use console_number::Float;

/// Seconds since the machine booted, counting the time it spent asleep.
///
/// Never goes backwards, and is not affected by the clock being set, so it can
/// be subtracted from itself to get a duration the way `Instant` can.
pub fn since_boot() -> f64 {
    let mut when = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: a write into a timespec this call owns for the length of it.
    let asked = unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut when) };

    if asked != 0 {
        // Older kernels and stranger platforms. Monotonic is what this had
        // before and is wrong only across a suspend, which is better than a
        // daemon that will not start.
        return monotonic();
    }

    when.tv_sec.float() + when.tv_nsec.float() / 1e9
}

/// The clock that stops while the machine is asleep, for when the other one
/// cannot be had.
fn monotonic() -> f64 {
    let mut when = libc::timespec { tv_sec: 0, tv_nsec: 0 };

    // SAFETY: as above.
    unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut when) };

    when.tv_sec.float() + when.tv_nsec.float() / 1e9
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_answers_with_a_time_the_machine_has_been_up() {
        assert!(since_boot() > 0.0, "a booted machine has been up for some seconds");
    }

    #[test]
    fn it_goes_forwards() {
        let first = since_boot();
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert!(since_boot() > first);
    }

    /// The point of it. A machine that has slept has a boot time ahead of its
    /// monotonic one, and on a machine that has not they are the same. Neither
    /// way round may this be behind.
    #[test]
    fn it_is_never_behind_the_clock_that_stops_for_a_suspend() {
        assert!(since_boot() >= monotonic() - 0.05);
    }

    /// The one that would have caught this. The kernel's own uptime counts the
    /// time the machine spent asleep, so on any machine that has suspended it
    /// stands where this does and well ahead of `Instant`. Put back on the
    /// monotonic clock, this fails on the device by however long it has slept.
    #[test]
    fn it_agrees_with_what_the_kernel_calls_uptime() {
        let Ok(said) = std::fs::read_to_string("/proc/uptime") else {
            return; // not a Linux running this, and the daemon only runs on one
        };
        let uptime: f64 =
            said.split_whitespace().next().expect("a first word").parse().expect("seconds");
        let ours = since_boot();
        assert!(
            (ours - uptime).abs() < 2.0,
            "the kernel says the machine has been up {uptime}s and this says {ours}s"
        );
    }
}
