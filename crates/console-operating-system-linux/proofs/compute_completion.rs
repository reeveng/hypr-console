//! Prove the publication gate and retry bounds used by compute routing.

use super::*;

fn backend(kind: u8) -> Backend {
  match kind {
    0 => Backend::SerialCpu,
    1 => Backend::Cpu,
    _ => Backend::Vulkan,
  }
}

#[kani::proof]
fn only_the_pending_effect_and_backend_can_publish() {
  let id = ComputeId(kani::any());
  let reply_id = if kani::any() {
    id
  } else {
    ComputeId(kani::any())
  };
  let pending = backend(kani::any());
  let reply = backend(kani::any());
  let Ok(mut completion) = Completion::new(id, pending);
  let Ok(accepted) = completion.accept(reply_id, reply);
  let accepted = accepted == Admission::Accepted;
  assert_eq!(accepted, id == reply_id && pending == reply);
  if accepted {
    assert_eq!(completion.accept(reply_id, reply), Ok(Admission::Stale));
    let Ok(()) = completion.fallback(Backend::Cpu);
    assert_eq!(completion.accept(id, Backend::Cpu), Ok(Admission::Stale));
  } else {
    assert_eq!(completion.accept(id, pending), Ok(Admission::Accepted));
  }
}

#[kani::proof]
fn fallback_retires_the_vulkan_reply_and_cancellation_retires_both() {
  let id = ComputeId(kani::any());
  let cpu = if kani::any() {
    Backend::Cpu
  } else {
    Backend::SerialCpu
  };
  let Ok(mut completion) = Completion::new(id, Backend::Vulkan);
  let Ok(()) = completion.fallback(cpu);
  assert_eq!(completion.accept(id, Backend::Vulkan), Ok(Admission::Stale));
  if kani::any() {
    let Ok(()) = completion.cancel();
    let Ok(()) = completion.fallback(cpu);
    assert_eq!(
      completion.accept(id, backend(kani::any())),
      Ok(Admission::Stale)
    );
  } else {
    assert_eq!(completion.accept(id, cpu), Ok(Admission::Accepted));
    assert_eq!(
      completion.accept(id, backend(kani::any())),
      Ok(Admission::Stale)
    );
  }
}

#[kani::proof]
fn probe_and_retry_backoff_stays_within_the_cap_without_wrapping() {
  let first: u64 = kani::any();
  let maximum: u64 = kani::any();
  let turn: u64 = kani::any();
  kani::assume(first != 0 && maximum != 0);
  let policy = RoutingPolicy {
    probe_every: NonZeroU64::new(first).unwrap(),
    max_probe_interval: NonZeroU64::new(maximum).unwrap(),
    ..RoutingPolicy::default()
  };
  let mut backoff = Backoff {
    interval: kani::any(),
    due: kani::any(),
  };
  let Ok(()) = backoff.advance(turn, policy);
  assert!(backoff.interval >= first.min(maximum));
  assert!(backoff.interval <= maximum);
  assert!(backoff.due >= turn);
  assert!(backoff.due - turn <= maximum);
}
