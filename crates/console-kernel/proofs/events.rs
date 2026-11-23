//! Run with `cargo kani -p console-kernel --lib --harness events`.

use super::*;

fn queue_with<T: Copy>(before: [Option<T>; 2], seed: T) -> Queue<T, 2> {
  let mut queue = Queue::EMPTY;
  if kani::any() {
    assert!(queue.push(seed).is_ok());
    let Ok(_) = queue.pop();
  }
  for value in before.into_iter().flatten() {
    assert!(queue.push(value).is_ok());
  }

  queue
}

#[kani::proof]
#[kani::unwind(8)]
fn committing_effects_is_atomic_and_spends_only_the_accepted_credits() {
  let occupied: u8 = kani::any();
  let length: u8 = kani::any();
  kani::assume(occupied <= 2 && length <= 3);
  let original_state: u64 = kani::any();
  let next: u64 = kani::any();
  let initial_credit: u32 = kani::any();
  let mut credits = EffectCredits(initial_credit);
  let [first, second]: [u8; 2] = kani::any();
  let before = [
    (occupied > 0).then_some(first),
    (occupied > 1).then_some(second),
  ];
  let mut machine = Transitions {
    state: original_state,
    queue: queue_with(before, first),
  };
  let offered: [u8; 3] = kani::any();
  let [new_first, new_second, _third] = offered;
  let effects = &offered[..usize::from(length)];
  let result = machine.commit(Proposal { next, effects }, &mut credits);

  let expected = if u32::from(length) > initial_credit || occupied + length > 2 {
    let reason = if u32::from(length) > initial_credit {
      Backpressure::Credits
    } else {
      Backpressure::Queue
    };
    assert_eq!(result, Err(reason));
    assert_eq!(machine.state, original_state);
    assert_eq!(credits.0, initial_credit);
    before
  } else {
    assert_eq!(result, Ok(()));
    assert_eq!(machine.state, next);
    assert_eq!(credits.0, initial_credit - u32::from(length));
    match (occupied, length) {
      (_, 0) => before,
      (0, 1) => [Some(new_first), None],
      (0, 2) => [Some(new_first), Some(new_second)],
      (1, 1) => [Some(first), Some(new_first)],
      _ => unreachable!(),
    }
  };
  let Ok(queued_first) = machine.next_effect();
  let Ok(queued_second) = machine.next_effect();
  let Ok(empty) = machine.next_effect();
  assert_eq!([queued_first, queued_second], expected);
  assert_eq!(empty, None);
}

#[kani::proof]
#[kani::unwind(6)]
fn ring_admission_and_removal_change_only_the_selected_slot() {
  let position = usize::from(kani::any::<u8>() % 3);
  let length = usize::from(kani::any::<u8>() % 4);
  let values: [u8; 3] = kani::any();
  let value = kani::any();
  let mut queue: Queue<u8, 3> = Queue::EMPTY;
  for _ in 0..position {
    assert!(queue.push(value).is_ok());
    let Ok(popped) = queue.pop();
    assert_eq!(popped, Some(value));
  }
  for &queued in &values[..length] {
    assert!(queue.push(queued).is_ok());
  }
  let mut slots = queue.slots;
  if kani::any() {
    let Ok(popped) = queue.pop();
    if length == 0 {
      assert_eq!(popped, None);
    } else {
      assert_eq!(popped, Some(values[0]));
      slots[position] = None;
    }
  } else if length == 3 {
    assert_eq!(queue.push(value), Err(value));
  } else {
    assert_eq!(queue.push(value), Ok(()));
    slots[(position + length) % 3] = Some(value);
  }
  assert_eq!(queue.slots, slots);
}

#[kani::proof]
fn an_empty_capacity_rejects_an_enqueue_and_returns_no_items() {
  let mut queue: Queue<u8, 0> = Queue::EMPTY;
  let offered = kani::any();
  assert_eq!(queue.push(offered), Err(offered));
  assert!(matches!(queue.tail_mut(), Ok(None)));
  let Ok(removed) = queue.pop();
  assert_eq!(removed, None);
}

fn request() -> Begin {
  Begin {
    operation: OperationId(kani::any()),
    target: TargetId(kani::any()),
    replies_from: Source::Device(DeviceId(kani::any())),
    reserved: Bytes(kani::any()),
  }
}

#[kani::proof]
fn a_completion_settles_only_its_matching_slot_and_refunds_exactly_once() {
  let mut slot = OperationSlot::EMPTY;
  let begin = request();
  let source = begin.replies_from;
  let reserved = begin.reserved;
  let matching = slot.begin(begin).unwrap();
  let claimed = Correlation {
    operation: OperationId(kani::any()),
    target: TargetId(kani::any()),
    generation: Generation(kani::any()),
  };
  let sending = if kani::any() {
    Source::Device(DeviceId(kani::any()))
  } else {
    Source::Kernel
  };
  let value: u32 = kani::any();
  let Ok(packet) = Envelope::from_context(
    EventId(kani::any()),
    sending,
    Completion {
      correlation: claimed,
      value,
    },
  );
  let Ok(result) = slot.complete(packet);

  if matching == claimed && sending == source {
    assert_eq!(
      result,
      Settlement::Complete {
        value,
        refund: reserved
      }
    );
    let Ok(duplicate) = Envelope::from_context(
      EventId(kani::any()),
      source,
      Completion {
        correlation: matching,
        value,
      },
    );
    let Ok(after) = slot.complete(duplicate);
    assert_eq!(after, Settlement::Ignored);
    let Ok(cancellation) = slot.cancel(matching);
    assert_eq!(cancellation, Settlement::Ignored);
  } else {
    assert_eq!(result, Settlement::Ignored);
    let Ok(cancellation) = slot.cancel(matching);
    assert_eq!(
      cancellation,
      Settlement::Complete {
        value: (),
        refund: reserved
      }
    );
  }
  kani::cover!(matching == claimed && sending == source);
}

#[kani::proof]
fn cancellation_and_reuse_reject_old_completions_and_generation_never_wraps() {
  let request = request();
  let Begin {
    operation,
    target,
    replies_from,
    reserved,
  } = request;
  let generation: u64 = kani::any();
  let mut slot = OperationSlot {
    generation: Generation(generation),
    pending: None,
  };
  let recreate = || Begin {
    operation,
    target,
    replies_from,
    reserved,
  };

  let old = match slot.begin(recreate()) {
    Ok(key) => key,
    Err(error) => {
      assert_eq!(error, BeginError::Exhausted);
      assert_eq!(generation, u64::MAX);
      assert!(slot.pending.is_none());
      return;
    }
  };
  assert_eq!(slot.begin(recreate()), Err(BeginError::Busy));
  let Ok(cancelled) = slot.cancel(old);
  assert_eq!(
    cancelled,
    Settlement::Complete {
      value: (),
      refund: reserved
    }
  );

  match slot.begin(recreate()) {
    Ok(new) => {
      assert!(new.generation.0 > old.generation.0);
      let Ok(late) = Envelope::from_context(
        EventId(kani::any()),
        replies_from,
        Completion {
          correlation: old,
          value: (),
        },
      );
      let Ok(after) = slot.complete(late);
      assert_eq!(after, Settlement::Ignored);
      let Ok(correct) = Envelope::from_context(
        EventId(kani::any()),
        replies_from,
        Completion {
          correlation: new,
          value: (),
        },
      );
      let Ok(settled) = slot.complete(correct);
      assert_eq!(
        settled,
        Settlement::Complete {
          value: (),
          refund: reserved
        }
      );
    }
    Err(error) => {
      assert_eq!(error, BeginError::Exhausted);
      assert_eq!(old.generation.0, u64::MAX);
      assert!(slot.pending.is_none());
    }
  }
}

fn input() -> InputEvent {
  let id = EventId(kani::any());
  let input = if kani::any() {
    Input::Position {
      device: DeviceId(kani::any()),
      sequence: Sequence(kani::any()),
      x: kani::any(),
      y: kani::any(),
    }
  } else {
    Input::Key {
      code: kani::any(),
      edge: if kani::any() {
        KeyEdge::Press
      } else {
        KeyEdge::Release
      },
    }
  };
  InputEvent { id, input }
}

#[kani::proof]
#[kani::unwind(7)]
fn overload_preserves_edges_and_only_replaces_a_newer_snapshot_for_the_same_device() {
  let occupied: u8 = kani::any();
  kani::assume(occupied <= 2);
  let head = input();
  let tail = input();
  let incoming = input();
  let before = [
    (occupied > 0).then_some(head),
    (occupied > 1).then_some(tail),
  ];
  let mut queue = InputQueue(queue_with(before, head));
  let (previous, same_device) = match (occupied, head.input, tail.input, incoming.input) {
    (
      1,
      Input::Position {
        device, sequence, ..
      },
      _,
      Input::Position {
        device: new,
        sequence: next,
        ..
      },
    ) if device == new => (Some((head, sequence, next)), true),
    (
      2,
      _,
      Input::Position {
        device, sequence, ..
      },
      Input::Position {
        device: new,
        sequence: next,
        ..
      },
    ) if device == new => (Some((tail, sequence, next)), true),
    _ => (None, false),
  };
  let replaced = previous.is_some_and(|(_, old, new)| new.0 > old.0);
  let outcome = queue.offer(incoming);
  let expected = if let Some((old, _, _)) = previous {
    if replaced {
      Ok(Admission::Replaced(old.id))
    } else {
      Err(InputFault::Stale(incoming))
    }
  } else if occupied == 2 {
    Err(InputFault::Backpressure(incoming))
  } else {
    Ok(Admission::Enqueued)
  };
  assert_eq!(outcome, expected);

  let Ok(first) = queue.take();
  let Ok(second) = queue.take();
  let Ok(empty) = queue.take();
  match occupied {
    0 => assert_eq!((first, second), (Some(incoming), None)),
    1 if replaced => assert_eq!((first, second), (Some(incoming), None)),
    1 if same_device => assert_eq!((first, second), (Some(head), None)),
    1 => assert_eq!((first, second), (Some(head), Some(incoming))),
    2 => assert_eq!(
      (first, second),
      (Some(head), Some(if replaced { incoming } else { tail }))
    ),
    _ => unreachable!(),
  }
  assert_eq!(empty, None);
  kani::cover!(replaced && occupied == 2);
  kani::cover!(matches!(incoming.input, Input::Key { .. }) && occupied == 2);
}
