//! `cargo kani -p console-compositor --lib --harness click_routing`.
//!
//! Reference model for the Rust compositor port. The input and actual frame
//! presentations use comparable stamps. The history is ordered oldest first
//! and supplies the hit at the click's screen position in each frame on its
//! output. A frame presented at the click's stamp is eligible. The dispatcher
//! checks the captured surface identity against all live surfaces, including
//! hidden workspaces. A surface ID's generation distinguishes its successive
//! lifetimes. The port must validate and deliver in the same serialized turn.
//!
//! The model assumes accurate stamps, a complete retained history, correct hit
//! testing, and surface lifetime tracking. It checks routing once a click is
//! recognized. The two-slot queue makes backpressure reachable. A caller that
//! retries keeps the resolved click and admits older rejected clicks before
//! newer ones to preserve capture order.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Surface {
  id: u32,
  generation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Hit {
  surface: Surface,
  local: (i32, i32),
}

#[derive(Clone, Copy)]
struct Presented {
  at: u64,
  workspace: i64,
  hit: Option<Hit>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Click {
  workspace: i64,
  hit: Hit,
}

#[derive(Debug, PartialEq, Eq)]
enum Capture {
  HistoryUnavailable,
  Miss,
  Resolved(Click),
}

fn capture(at: u64, history: &[Presented]) -> Capture {
  match history.iter().rev().find(|frame| frame.at <= at) {
    Some(frame) => match frame.hit {
      Some(hit) => Capture::Resolved(Click {
        workspace: frame.workspace,
        hit,
      }),
      None => Capture::Miss,
    },
    None => Capture::HistoryUnavailable,
  }
}

#[derive(Debug, PartialEq, Eq)]
enum Dispatch {
  Idle,
  Deliver(Click),
  Cancelled(Click),
}

struct PendingClicks([Option<Click>; 2]);

impl PendingClicks {
  const EMPTY: Self = Self([None, None]);

  fn enqueue(&mut self, click: Click) -> Result<(), Click> {
    match &mut self.0 {
      [slot @ None, _] | [Some(_), slot @ None] => {
        *slot = Some(click);
        Ok(())
      }
      [Some(_), Some(_)] => Err(click),
    }
  }

  fn dispatch(&mut self, live: [Option<Surface>; 2]) -> Dispatch {
    let [head, tail] = self.0;
    self.0 = [tail, None];

    match head {
      Some(click) if live.contains(&Some(click.hit.surface)) => Dispatch::Deliver(click),
      Some(click) => Dispatch::Cancelled(click),
      None => Dispatch::Idle,
    }
  }
}

fn hit() -> Hit {
  Hit {
    surface: Surface {
      id: kani::any(),
      generation: kani::any(),
    },
    local: kani::any(),
  }
}

fn click() -> Click {
  Click {
    workspace: kani::any(),
    hit: hit(),
  }
}

fn optional<T>(value: T) -> Option<T> {
  kani::any::<bool>().then_some(value)
}

fn presented(at: u64, click: Click) -> Presented {
  Presented {
    at,
    workspace: click.workspace,
    hit: Some(click.hit),
  }
}

#[kani::proof]
#[kani::unwind(6)]
fn a_backlogged_click_uses_the_workspace_and_surface_visible_at_capture() {
  let original = click();
  let next = click();
  let [old_frame, press, new_frame]: [u64; 3] = kani::any();
  kani::assume(old_frame <= press && press < new_frame);
  let history = [presented(old_frame, original), presented(new_frame, next)];
  let mut queue = PendingClicks::EMPTY;

  // Input processing starts after the workspace switch has already been drawn.
  let Capture::Resolved(first) = capture(press, &history) else {
    panic!("the old frame is still in history");
  };
  let Capture::Resolved(second) = capture(new_frame, &history) else {
    panic!("the new frame is in history");
  };
  assert_eq!(queue.enqueue(first), Ok(()));
  assert_eq!(queue.enqueue(second), Ok(()));

  let live = [Some(original.hit.surface), Some(next.hit.surface)];
  assert_eq!(queue.dispatch(live), Dispatch::Deliver(original));
  assert_eq!(queue.dispatch(live), Dispatch::Deliver(next));
  assert_eq!(queue.dispatch(live), Dispatch::Idle);
  kani::cover!(original.workspace != next.workspace && original.hit.surface != next.hit.surface);
}

#[kani::proof]
#[kani::unwind(6)]
fn capture_uses_the_latest_eligible_frame_even_when_it_has_no_hit() {
  let [first, second, third, press]: [u64; 4] = kani::any();
  kani::assume(first < second && second < third);
  let original = click();
  let current = click();
  let history = [
    presented(first, original),
    Presented {
      at: second,
      workspace: kani::any(),
      hit: None,
    },
    presented(third, current),
  ];

  let expected = if press < first {
    Capture::HistoryUnavailable
  } else if press < second {
    Capture::Resolved(original)
  } else if press < third {
    Capture::Miss
  } else {
    Capture::Resolved(current)
  };
  assert_eq!(capture(press, &history), expected);
  assert_eq!(capture(press, &[]), Capture::HistoryUnavailable);
  kani::cover!(press < first);
  kani::cover!(press == second);
}

#[kani::proof]
#[kani::unwind(4)]
fn a_full_queue_returns_the_resolved_click_for_retry_after_a_switch() {
  let first = click();
  let second = click();
  let original = click();
  let next = click();
  let mut queue = PendingClicks::EMPTY;
  assert_eq!(queue.enqueue(first), Ok(()));
  assert_eq!(queue.enqueue(second), Ok(()));
  let Capture::Resolved(captured) = capture(0, &[presented(0, original)]) else {
    panic!("the original frame is in history");
  };
  let rejected = queue.enqueue(captured).unwrap_err();
  assert_eq!(rejected, original);

  // By the time there is room again, the old frame may be gone from history.
  assert_eq!(
    capture(0, &[presented(1, next)]),
    Capture::HistoryUnavailable
  );
  assert_eq!(
    queue.dispatch([None, Some(first.hit.surface)]),
    Dispatch::Deliver(first),
  );
  assert_eq!(queue.enqueue(rejected), Ok(()));
  assert_eq!(
    queue.dispatch([None, Some(second.hit.surface)]),
    Dispatch::Deliver(second),
  );
  assert_eq!(
    queue.dispatch([None, Some(original.hit.surface)]),
    Dispatch::Deliver(original),
  );
  assert_eq!(queue.dispatch([None, None]), Dispatch::Idle);
  kani::cover!(next.workspace != original.workspace);
}

#[kani::proof]
#[kani::unwind(4)]
fn a_closed_surface_cancels_its_click_even_when_its_id_is_reused() {
  let original = click();
  let replacement = Surface {
    id: original.hit.surface.id,
    generation: kani::any(),
  };
  kani::assume(replacement.generation != original.hit.surface.generation);

  let mut queue = PendingClicks::EMPTY;
  assert_eq!(queue.enqueue(original), Ok(()));
  let live = [None, optional(replacement)];
  assert_eq!(queue.dispatch(live), Dispatch::Cancelled(original));
  assert_eq!(queue.dispatch(live), Dispatch::Idle);
  kani::cover!(live[1].is_some());
}
