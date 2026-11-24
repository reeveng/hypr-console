//! `cargo kani -p console-program-runtime --lib --harness jobs`.

use console_program_contract::{JobId, JobOwner, Task};

use super::*;

const FIRST: JobOwner = JobOwner("first");
const OTHER: JobOwner = JobOwner("other");

fn job() -> Task {
  Task {
    owner: if kani::any() { FIRST } else { OTHER },
    id: JobId(kani::any()),
  }
}

#[kani::proof]
#[kani::unwind(8)]
fn a_live_invocation_requires_the_exact_identity_and_reply_key() {
  let live = Invocation {
    job: job(),
    sequence: kani::any(),
  };
  let incoming = Invocation {
    job: if kani::any() { live.job } else { job() },
    sequence: kani::any(),
  };
  let replied = if kani::any() { None } else { Some(job()) };

  let Ok(accepted) = live.matches_reply(incoming, replied);
  assert_eq!(
    accepted == ReplyStatus::Accepted,
    live.sequence == incoming.sequence
      && live.job.owner == incoming.job.owner
      && live.job.id == incoming.job.id
      && replied == Some(live.job)
  );
  kani::cover!(accepted == ReplyStatus::Accepted);
  kani::cover!(
    accepted == ReplyStatus::Stale && live.job == incoming.job && replied == Some(live.job)
  );
}

#[kani::proof]
#[kani::unwind(8)]
fn reusing_a_job_key_cannot_reuse_its_invocation() {
  let mut sequence: u64 = kani::any();
  kani::assume(sequence <= u64::MAX - 2);
  let key = job();
  let old = Invocation::issue(&mut sequence, key).unwrap();
  let current = Invocation::issue(&mut sequence, key).unwrap();

  assert_eq!(old.job, current.job);
  assert!(old.sequence < current.sequence);
  let Ok(stale) = current.matches_reply(old, Some(key));
  let Ok(correct) = current.matches_reply(current, Some(key));
  assert_eq!(stale, ReplyStatus::Stale);
  assert_eq!(correct, ReplyStatus::Accepted);
}

#[kani::proof]
#[kani::unwind(8)]
fn exhausting_invocation_ids_refuses_to_wrap() {
  let mut sequence = u64::MAX - 1;
  let last = Invocation::issue(&mut sequence, job()).unwrap();
  assert_eq!(last.sequence, u64::MAX);
  assert_eq!(
    Invocation::issue(&mut sequence, job()),
    Err(StartError::Exhausted)
  );
  assert_eq!(sequence, last.sequence);
}
