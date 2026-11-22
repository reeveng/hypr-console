extern crate std;

use super::*;

const MACHINE: Bytes = Bytes(268_435_456);

const SHARE: Bytes = Bytes(1_048_576);

const PAGE: Bytes = Bytes(4096);

fn first(handles: &[HandleValue]) -> Result<HandleValue, HandleError> {
  match handles.first() {
    Some(handle) => Ok(*handle),
    None => Err(HandleError::BadHandle),
  }
}

fn second(handles: &[HandleValue]) -> Result<HandleValue, HandleError> {
  match handles.get(1) {
    Some(handle) => Ok(*handle),
    None => Err(HandleError::BadHandle),
  }
}

fn bootstrap(kernel: &mut Kernel, process: ProcessId) -> Result<HandleValue, HandleError> {
  let held = kernel.handles(process)?;

  first(&held)
}

#[test]
fn a_process_is_born_holding_one_channel_end_and_nothing_else() -> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let child = kernel.create_process(root, root_job)?;

  let held = kernel.handles(child.id)?;

  assert_eq!(held.len(), 1);
  let only = bootstrap(&mut kernel, child.id)?;
  assert_eq!(kernel.read(child.id, only), Err(HandleError::ShouldWait));

  Ok(())
}

#[test]
fn a_handle_written_into_a_channel_leaves_the_writer_and_arrives_with_the_reader()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let child = kernel.create_process(root, root_job)?;
  let memory = kernel.create_memory(root, PAGE)?;
  let granted = kernel.handle_rights(root, memory)?;

  kernel.write(root, child.channel, b"here", &[memory])?;

  assert_eq!(
    kernel.handle_rights(root, memory),
    Err(HandleError::BadHandle)
  );
  let only = bootstrap(&mut kernel, child.id)?;
  let message = kernel.read(child.id, only)?;
  assert_eq!(message.bytes, b"here".to_vec());
  let arrived = first(&message.handles)?;
  assert_eq!(kernel.handle_rights(child.id, arrived), Ok(granted));

  Ok(())
}

#[test]
fn a_write_that_cannot_move_every_handle_moves_none_and_delivers_nothing()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let child = kernel.create_process(root, root_job)?;
  let movable = kernel.create_memory(root, PAGE)?;
  let Ok(kept_back) = Rights::from_rights(&[Right::Read]);
  let fixed = kernel.duplicate(root, movable, kept_back)?;

  assert_eq!(
    kernel.write(root, child.channel, b"", &[movable, fixed]),
    Err(HandleError::AccessDenied(Right::Transfer))
  );
  kernel.handle_rights(root, movable)?;
  let only = bootstrap(&mut kernel, child.id)?;
  assert_eq!(kernel.read(child.id, only), Err(HandleError::ShouldWait));

  Ok(())
}

#[test]
fn a_duplicate_can_carry_fewer_rights_and_never_more() -> Result<(), HandleError> {
  let Boot {
    mut kernel, root, ..
  } = Kernel::boot(MACHINE)?;
  let memory = kernel.create_memory(root, PAGE)?;
  let Ok(read) = Rights::from_rights(&[Right::Read, Right::Duplicate]);
  let narrowed = kernel.duplicate(root, memory, read)?;
  let Ok(wider) = read.union(Right::Write);

  assert_eq!(
    kernel.duplicate(root, narrowed, wider),
    Err(HandleError::InvalidRights)
  );

  Ok(())
}

#[test]
fn a_reader_out_of_handle_numbers_leaves_the_message_and_its_charge_pending()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let child = kernel.create_process(root, root_job)?;
  let only = bootstrap(&mut kernel, child.id)?;
  let memory = kernel.create_memory(root, PAGE)?;
  let before = kernel.budget(root, root_job)?;

  kernel.write(root, child.channel, b"frame", &[memory])?;

  let queued = kernel.budget(root, root_job)?;
  let reader = kernel.process(child.id)?;
  let available = core::mem::replace(&mut reader.next_handle, u64::MAX);

  assert_eq!(kernel.read(child.id, only), Err(HandleError::NoResources));
  assert_eq!(kernel.budget(root, root_job), Ok(queued));

  let restored = kernel.process(child.id)?;
  restored.next_handle = available;
  let delivered = kernel.read(child.id, only)?;

  assert_eq!(delivered.bytes, b"frame");
  assert_eq!(delivered.handles.len(), 1);
  assert_eq!(kernel.budget(root, root_job), Ok(before));

  Ok(())
}

#[test]
fn a_handle_value_means_nothing_in_another_process() -> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let child = kernel.create_process(root, root_job)?;
  let memory = kernel.create_memory(root, PAGE)?;

  assert_eq!(
    kernel.handle_rights(child.id, memory),
    Err(HandleError::BadHandle)
  );

  Ok(())
}

#[test]
fn memory_runs_only_for_the_holder_of_the_resource_and_then_cannot_be_written()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    executable_resource,
  } = Kernel::boot(MACHINE)?;
  let child = kernel.create_process(root, root_job)?;
  let theirs = kernel.create_memory(child.id, PAGE)?;
  let only = bootstrap(&mut kernel, child.id)?;

  assert_eq!(
    kernel.replace_as_executable(child.id, theirs, only),
    Err(HandleError::WrongType(ObjectType::ExecutableResource))
  );

  let ours = kernel.create_memory(root, PAGE)?;
  let running = kernel.replace_as_executable(root, ours, executable_resource)?;
  let granted = kernel.handle_rights(root, running)?;

  assert_eq!(granted.contains(Right::Execute), Ok(Contains::Yes));
  assert_eq!(granted.contains(Right::Write), Ok(Contains::No));
  assert_eq!(
    kernel.handle_rights(root, ours),
    Err(HandleError::BadHandle)
  );

  Ok(())
}

#[test]
fn making_memory_executable_seals_local_and_in_flight_writable_aliases() -> Result<(), HandleError>
{
  let Boot {
    mut kernel,
    root,
    root_job,
    executable_resource,
  } = Kernel::boot(MACHINE)?;
  let child = kernel.create_process(root, root_job)?;
  let only = bootstrap(&mut kernel, child.id)?;
  let memory = kernel.create_memory(root, PAGE)?;
  let writable = kernel.handle_rights(root, memory)?;
  let local = kernel.duplicate(root, memory, writable)?;
  let moving = kernel.duplicate(root, memory, writable)?;
  let Ok(read) = Rights::from_rights(&[Right::Read]);
  let reader = kernel.duplicate(root, memory, read)?;

  assert_eq!(
    kernel.replace_as_executable(root, reader, executable_resource),
    Err(HandleError::AccessDenied(Right::Write))
  );

  let contents = kernel.write_memory(root, local)?;
  contents.fill(0x5a);
  kernel.write(root, child.channel, &[], &[moving])?;

  let running = kernel.replace_as_executable(root, memory, executable_resource)?;
  let delivered = kernel.read(child.id, only)?;
  let remote = first(&delivered.handles)?;

  assert_eq!(
    kernel.write_memory(root, local),
    Err(HandleError::AccessDenied(Right::Write))
  );
  assert_eq!(
    kernel.write_memory(child.id, remote),
    Err(HandleError::AccessDenied(Right::Write))
  );
  let received = kernel.read_memory(child.id, remote)?;
  assert!(received.iter().all(|byte| *byte == 0x5a));

  kernel.close(root, running)?;

  assert_eq!(
    kernel.write_memory(child.id, remote),
    Err(HandleError::AccessDenied(Right::Write))
  );

  Ok(())
}

#[test]
fn when_the_last_holder_of_one_end_goes_the_other_end_is_told() -> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let child = kernel.create_process(root, root_job)?;

  kernel.kill(root, child.process)?;

  assert_eq!(
    kernel.write(root, child.channel, b"", &[]),
    Err(HandleError::PeerClosed)
  );
  assert_eq!(
    kernel.read(root, child.channel),
    Err(HandleError::PeerClosed)
  );

  Ok(())
}

#[test]
fn a_message_nobody_read_lets_go_of_what_it_carried() -> Result<(), HandleError> {
  let Boot {
    mut kernel, root, ..
  } = Kernel::boot(MACHINE)?;
  let (one, other) = kernel.create_channel(root)?;
  let (near, far) = kernel.create_channel(root)?;

  kernel.write(root, one, b"", &[far])?;
  kernel.close(root, other)?;
  kernel.close(root, one)?;

  assert_eq!(
    kernel.write(root, near, b"", &[]),
    Err(HandleError::PeerClosed)
  );

  Ok(())
}

#[test]
fn killing_a_job_ends_every_process_in_it_and_under_it_and_nothing_else()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let browser_job = kernel.create_job(root, root_job, SHARE)?;
  let renderers = kernel.create_job(root, browser_job, PAGE)?;
  let browser = kernel.create_process(root, browser_job)?;
  let renderer = kernel.create_process(root, renderers)?;
  let neighbour = kernel.create_process(root, root_job)?;

  kernel.kill(root, browser_job)?;

  assert_eq!(
    kernel.handles(browser.id),
    Err(HandleError::ProcessNotFound)
  );
  assert_eq!(
    kernel.handles(renderer.id),
    Err(HandleError::ProcessNotFound)
  );
  assert_eq!(
    kernel.write(root, renderer.channel, b"", &[]),
    Err(HandleError::PeerClosed)
  );
  kernel.handles(neighbour.id)?;
  assert_eq!(
    kernel
      .create_process(root, renderers)
      .map(|created| created.id),
    Err(HandleError::BadState)
  );

  Ok(())
}

#[test]
fn a_job_refuses_what_its_policy_denies_and_so_does_every_job_under_it() -> Result<(), HandleError>
{
  let Boot {
    mut kernel,
    root,
    root_job,
    executable_resource,
  } = Kernel::boot(MACHINE)?;
  let browser_job = kernel.create_job(root, root_job, SHARE)?;

  kernel.set_policy(
    root,
    browser_job,
    &[Condition::ReplaceAsExecutable, Condition::NewProcess],
  )?;

  let under = kernel.create_job(root, browser_job, PAGE)?;
  let renderer = kernel.create_process(root, under)?;
  let memory = kernel.create_memory(root, PAGE)?;
  let Ok(lent) = Rights::from_rights(&[Right::Duplicate, Right::Transfer]);
  let resource = kernel.duplicate(root, executable_resource, lent)?;

  kernel.write(root, renderer.channel, b"", &[memory, resource])?;

  let only = bootstrap(&mut kernel, renderer.id)?;
  let message = kernel.read(renderer.id, only)?;
  let given = message.handles;
  let memory_2 = first(&given)?;
  let resource_2 = second(&given)?;

  assert_eq!(
    kernel.replace_as_executable(renderer.id, memory_2, resource_2),
    Err(HandleError::DeniedByPolicy(Condition::ReplaceAsExecutable))
  );
  assert_eq!(
    kernel
      .create_process(renderer.id, only)
      .map(|created| created.id),
    Err(HandleError::DeniedByPolicy(Condition::NewProcess))
  );

  Ok(())
}

#[test]
fn a_policy_is_set_on_an_empty_job_or_not_at_all() -> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let job = kernel.create_job(root, root_job, SHARE)?;
  let _started = kernel.create_process(root, job)?;

  assert_eq!(
    kernel.set_policy(root, job, &[Condition::NewChannel]),
    Err(HandleError::BadState)
  );

  Ok(())
}

#[test]
fn revoking_takes_back_every_copy_made_from_a_handle_wherever_it_went_and_keeps_the_handle()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let browser = kernel.create_process(root, root_job)?;
  let microphone = kernel.create_memory(root, PAGE)?;
  let Ok(lent) = Rights::from_rights(&[Right::Read, Right::Duplicate, Right::Transfer]);
  let copy = kernel.duplicate(root, microphone, lent)?;

  kernel.write(root, browser.channel, b"", &[copy])?;

  let only = bootstrap(&mut kernel, browser.id)?;
  let message = kernel.read(browser.id, only)?;
  let held = first(&message.handles)?;
  let again = kernel.duplicate(browser.id, held, lent)?;

  kernel.revoke(root, microphone)?;

  assert_eq!(
    kernel.handle_rights(browser.id, held),
    Err(HandleError::BadHandle)
  );
  assert_eq!(
    kernel.handle_rights(browser.id, again),
    Err(HandleError::BadHandle)
  );
  kernel.handle_rights(root, microphone)?;

  Ok(())
}

#[test]
fn revoking_one_branch_keeps_its_sibling_and_unrelated_handles_in_the_same_message()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let recipient = kernel.create_process(root, root_job)?;
  let bootstrap = bootstrap(&mut kernel, recipient.id)?;
  let original = kernel.create_memory(root, PAGE)?;
  let other = kernel.create_memory(root, PAGE)?;
  let original_bytes = kernel.write_memory(root, original)?;
  let first_byte = original_bytes.first_mut().ok_or(HandleError::BadState)?;
  *first_byte = 7;

  let other_bytes = kernel.write_memory(root, other)?;
  let other_byte = other_bytes.first_mut().ok_or(HandleError::BadState)?;
  *other_byte = 9;
  let Ok(lendable) = Rights::from_rights(&[Right::Read, Right::Duplicate, Right::Transfer]);
  let branch = kernel.duplicate(root, original, lendable)?;
  let sibling = kernel.duplicate(root, original, lendable)?;
  let descendant = kernel.duplicate(root, branch, lendable)?;
  let queued = kernel.duplicate(root, descendant, lendable)?;
  let queued_sibling = kernel.duplicate(root, sibling, lendable)?;
  let queued_other = kernel.duplicate(root, other, lendable)?;

  kernel.write(
    root,
    recipient.channel,
    b"mixed",
    &[queued, queued_sibling, queued_other],
  )?;
  kernel.revoke(root, branch)?;

  assert_eq!(
    kernel.handle_rights(root, descendant),
    Err(HandleError::BadHandle)
  );

  for retained in [original, branch, sibling, other] {
    kernel.handle_rights(root, retained)?;
  }

  let message = kernel.read(recipient.id, bootstrap)?;
  assert_eq!(message.bytes, b"mixed");

  let (sibling_2, other_2) = match message.handles.as_slice() {
    [left, right] => (*left, *right),
    _other_count => return Err(HandleError::BadState),
  };

  let sibling_bytes = kernel.read_memory(recipient.id, sibling_2)?;
  assert_eq!(sibling_bytes.first(), Some(&7));

  let unrelated_bytes = kernel.read_memory(recipient.id, other_2)?;
  assert_eq!(unrelated_bytes.first(), Some(&9));
  Ok(())
}

#[test]
fn a_copy_outlives_the_handle_it_was_made_from_and_is_still_taken_back() -> Result<(), HandleError>
{
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let browser = kernel.create_process(root, root_job)?;
  let microphone = kernel.create_memory(root, PAGE)?;
  let Ok(lent) = Rights::from_rights(&[Right::Read, Right::Duplicate, Right::Transfer]);
  let copy = kernel.duplicate(root, microphone, lent)?;

  kernel.write(root, browser.channel, b"", &[copy])?;

  let only = bootstrap(&mut kernel, browser.id)?;
  let message = kernel.read(browser.id, only)?;
  let held = first(&message.handles)?;
  let kept = kernel.duplicate(browser.id, held, lent)?;

  kernel.close(browser.id, held)?;
  kernel.revoke(root, microphone)?;

  assert_eq!(
    kernel.handle_rights(browser.id, kept),
    Err(HandleError::BadHandle)
  );

  Ok(())
}

#[test]
fn a_reader_that_never_reads_stops_its_writer_and_the_writer_keeps_what_it_tried_to_send()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let renderer = kernel.create_process(root, root_job)?;

  for _ in 0..WAITING_MESSAGES_LIMIT {
    kernel.write(root, renderer.channel, b"a frame", &[])?;
  }

  let memory = kernel.create_memory(root, PAGE)?;

  assert_eq!(
    kernel.write(root, renderer.channel, b"one more", &[memory]),
    Err(HandleError::ChannelFull)
  );
  kernel.handle_rights(root, memory)?;

  let only = bootstrap(&mut kernel, renderer.id)?;
  let _read = kernel.read(renderer.id, only)?;

  kernel.write(root, renderer.channel, b"one more", &[memory])?;

  assert_eq!(
    kernel.handle_rights(root, memory),
    Err(HandleError::BadHandle)
  );

  Ok(())
}

#[test]
fn a_message_larger_than_a_channel_carries_is_refused_whole() -> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let renderer = kernel.create_process(root, root_job)?;
  let memory = kernel.create_memory(root, PAGE)?;
  let too_long: Vec<u8> = (0..=MESSAGE_BYTES_LIMIT).map(|_| 0_u8).collect();
  let longest: Vec<u8> = (0..MESSAGE_BYTES_LIMIT).map(|_| 0_u8).collect();

  assert_eq!(
    kernel.write(root, renderer.channel, &too_long, &[memory]),
    Err(HandleError::MessageTooLarge)
  );
  kernel.handle_rights(root, memory)?;
  let only = bootstrap(&mut kernel, renderer.id)?;
  assert_eq!(kernel.read(renderer.id, only), Err(HandleError::ShouldWait));

  kernel.write(root, renderer.channel, &longest, &[memory])?;

  Ok(())
}

#[test]
fn a_channel_delivers_the_maximum_number_of_handles_in_order() -> Result<(), HandleError> {
  let Boot {
    mut kernel, root, ..
  } = Kernel::boot(MACHINE)?;
  let (sending, receiving) = kernel.create_channel(root)?;
  let mut offered = [HandleValue(0); 65];

  for (size, slot) in (1..=MESSAGE_HANDLES_LIMIT).zip(&mut offered) {
    let checked_value = kernel.create_memory(root, Bytes(u64::from(size)))?;
    *slot = checked_value;
  }

  let excess = kernel.create_memory(root, PAGE)?;
  let tail = offered.last_mut().ok_or(HandleError::BadState)?;
  *tail = excess;

  assert_eq!(
    kernel.write(root, sending, b"", &offered),
    Err(HandleError::MessageTooLarge)
  );
  let (_extra, outgoing) = offered.split_last().ok_or(HandleError::BadState)?;

  kernel.write(root, sending, b"", outgoing)?;
  let message = kernel.read(root, receiving)?;
  assert_eq!(message.handles.len(), outgoing.len());

  for (handle, size) in message.handles.into_iter().zip(1..=MESSAGE_HANDLES_LIMIT) {
    let memory = kernel.read_memory(root, handle)?;
    assert_eq!(u32::try_from(memory.len()), Ok(size));
  }

  Ok(())
}

#[test]
fn what_is_made_and_let_go_of_leaves_nothing_in_the_kernel() -> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let before = kernel.object_count()?;
  let paid = kernel.budget(root, root_job)?;

  let memory = kernel.create_memory(root, PAGE)?;
  let (one, other) = kernel.create_channel(root)?;
  let job = kernel.create_job(root, root_job, SHARE)?;
  let under = kernel.create_job(root, job, SHARE)?;
  let started = kernel.create_process(root, under)?;

  kernel.close(root, memory)?;
  kernel.close(root, one)?;
  kernel.close(root, other)?;
  kernel.close(root, job)?;
  kernel.close(root, under)?;

  kernel.handles(started.id)?;

  kernel.kill(root, started.process)?;
  kernel.close(root, started.process)?;
  kernel.close(root, started.channel)?;

  assert_eq!(kernel.object_count(), Ok(before));
  assert_eq!(kernel.budget(root, root_job), Ok(paid));

  Ok(())
}

#[test]
fn a_job_held_only_from_inside_the_job_being_killed_goes_with_it() -> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let before = kernel.object_count()?;
  let browser_job = kernel.create_job(root, root_job, SHARE)?;
  let renderers = kernel.create_job(root, browser_job, PAGE)?;
  let browser = kernel.create_process(root, browser_job)?;

  kernel.write(root, browser.channel, b"", &[renderers])?;

  let only = bootstrap(&mut kernel, browser.id)?;
  let _given = kernel.read(browser.id, only)?;

  kernel.kill(root, browser_job)?;
  kernel.close(root, browser_job)?;
  kernel.close(root, browser.process)?;
  kernel.close(root, browser.channel)?;

  assert_eq!(kernel.object_count(), Ok(before));

  Ok(())
}

#[test]
fn a_job_has_no_more_than_its_share_and_its_neighbour_is_not_asked_to_pay()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let browser_job = kernel.create_job(root, root_job, SHARE)?;
  let browser = kernel.create_process(root, browser_job)?;
  let neighbour = kernel.create_process(root, root_job)?;

  assert_eq!(
    kernel.create_memory(browser.id, SHARE),
    Err(HandleError::OverBudget)
  );
  let within = kernel.create_memory(browser.id, PAGE)?;
  kernel.handle_rights(browser.id, within)?;
  let _beside = kernel.create_memory(neighbour.id, SHARE)?;

  Ok(())
}

#[test]
fn a_memory_span_returns_to_the_boot_reserve_after_its_last_handle_closes()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot_with_memory_backing(MACHINE, Bytes(8), 2)?;
  let left = kernel.create_memory(root, Bytes(4))?;
  let right = kernel.create_memory(root, Bytes(4))?;
  let rights = kernel.handle_rights(root, left)?;
  let copy = kernel.duplicate(root, left, rights)?;
  let left_contents = kernel.write_memory(root, left)?;
  left_contents.fill(0x5a);

  let right_contents = kernel.write_memory(root, right)?;
  right_contents.fill(0x7f);
  kernel.close(root, left)?;
  let charged = kernel.budget(root, root_job)?;

  assert_eq!(
    kernel.create_memory(root, Bytes(1)),
    Err(HandleError::NoResources)
  );
  assert_eq!(kernel.budget(root, root_job), Ok(charged));

  kernel.close(root, copy)?;
  let reused = kernel.create_memory(root, Bytes(4))?;
  let reused_contents = kernel.read_memory(root, reused)?;
  assert_eq!(reused_contents, &[0; 4]);

  let kept_contents = kernel.read_memory(root, right)?;
  assert_eq!(kept_contents, &[0x7f; 4]);
  kernel.close(root, reused)?;
  kernel.close(root, right)?;
  let whole = kernel.create_memory(root, Bytes(8))?;
  let whole_contents = kernel.read_memory(root, whole)?;
  assert_eq!(whole_contents, &[0; 8]);

  Ok(())
}

#[test]
fn a_share_comes_out_of_the_job_above_and_goes_back_when_the_job_does() -> Result<(), HandleError>
{
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let before = kernel.budget(root, root_job)?;

  assert_eq!(
    kernel.create_job(root, root_job, MACHINE),
    Err(HandleError::OverBudget)
  );
  assert_eq!(kernel.budget(root, root_job), Ok(before));

  let job = kernel.create_job(root, root_job, SHARE)?;
  let under = kernel.create_job(root, job, PAGE)?;

  assert_eq!(
    kernel.create_job(root, job, SHARE),
    Err(HandleError::OverBudget)
  );
  assert_eq!(
    kernel.budget(root, job),
    Ok(Budget {
      share: SHARE,
      charged: PAGE
    })
  );

  kernel.close(root, under)?;
  kernel.close(root, job)?;

  assert_eq!(kernel.budget(root, root_job), Ok(before));

  Ok(())
}

#[test]
fn a_message_is_its_writers_until_it_is_read_and_then_nobodys() -> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let writers = kernel.create_job(root, root_job, SHARE)?;
  let readers = kernel.create_job(root, root_job, SHARE)?;
  let writer = kernel.create_process(root, writers)?;
  let reader = kernel.create_process(root, readers)?;
  let (near, far) = kernel.create_channel(root)?;

  kernel.write(root, writer.channel, b"", &[near])?;
  kernel.write(root, reader.channel, b"", &[far])?;

  let writing = bootstrap(&mut kernel, writer.id)?;
  let reading = bootstrap(&mut kernel, reader.id)?;
  let sent = kernel.read(writer.id, writing)?;
  let received = kernel.read(reader.id, reading)?;
  let out = first(&sent.handles)?;
  let into = first(&received.handles)?;
  let memory = kernel.create_memory(writer.id, PAGE)?;
  let quiet = kernel.budget(root, writers)?;
  let waiting = kernel.budget(root, readers)?;

  kernel.write(writer.id, out, b"eight by", &[memory])?;

  assert_eq!(
    kernel.budget(root, writers),
    Ok(Budget {
      share: SHARE,
      charged: Bytes(
        quiet
          .charged
          .0
          .saturating_add(8)
          .saturating_add(MESSAGE_COST.0)
      )
    })
  );
  assert_eq!(kernel.budget(root, readers), Ok(waiting));

  let _arrived = kernel.read(reader.id, into)?;

  assert_eq!(
    kernel.budget(root, writers),
    Ok(Budget {
      share: SHARE,
      charged: Bytes(quiet.charged.0.saturating_sub(HANDLE_COST.0))
    })
  );
  assert_eq!(
    kernel.budget(root, readers),
    Ok(Budget {
      share: SHARE,
      charged: Bytes(waiting.charged.0.saturating_add(HANDLE_COST.0))
    })
  );

  Ok(())
}

#[test]
fn a_reader_with_no_budget_for_what_a_message_carries_leaves_it_waiting_and_its_writer_still_paying()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let cramped = kernel.create_job(root, root_job, HANDLE_COST)?;
  let reader = kernel.create_process(root, cramped)?;
  let memory = kernel.create_memory(root, PAGE)?;
  let before = kernel.budget(root, root_job)?;

  kernel.write(root, reader.channel, b"", &[memory])?;

  let reading = bootstrap(&mut kernel, reader.id)?;

  assert_eq!(
    kernel.read(reader.id, reading),
    Err(HandleError::OverBudget)
  );
  assert_eq!(
    kernel.budget(root, root_job),
    Ok(Budget {
      share: before.share,
      charged: Bytes(before.charged.0.saturating_add(MESSAGE_COST.0))
    })
  );
  assert_eq!(
    kernel.read(reader.id, reading),
    Err(HandleError::OverBudget)
  );

  Ok(())
}

struct Scattered {
  kernel: Kernel,
  root: ProcessId,
  lent: HandleValue,
  borrower: ProcessId,
  kept_by_borrower: HandleValue,
  running: HandleValue,
  made_from_running: HandleValue,
  unread: CreatedProcess,
  on_its_way: CreatedProcess,
}

fn lending() -> Result<Rights, Never> {
  Rights::from_rights(&[Right::Read, Right::Duplicate, Right::Transfer])
}

fn everywhere(kernel: &Kernel) -> Result<BTreeMap<HandleId, Location>, Never> {
  let mut found = BTreeMap::new();

  for (process, holder) in &kernel.processes {
    for (value, handle) in &holder.handles {
      let _fresh = found.insert(
        handle.id,
        Location::Table {
          process: *process,
          value: *value,
        },
      );
    }
  }

  for (endpoint, entry) in &kernel.objects {
    match &entry.object {
      Object::Endpoint(held) => {
        for (message, queued) in &held.waiting {
          for (slot, handle) in &queued.handles {
            let _fresh = found.insert(
              handle.id,
              Location::Message {
                endpoint: *endpoint,
                message: *message,
                slot: *slot,
              },
            );
          }
        }
      }
      Object::Memory(_)
      | Object::PhysicalFrame(_)
      | Object::ExecutableResource
      | Object::TaskObj(_)
      | Object::Process(_) => {}
    }
  }

  Ok(found)
}

fn made_from(
  kernel: &mut Kernel,
  by: ProcessId,
  at: HandleValue,
) -> Result<BTreeSet<HandleId>, HandleError> {
  let holder = kernel.process(by)?;
  let handle = holder.get(at)?;
  let Ok(under) = kernel.descendants(handle.id);

  Ok(under)
}

fn scattered() -> Result<Scattered, HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    executable_resource,
  } = Kernel::boot(MACHINE)?;
  let Ok(shared) = lending();
  let lent = kernel.create_memory(root, PAGE)?;

  let borrower = kernel.create_process(root, root_job)?;
  let handed = kernel.duplicate(root, lent, shared)?;

  kernel.write(root, borrower.channel, b"", &[handed])?;

  let borrowing = bootstrap(&mut kernel, borrower.id)?;
  let arrived = kernel.read(borrower.id, borrowing)?;
  let borrowed = first(&arrived.handles)?;
  let kept_by_borrower = kernel.duplicate(borrower.id, borrowed, shared)?;

  kernel.close(borrower.id, borrowed)?;

  let Ok(sealing) = shared.union(Right::Write);
  let writable = kernel.duplicate(root, lent, sealing)?;
  let running = kernel.replace_as_executable(root, writable, executable_resource)?;
  let made_from_running = kernel.duplicate(root, running, shared)?;

  let unread = kernel.create_process(root, root_job)?;
  let waiting = kernel.duplicate(root, lent, shared)?;
  let beside = kernel.create_memory(root, PAGE)?;

  kernel.write(root, unread.channel, b"", &[waiting, beside])?;

  let (near, far) = kernel.create_channel(root)?;
  let inside = kernel.duplicate(root, lent, shared)?;

  kernel.write(root, near, b"inside", &[inside])?;

  let on_its_way = kernel.create_process(root, root_job)?;

  kernel.write(root, on_its_way.channel, b"", &[far])?;

  let leaving = kernel.create_process(root, root_job)?;
  let taken_along = kernel.duplicate(root, lent, shared)?;

  kernel.write(root, leaving.channel, b"", &[taken_along])?;

  let last = bootstrap(&mut kernel, leaving.id)?;
  let _held_until_the_end = kernel.read(leaving.id, last)?;

  kernel.exit(leaving.id)?;

  let (one, other) = kernel.create_channel(root)?;
  let dropped = kernel.duplicate(root, lent, shared)?;

  kernel.write(root, one, b"", &[dropped])?;
  kernel.close(root, one)?;
  kernel.close(root, other)?;

  Ok(Scattered {
    kernel,
    root,
    lent,
    borrower: borrower.id,
    kept_by_borrower,
    running,
    made_from_running,
    unread,
    on_its_way,
  })
}

#[test]
fn where_the_index_says_a_handle_is_is_where_it_is_after_every_kind_of_move()
-> Result<(), HandleError> {
  let Scattered {
    mut kernel,
    root,
    lent,
    ..
  } = scattered()?;
  let Ok(moved) = everywhere(&kernel);

  assert_eq!(kernel.located, moved);

  let _steps = kernel.revoked(root, lent)?;

  let Ok(left) = everywhere(&kernel);

  assert_eq!(kernel.located, left);

  Ok(())
}

#[test]
fn taking_back_after_many_moves_leaves_no_copy_in_a_table_in_a_message_or_in_a_channel_end_still_on_its_way()
-> Result<(), HandleError> {
  let Scattered {
    mut kernel,
    root,
    lent,
    borrower,
    kept_by_borrower,
    running,
    made_from_running,
    unread,
    on_its_way,
  } = scattered()?;
  let copies = made_from(&mut kernel, root, lent)?;

  assert_eq!(copies.len(), 5);

  let _steps = kernel.revoked(root, lent)?;

  let Ok(left) = everywhere(&kernel);
  let survivors: Vec<&HandleId> = left.keys().filter(|id| copies.contains(id)).collect();

  assert!(survivors.is_empty());
  assert_eq!(made_from(&mut kernel, root, lent), Ok(BTreeSet::new()));
  kernel.handle_rights(root, lent)?;
  assert_eq!(
    kernel.handle_rights(borrower, kept_by_borrower),
    Err(HandleError::BadHandle)
  );
  assert_eq!(
    kernel.handle_rights(root, running),
    Err(HandleError::BadHandle)
  );
  assert_eq!(
    kernel.handle_rights(root, made_from_running),
    Err(HandleError::BadHandle)
  );

  let reading = bootstrap(&mut kernel, unread.id)?;
  let beside = kernel.read(unread.id, reading)?;

  assert_eq!(beside.handles.len(), 1);

  let receiving = bootstrap(&mut kernel, on_its_way.id)?;
  let delivered = kernel.read(on_its_way.id, receiving)?;
  let far = first(&delivered.handles)?;
  let emptied = kernel.read(on_its_way.id, far)?;

  assert_eq!(emptied.bytes, b"inside".to_vec());
  assert!(emptied.handles.is_empty());

  Ok(())
}

struct Crowded {
  kernel: Kernel,
  root: ProcessId,
  lent: HandleValue,
}

fn crowded(neighbours: u32) -> Result<Crowded, HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let Ok(shared) = lending();
  let lent = kernel.create_memory(root, PAGE)?;
  let _kept = kernel.duplicate(root, lent, shared)?;
  let borrower = kernel.create_process(root, root_job)?;
  let handed = kernel.duplicate(root, lent, shared)?;

  kernel.write(root, borrower.channel, b"", &[handed])?;

  let borrowing = bootstrap(&mut kernel, borrower.id)?;
  let _held = kernel.read(borrower.id, borrowing)?;
  let waiting = kernel.duplicate(root, lent, shared)?;

  kernel.write(root, borrower.channel, b"", &[waiting])?;

  for _ in 0..neighbours {
    let neighbour = kernel.create_process(root, root_job)?;
    let theirs = kernel.create_memory(root, PAGE)?;
    let copy = kernel.duplicate(root, theirs, shared)?;
    let _own = kernel.create_memory(neighbour.id, PAGE)?;

    kernel.write(root, neighbour.channel, b"noise", &[copy])?;
  }

  Ok(Crowded { kernel, root, lent })
}

#[test]
fn taking_back_costs_what_is_taken_back_and_nothing_else_however_much_the_kernel_holds()
-> Result<(), HandleError> {
  let mut quiet = crowded(0)?;
  let mut busy = crowded(512)?;
  let in_two_tables_and_one_message =
    Steps(TABLE_STEPS.saturating_mul(2).saturating_add(MESSAGE_STEPS));

  assert_eq!(quiet.kernel.processes.len(), 2);
  assert_eq!(busy.kernel.processes.len(), 514);
  assert_eq!(
    busy
      .kernel
      .located
      .len()
      .checked_sub(quiet.kernel.located.len()),
    Some(3072)
  );

  assert_eq!(
    quiet.kernel.revoked(quiet.root, quiet.lent),
    Ok(in_two_tables_and_one_message)
  );
  assert_eq!(
    busy.kernel.revoked(busy.root, busy.lent),
    Ok(in_two_tables_and_one_message)
  );

  Ok(())
}
