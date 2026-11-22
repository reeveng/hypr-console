//! Model of a revocation worker. The kernel must make revoked authority
//! unavailable before it lets other tasks run between slices. Each work item
//! covers one record or link. Newly discovered cleanup must also be counted.

use core::num::NonZeroU8;

use console_core_never::Never;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Work {
  VisitEdge,
  DetachHandle,
  RelinkChild,
  ReleaseHandle,
  DrainMessage,
  ReapJob,
}

#[derive(Debug, PartialEq, Eq)]
enum Progress {
  Complete,
  Yield,
}

struct Pending([u64; 6]);

const MAX_SLICE: u8 = 8;

#[derive(Debug)]
struct InvalidBudget;

#[derive(Clone, Copy)]
struct WorkBudget(NonZeroU8);

impl WorkBudget {
  fn within(units: u8) -> Result<Self, InvalidBudget> {
    match NonZeroU8::new(units) {
      Some(count) => match ::core::primitive::bool::then_some(count.get() <= MAX_SLICE, ()) {
        ::core::option::Option::Some(()) => Ok(Self(count)),
        ::core::option::Option::None => Err(InvalidBudget),
      },
      None => Err(InvalidBudget),
    }
  }
}

impl Pending {
  fn next(&mut self) -> Result<Option<Work>, Never> {
    let [edges, handles, children, releases, messages, jobs] = &mut self.0;
    let (remaining, work) = match (*edges, *handles, *children, *releases, *messages, *jobs) {
      (1.., _, _, _, _, _) => (edges, Work::VisitEdge),
      (0, 1.., _, _, _, _) => (handles, Work::DetachHandle),
      (0, 0, 1.., _, _, _) => (children, Work::RelinkChild),
      (0, 0, 0, 1.., _, _) => (releases, Work::ReleaseHandle),
      (0, 0, 0, 0, 1.., _) => (messages, Work::DrainMessage),
      (0, 0, 0, 0, 0, 1..) => (jobs, Work::ReapJob),
      (0, 0, 0, 0, 0, 0) => return Ok(None),
    };
    *remaining = remaining.saturating_sub(1);

    Ok(Some(work))
  }

  fn slice(
    &mut self,
    budget: WorkBudget,
    mut perform: impl FnMut(Work),
  ) -> Result<Progress, Never> {
    for _credit in 0..budget.0.get() {
      let Ok(next) = self.next();

      match next {
        Some(item) => perform(item),
        None => break,
      }
    }

    Ok(match ::core::primitive::bool::then_some(self.0.iter().all(|units| *units == 0), ()) {
      ::core::option::Option::Some(()) => Progress::Complete,
      ::core::option::Option::None => Progress::Yield,
    })
  }
}

#[cfg(kani)]
mod verification {
  use super::*;

  #[kani::proof]
  #[kani::unwind(10)]
  fn each_revocation_work_item_charges_exactly_one_record_of_its_kind() {
    let before: [u64; 6] = kani::any();
    let mut pending = Pending(before);
    let Ok(work) = pending.next();
    let [edge, handle, child, release, message, job] = before;
    let expected = match work {
      Some(Work::VisitEdge) => [edge - 1, handle, child, release, message, job],
      Some(Work::DetachHandle) => [edge, handle - 1, child, release, message, job],
      Some(Work::RelinkChild) => [edge, handle, child - 1, release, message, job],
      Some(Work::ReleaseHandle) => [edge, handle, child, release - 1, message, job],
      Some(Work::DrainMessage) => [edge, handle, child, release, message - 1, job],
      Some(Work::ReapJob) => [edge, handle, child, release, message, job - 1],
      None => {
        assert!(before.into_iter().all(|count| count == 0));
        before
      }
    };

    assert!(
      pending
        .0
        .into_iter()
        .zip(expected)
        .all(|(got, want)| got == want)
    );
    kani::cover!(matches!(work, Some(Work::VisitEdge)));
    kani::cover!(matches!(work, Some(Work::ReapJob)));
  }

  #[kani::proof]
  #[kani::unwind(10)]
  fn revocation_slices_bound_discovery_and_cleanup_work_and_make_progress() {
    let before: [u64; 6] = kani::any();
    let limit: u8 = kani::any();
    let budget = match WorkBudget::within(limit) {
      Ok(budget) => {
        assert!((1..=MAX_SLICE).contains(&limit));
        budget
      }
      Err(InvalidBudget) => {
        assert!(!(1..=MAX_SLICE).contains(&limit));
        return;
      }
    };
    let mut pending = Pending(before);
    let mut consumed = 0_u8;
    let Ok(result) = pending.slice(budget, |_item| consumed += 1);

    assert!(consumed <= limit);
    assert_eq!(consumed == 0, before.into_iter().all(|count| count == 0));
    assert_eq!(
      result == Progress::Complete,
      pending.0.into_iter().all(|count| count == 0)
    );
    if result == Progress::Yield {
      assert_eq!(consumed, limit);
    }
    kani::cover!(result == Progress::Yield);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn one_budget_carries_through_discovery_detachment_and_cleanup() -> Result<(), InvalidBudget> {
    let mut pending = Pending([1, 1, 1, 1, 1, 1]);
    let budget = WorkBudget::within(2)?;
    let Ok(first) = pending.slice(budget, |_item| ());
    assert_eq!(first, Progress::Yield);
    assert_eq!(pending.0, [0, 0, 1, 1, 1, 1]);

    let Ok(second) = pending.slice(budget, |_item| ());
    assert_eq!(second, Progress::Yield);
    assert_eq!(pending.0, [0, 0, 0, 0, 1, 1]);

    let Ok(third) = pending.slice(budget, |_item| ());
    assert_eq!(third, Progress::Complete);
    assert_eq!(pending.0, [0; 6]);

    Ok(())
  }
}
