//! A realtime wakeup follows wall-clock jumps and catches up after suspend.

use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use rustix::io::Errno;
use rustix::time::{
  ClockId, Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags, Timespec, clock_gettime,
  timerfd_create, timerfd_settime,
};

const SECONDS_PER_MINUTE: i64 = 60;

pub struct Clock(OwnedFd);

impl Clock {
  pub fn new() -> io::Result<Self> {
    let fd = timerfd_create(
      TimerfdClockId::Realtime,
      TimerfdFlags::CLOEXEC | TimerfdFlags::NONBLOCK,
    )?;

    Ok(Self(fd))
  }

  pub fn arm(&self) -> io::Result<i64> {
    let now = clock_gettime(ClockId::Realtime).tv_sec;
    let next = now
      .saturating_sub(now.rem_euclid(SECONDS_PER_MINUTE))
      .saturating_add(SECONDS_PER_MINUTE);
    let _previous = timerfd_settime(
      &self.0,
      TimerfdTimerFlags::ABSTIME | TimerfdTimerFlags::CANCEL_ON_SET,
      &Itimerspec {
        it_interval: Timespec::default(),
        it_value: Timespec {
          tv_sec: next,
          tv_nsec: 0,
        },
      },
    )?;

    Ok(now)
  }

  pub fn tick(&self) -> io::Result<Option<i64>> {
    let mut count = [0; 8];

    match rustix::io::read(&self.0, &mut count) {
      Ok(_) | Err(Errno::CANCELED) => self.arm().map(Some),
      Err(Errno::AGAIN | Errno::INTR) => Ok(None),
      Err(fault) => Err(io::Error::from(fault)),
    }
  }
}

impl AsFd for Clock {
  fn as_fd(&self) -> BorrowedFd<'_> {
    self.0.as_fd()
  }
}
