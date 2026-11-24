//! Selects compute backends by workload and recent successful end-to-end timings.
//! Each logical calculation has one ID across attempts. A failed Vulkan probe
//! can fall back to CPU, and a parity check publishes the serial answer once.
//! Keep a router for the interpreter's lifetime and publish external effects
//! only after calculation succeeds.

use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::num::NonZeroU64;
use std::time::{Duration, Instant};

use console_core_never::Never;
use console_core_state_machine::SingleEffectMachine;

use super::{Target, Vulkan};

mod machine;

use machine::{AttemptResult, Effect, Intent, Profile, Request, Route, Start};

const RECENT: u64 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Workload {
  pub effect: &'static str,
  pub size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
  SerialCpu,
  Cpu,
  Vulkan,
}

impl Backend {
  const ALL: [Self; 3] = [Self::SerialCpu, Self::Cpu, Self::Vulkan];

  fn target(self, gpu: Option<&Vulkan>) -> io::Result<Target<'_>> {
    match self {
      Self::SerialCpu => Ok(Target::SerialCpu),
      Self::Cpu => Ok(Target::Cpu),
      Self::Vulkan => gpu.map(Target::RequireVulkan).ok_or_else(|| {
        io::Error::new(
          io::ErrorKind::Unsupported,
          "Vulkan worker is not configured",
        )
      }),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComputeId(pub u64);

#[derive(Debug, PartialEq, Eq)]
pub struct Routed<T> {
  pub id: ComputeId,
  pub backend: Backend,
  pub value: T,
}

struct Completion {
  id: ComputeId,
  pending: Option<Backend>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Admission {
  Accepted,
  Stale,
}

impl Completion {
  fn new(id: ComputeId, backend: Backend) -> Result<Self, Never> {
    Ok(Self {
      id,
      pending: Some(backend),
    })
  }

  fn fallback(&mut self, backend: Backend) -> Result<(), Never> {
    match (self.pending, backend) {
      (Some(Backend::Vulkan), Backend::SerialCpu | Backend::Cpu) => {
        self.pending = Some(backend);
      }
      (Some(_) | None, _) => {}
    }

    Ok(())
  }

  fn cancel(&mut self) -> Result<(), Never> {
    self.pending = None;
    Ok(())
  }

  fn accept(&mut self, id: ComputeId, backend: Backend) -> Result<Admission, Never> {
    let current = id == self.id && self.pending == Some(backend);

    Ok(match bool::then_some(current, ()) {
      Some(()) => {
        self.pending = None;
        Admission::Accepted
      }
      None => Admission::Stale,
    })
  }
}

#[derive(Debug, Clone, Copy)]
pub struct RoutingPolicy {
  pub min_parallel_time: Duration,
  pub min_cpu_time: Duration,
  pub probe_every: NonZeroU64,
  pub max_probe_interval: NonZeroU64,
}

impl Default for RoutingPolicy {
  fn default() -> Self {
    const FIRST_PROBE: NonZeroU64 = match NonZeroU64::new(4) {
      Some(interval) => interval,
      None => NonZeroU64::MIN,
    };
    const PROBE_CAP: NonZeroU64 = match NonZeroU64::new(256) {
      Some(interval) => interval,
      None => NonZeroU64::MIN,
    };

    Self {
      min_parallel_time: Duration::from_micros(250),
      min_cpu_time: Duration::from_millis(2),
      probe_every: FIRST_PROBE,
      max_probe_interval: PROBE_CAP,
    }
  }
}

#[derive(Default)]
struct Samples {
  elapsed: VecDeque<Duration>,
  probe: Backoff,
  retry: Backoff,
  mismatch: bool,
}

#[derive(Default)]
struct Backoff {
  interval: u64,
  due: u64,
}

impl Backoff {
  fn advance(&mut self, turn: u64, policy: RoutingPolicy) -> Result<(), Never> {
    let limit = policy.max_probe_interval.get();
    let initial = policy.probe_every.get().min(limit);
    self.interval = self.interval.saturating_mul(2).clamp(initial, limit);
    self.due = turn.saturating_add(self.interval);
    Ok(())
  }
}

#[derive(Default)]
struct BackendValues<T> {
  serial: T,
  cpu: T,
  vulkan: T,
}

impl<T> BackendValues<T> {
  fn get(&self, backend: Backend) -> Result<&T, Never> {
    Ok(match backend {
      Backend::SerialCpu => &self.serial,
      Backend::Cpu => &self.cpu,
      Backend::Vulkan => &self.vulkan,
    })
  }

  fn get_mut(&mut self, backend: Backend) -> Result<&mut T, Never> {
    Ok(match backend {
      Backend::SerialCpu => &mut self.serial,
      Backend::Cpu => &mut self.cpu,
      Backend::Vulkan => &mut self.vulkan,
    })
  }
}

#[derive(Clone, Copy)]
pub(super) enum Device {
  CpuOnly,
  Vulkan,
}

#[derive(Clone, Copy)]
enum Eligibility {
  Ready,
  Deferred,
}

impl From<bool> for Eligibility {
  fn from(ready: bool) -> Self {
    match bool::then_some(ready, ()) {
      Some(()) => Self::Ready,
      None => Self::Deferred,
    }
  }
}

#[derive(Clone, Copy)]
struct Available {
  cpu: Eligibility,
  vulkan: Eligibility,
}

impl Available {
  fn eligibility(self, backend: Backend) -> Result<Eligibility, Never> {
    Ok(match backend {
      Backend::SerialCpu => Eligibility::Ready,
      Backend::Cpu => self.cpu,
      Backend::Vulkan => self.vulkan,
    })
  }
}

#[derive(Default)]
pub struct History {
  samples: BackendValues<Samples>,
  turns: u64,
}

impl History {
  #[must_use]
  pub fn recent(
    &self,
    backend: Backend,
  ) -> Result<impl ExactSizeIterator<Item = Duration> + '_, Never> {
    let Ok(samples) = self.samples.get(backend);
    Ok(samples.elapsed.iter().copied())
  }

  #[must_use]
  pub fn average(&self, backend: Backend) -> Result<Option<Duration>, Never> {
    use console_core_number_conversion::{fixed_index, wrapped};

    const NANOS_PER_SECOND: u128 = 1_000_000_000;

    let Ok(recent) = self.recent(backend);
    let Ok(count) = fixed_index(recent.len());
    let total = recent.map(|elapsed| elapsed.as_nanos()).sum::<u128>();
    let mean = match total.checked_div(u128::from(count)) {
      Some(observed) => observed,
      None => return Ok(None),
    };
    let Ok(seconds) = wrapped(mean.wrapping_div(NANOS_PER_SECOND));
    let Ok(nanoseconds) = wrapped(mean.wrapping_rem(NANOS_PER_SECOND));

    Ok(Some(Duration::new(seconds, nanoseconds)))
  }

  fn record(&mut self, backend: Backend, elapsed: Duration) -> Result<(), Never> {
    let Ok(samples) = self.samples.get_mut(backend);
    let Ok(recorded) = console_core_number_conversion::fixed_index(samples.elapsed.len());

    match recorded {
      RECENT => {
        let _oldest = samples.elapsed.pop_front();
      }
      _ => {}
    }

    samples.elapsed.push_back(elapsed);
    samples.retry = Backoff::default();
    Ok(())
  }

  fn usable(&self, backend: Backend) -> Result<Eligibility, Never> {
    let Ok(samples) = self.samples.get(backend);
    Ok(Eligibility::from(
      !samples.mismatch && self.turns >= samples.retry.due,
    ))
  }

  fn available(&self, policy: RoutingPolicy, device: Device) -> Result<Available, Never> {
    let Ok(cpu) = self.usable(Backend::Cpu);
    let Ok(serial_average) = self.average(Backend::SerialCpu);
    let multicore = matches!(cpu, Eligibility::Ready)
      && serial_average.is_some_and(|elapsed| elapsed >= policy.min_parallel_time);
    let mut available = Available {
      cpu: Eligibility::from(multicore),
      vulkan: Eligibility::Deferred,
    };

    match device {
      Device::CpuOnly => {}
      Device::Vulkan => {
        let Ok(usable) = self.usable(Backend::Vulkan);
        let Ok(best_cpu) = self.preferred(available);
        let Ok(cpu_average) = self.average(Backend::Cpu);
        let Ok(best_average) = self.average(best_cpu);
        available.vulkan = Eligibility::from(
          matches!(usable, Eligibility::Ready)
            && (!multicore || cpu_average.is_some())
            && best_average.is_some_and(|elapsed| elapsed >= policy.min_cpu_time),
        );
      }
    }

    Ok(available)
  }

  fn preferred(&self, available: Available) -> Result<Backend, Never> {
    let mut winner = Backend::SerialCpu;

    for candidate in [Backend::Cpu, Backend::Vulkan] {
      let Ok(eligibility) = available.eligibility(candidate);

      match eligibility {
        Eligibility::Deferred => continue,
        Eligibility::Ready => {}
      }

      let Ok(winner_average) = self.average(winner);
      let Ok(candidate_average) = self.average(candidate);
      let faster = match (winner_average, candidate_average) {
        (Some(baseline), Some(elapsed)) => {
          elapsed.as_nanos().wrapping_mul(10) < baseline.as_nanos().wrapping_mul(9)
        }
        (None, _) | (_, None) => false,
      };

      match bool::then_some(faster, ()) {
        Some(()) => winner = candidate,
        None => {}
      }
    }

    Ok(winner)
  }

  fn choose(&self, available: Available, preferred: Backend) -> Result<Backend, Never> {
    let alternative = Backend::ALL
      .into_iter()
      .filter(|&backend| {
        let Ok(eligibility) = available.eligibility(backend);
        backend != preferred && matches!(eligibility, Eligibility::Ready)
      })
      .filter_map(|backend| {
        let Ok(samples) = self.samples.get(backend);
        bool::then_some(
          samples.probe.due <= self.turns,
          (backend, samples.probe.due),
        )
      })
      .min_by_key(|&(_, due)| due);

    Ok(match alternative {
      Some((backend, _)) => backend,
      None => preferred,
    })
  }

  fn reset_probes(&mut self, policy: RoutingPolicy) -> Result<(), Never> {
    let BackendValues {
      serial,
      cpu,
      vulkan,
    } = &mut self.samples;

    for samples in [serial, cpu, vulkan] {
      samples.probe = Backoff::default();

      match bool::then_some(samples.elapsed.is_empty(), ()) {
        Some(()) => {}
        None => {
          let Ok(()) = samples.probe.advance(self.turns, policy);
        }
      }
    }

    Ok(())
  }
}

type Comparison<'a, T> = dyn FnMut(&T, &T) -> bool + 'a;

pub struct Router<'a> {
  gpu: Option<&'a Vulkan>,
  policy: RoutingPolicy,
  next: u64,
  history: BTreeMap<Workload, Profile>,
}

impl<'a> Router<'a> {
  #[must_use]
  pub fn new(gpu: Option<&'a Vulkan>, policy: RoutingPolicy) -> Result<Self, Never> {
    Ok(Self {
      gpu,
      policy,
      next: 0,
      history: BTreeMap::new(),
    })
  }

  #[must_use]
  pub fn history(&self, work: Workload) -> Result<Option<&History>, Never> {
    Ok(self.history.get(&work).map(|profile| &profile.history))
  }

  pub fn run<T>(
    &mut self,
    work: Workload,
    execute: impl FnMut(Target<'_>) -> io::Result<T>,
  ) -> io::Result<Routed<T>> {
    self.drive(work, execute, None, Instant::now)
  }

  pub fn verify<T>(
    &mut self,
    work: Workload,
    execute: impl FnMut(Target<'_>) -> io::Result<T>,
    mut equivalent: impl FnMut(&T, &T) -> bool,
  ) -> io::Result<Routed<T>> {
    Backend::Vulkan.target(self.gpu)?;
    self.drive(work, execute, Some(&mut equivalent), Instant::now)
  }

  fn drive<T>(
    &mut self,
    work: Workload,
    mut execute: impl FnMut(Target<'_>) -> io::Result<T>,
    mut compare: Option<&mut Comparison<'_, T>>,
    now: impl Fn() -> Instant,
  ) -> io::Result<Routed<T>> {
    let id = self.start()?;
    let intent = match (compare.as_ref(), self.gpu) {
      (Some(_), _) => Intent::Verify,
      (None, Some(_)) => Intent::Automatic(Device::Vulkan),
      (None, None) => Intent::Automatic(Device::CpuOnly),
    };
    let start = Start {
      id,
      policy: self.policy,
      intent,
    };
    let saved = self.history.entry(work).or_insert_with(|| {
      let Ok(initial) = Route::initialize(&(), None);
      initial.state
    });
    let Ok(mut turn) = Route::handle(std::mem::take(saved), Request::Start(start));
    let mut value = None;
    let mut fault = None;
    let outcome = 'routing: {
      for _ in 0..4 {
        let attempt = match turn.effect {
          Some(Effect::Attempt(attempt)) => attempt,
          Some(Effect::Publish(reply)) => {
            break 'routing value
              .map(|finished| Routed {
                id: reply.id,
                backend: reply.backend,
                value: finished,
              })
              .ok_or_else(|| io::Error::other("compute completed without a value"));
          }
          None => {
            let reason = match fault {
              Some(previous_failure) => previous_failure,
              None => io::Error::other("compute ended without a completion"),
            };
            break 'routing Err(reason);
          }
        };
        let measured = match attempt.backend.target(self.gpu) {
          Ok(target) => timed(&mut execute, target, &now),
          Err(unavailable) => Err(unavailable),
        };
        let result = match measured {
          Ok(measurement) => match compare_measurement(
            attempt,
            work,
            measurement,
            &mut value,
            compare.as_deref_mut(),
          ) {
            Ok(recorded) => recorded,
            Err(mismatch) => {
              fault = Some(mismatch);
              AttemptResult::Disagreed
            }
          },
          Err(error) => {
            let Ok(()) = log_fallback(attempt, intent, work, &error);
            fault = Some(error);
            AttemptResult::Failed
          }
        };
        let Ok(next) = Route::handle(turn.state, Request::Replied(attempt, result));
        turn = next;
      }

      let Ok(()) = turn.state.abort();
      Err(io::Error::other(
        "compute route exceeded its attempt budget",
      ))
    };
    *saved = turn.state;
    outcome
  }

  fn start(&mut self) -> io::Result<ComputeId> {
    let checked_value = self
      .next
      .checked_add(1)
      .ok_or_else(|| io::Error::other("compute IDs exhausted"))?;
    self.next = checked_value;
    Ok(ComputeId(self.next))
  }
}

fn log_fallback(
  attempt: machine::Attempt,
  intent: Intent,
  work: Workload,
  fault: &io::Error,
) -> Result<(), Never> {
  match (attempt.backend, intent) {
    (Backend::Vulkan, Intent::Automatic(_)) => console_core_logging::debug!(
      "compute {:?} is using CPU for {}: {fault}",
      attempt.id,
      work.effect
    ),
    (Backend::SerialCpu | Backend::Cpu, _) | (Backend::Vulkan, Intent::Verify) => {}
  }

  Ok(())
}

fn compare_measurement<T>(
  attempt: machine::Attempt,
  work: Workload,
  (candidate, elapsed): (T, Duration),
  value: &mut Option<T>,
  comparison: Option<&mut Comparison<'_, T>>,
) -> io::Result<AttemptResult> {
  match (value.as_ref(), comparison) {
    (Some(reference), Some(equivalent)) => {
      match equivalent(reference, &candidate).then_some(()) {
        Some(()) => Ok(AttemptResult::Measured(elapsed)),
        None => Err(io::Error::new(
          io::ErrorKind::InvalidData,
          format!(
            "compute {:?} returned a different {:?} result for {}",
            attempt.id, attempt.backend, work.effect
          ),
        )),
      }
    }
    (None, _) | (_, None) => {
      *value = Some(candidate);
      Ok(AttemptResult::Measured(elapsed))
    }
  }
}

fn timed<T>(
  execute: &mut impl FnMut(Target<'_>) -> io::Result<T>,
  target: Target<'_>,
  now: &impl Fn() -> Instant,
) -> io::Result<(T, Duration)> {
  let started = now();
  let value = execute(target)?;
  Ok((value, now().saturating_duration_since(started)))
}

#[cfg(kani)]
#[path = "../../../proofs/compute_completion.rs"]
mod proofs;

#[cfg(test)]
mod tests {
  use super::*;

  const WORK: Workload = Workload {
    effect: "example",
    size: 12,
  };

  fn next(history: &History, policy: RoutingPolicy) -> Result<Backend, Never> {
    let Ok(available) = history.available(policy, Device::Vulkan);
    let Ok(preferred) = history.preferred(available);
    history.choose(available, preferred)
  }

  fn attempted(effect: Option<Effect>) -> io::Result<machine::Attempt> {
    match effect {
      Some(Effect::Attempt(attempt)) => Ok(attempt),
      Some(Effect::Publish(_)) | None => Err(io::Error::other("routing emitted no attempt")),
    }
  }

  #[test]
  fn starts_serial_then_measures_multicore_before_vulkan() {
    let policy = RoutingPolicy::default();
    let mut history = History::default();
    assert_eq!(next(&history, policy), Ok(Backend::SerialCpu));
    let Ok(()) = history.record(Backend::SerialCpu, Duration::from_micros(50));
    assert_eq!(next(&history, policy), Ok(Backend::SerialCpu));

    for _ in 0..RECENT {
      let Ok(()) = history.record(Backend::SerialCpu, Duration::from_millis(20));
    }

    assert_eq!(next(&history, policy), Ok(Backend::Cpu));
    let Ok(()) = history.record(Backend::Cpu, Duration::from_millis(5));
    let Ok(()) = history.reset_probes(policy);
    assert_eq!(next(&history, policy), Ok(Backend::Vulkan));

    let Ok(()) = history.record(Backend::Vulkan, Duration::from_millis(5));
    let Ok(()) = history.reset_probes(policy);
    assert_eq!(next(&history, policy), Ok(Backend::Cpu));
    let Ok(()) = history.record(Backend::Vulkan, Duration::from_millis(1));
    let Ok(()) = history.reset_probes(policy);
    assert_eq!(next(&history, policy), Ok(Backend::Vulkan));
  }

  #[test]
  fn stable_alternatives_back_off_separately_until_the_cap() -> io::Result<()> {
    let every = NonZeroU64::new(2).ok_or_else(|| io::Error::other("missing initial interval"))?;
    let cap = NonZeroU64::new(8).ok_or_else(|| io::Error::other("missing probe cap"))?;
    let policy = RoutingPolicy {
      probe_every: every,
      max_probe_interval: cap,
      ..RoutingPolicy::default()
    };
    let mut state = Profile::default();
    let elapsed = BackendValues {
      serial: Duration::from_millis(20),
      cpu: Duration::from_millis(25),
      vulkan: Duration::from_millis(30),
    };

    for backend in Backend::ALL {
      let Ok(duration) = elapsed.get(backend);
      let Ok(()) = state.history.record(backend, *duration);
    }

    let Ok(()) = state.history.reset_probes(policy);
    let mut probes: BackendValues<Vec<u64>> = BackendValues::default();

    for turn in 1..24 {
      let Ok(started) = Route::handle(
        state,
        Request::Start(Start {
          id: ComputeId(turn),
          policy,
          intent: Intent::Automatic(Device::Vulkan),
        }),
      );
      let attempt = attempted(started.effect)?;

      match attempt.backend {
        Backend::SerialCpu => {}
        Backend::Cpu | Backend::Vulkan => {
          let Ok(at) = probes.get_mut(attempt.backend);
          at.push(turn);
        }
      }

      let Ok(duration) = elapsed.get(attempt.backend);
      let Ok(finished) = Route::handle(
        started.state,
        Request::Replied(attempt, AttemptResult::Measured(*duration)),
      );
      assert_eq!(finished.effect, Some(Effect::Publish(attempt)));
      state = finished.state;
    }

    assert_eq!(probes.cpu, [2, 6, 14, 22]);
    assert_eq!(probes.vulkan, [3, 7, 15, 23]);
    Ok(())
  }

  #[test]
  fn a_faster_alternative_resets_probe_intervals() -> io::Result<()> {
    let policy = RoutingPolicy::default();
    let mut state = Profile::default();

    for (backend, millis) in [
      (Backend::SerialCpu, 20),
      (Backend::Cpu, 25),
      (Backend::Vulkan, 30),
    ] {
      let Ok(()) = state.history.record(backend, Duration::from_millis(millis));
    }

    state.history.samples.vulkan.probe = Backoff {
      interval: 256,
      due: 256,
    };
    let Ok(started) = Route::handle(
      state,
      Request::Start(Start {
        id: ComputeId(1),
        policy,
        intent: Intent::Automatic(Device::Vulkan),
      }),
    );
    let attempt = attempted(started.effect)?;
    assert_eq!(attempt.backend, Backend::Cpu);
    let Ok(finished) = Route::handle(
      started.state,
      Request::Replied(attempt, AttemptResult::Measured(Duration::from_millis(1))),
    );
    let delay = &finished.state.history.samples.vulkan.probe;
    assert_eq!(delay.interval, policy.probe_every.get());
    assert_eq!(delay.due, 1_u64.saturating_add(policy.probe_every.get()));
    Ok(())
  }

  #[test]
  fn a_parity_check_publishes_one_value_and_blocks_a_mismatched_target() -> io::Result<()> {
    let Ok(gpu) = Vulkan::new(<::std::path::PathBuf as ::core::convert::From<
      &::core::primitive::str,
    >>::from("unused-worker"));
    let Ok(mut router) = Router::new(Some(&gpu), RoutingPolicy::default());
    let checked = router.verify(WORK, |_| Ok(34), PartialEq::eq)?;
    assert_eq!((checked.id, checked.value), (ComputeId(1), 34));
    let mismatch = router
      .verify(
        WORK,
        |target| Ok(!matches!(target, Target::RequireVulkan(_))),
        PartialEq::eq,
      )
      .unwrap_err();
    assert_eq!(mismatch.kind(), io::ErrorKind::InvalidData);
    let next = router.run(WORK, |target| {
      assert!(matches!(target, Target::SerialCpu));
      Ok(34)
    })?;
    assert_eq!((next.id, next.value), (ComputeId(3), 34));
    Ok(())
  }

  #[test]
  fn a_failed_gpu_probe_falls_back_under_one_id_and_waits_before_retrying() -> io::Result<()> {
    let Ok(gpu) = Vulkan::new(<::std::path::PathBuf as ::core::convert::From<
      &::core::primitive::str,
    >>::from("unused-worker"));
    let Ok(mut router) = Router::new(
      Some(&gpu),
      RoutingPolicy {
        min_parallel_time: Duration::MAX,
        min_cpu_time: Duration::ZERO,
        ..RoutingPolicy::default()
      },
    );
    let first = router.run(WORK, |_| Ok(9))?;
    assert_eq!(first.backend, Backend::SerialCpu);
    let attempts = std::cell::Cell::new(0_u32);
    let second = router.run(WORK, |target| {
      attempts.set(attempts.get().saturating_add(1));

      match target {
        Target::RequireVulkan(_) => Err(io::Error::other("test GPU failure")),
        Target::SerialCpu | Target::Cpu | Target::PreferVulkan(_) => Ok(9),
      }
    })?;
    assert_eq!(attempts.get(), 2);
    assert_eq!(
      second,
      Routed {
        id: ComputeId(2),
        backend: Backend::SerialCpu,
        value: 9
      }
    );
    let third = router.run(WORK, |target| {
      assert!(matches!(target, Target::SerialCpu));
      Ok(9)
    })?;
    assert_eq!(third.id, ComputeId(3));
    let Ok(recorded) = router.history(WORK);
    let history = recorded.ok_or_else(|| io::Error::other("missing timed routes"))?;
    let Ok(serial) = history.recent(Backend::SerialCpu);
    let Ok(vulkan) = history.recent(Backend::Vulkan);
    assert_eq!(serial.len(), 3);
    assert_eq!(vulkan.len(), 0);
    Ok(())
  }
}
