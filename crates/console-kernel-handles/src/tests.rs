extern crate std;

use super::*;

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
    let Boot { mut kernel, root, root_job, .. } = Kernel::boot()?;
    let child = kernel.create_process(root, root_job)?;

    let held = kernel.handles(child.id)?;

    assert_eq!(held.len(), 1);
    let only = bootstrap(&mut kernel, child.id)?;
    assert_eq!(kernel.read(child.id, only), Err(HandleError::ShouldWait));

    Ok(())
}

#[test]
fn a_handle_written_into_a_channel_leaves_the_writer_and_arrives_with_the_reader() -> Result<(), HandleError> {
    let Boot { mut kernel, root, root_job, .. } = Kernel::boot()?;
    let child = kernel.create_process(root, root_job)?;
    let memory = kernel.create_memory(root)?;
    let granted = kernel.handle_rights(root, memory)?;

    kernel.write(root, child.channel, b"here", &[memory])?;

    assert_eq!(kernel.handle_rights(root, memory), Err(HandleError::BadHandle));
    let only = bootstrap(&mut kernel, child.id)?;
    let message = kernel.read(child.id, only)?;
    assert_eq!(message.bytes, b"here".to_vec());
    let arrived = first(&message.handles)?;
    assert_eq!(kernel.handle_rights(child.id, arrived), Ok(granted));

    Ok(())
}

#[test]
fn a_write_that_cannot_move_every_handle_moves_none_and_delivers_nothing() -> Result<(), HandleError> {
    let Boot { mut kernel, root, root_job, .. } = Kernel::boot()?;
    let child = kernel.create_process(root, root_job)?;
    let movable = kernel.create_memory(root)?;
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
    let Boot { mut kernel, root, .. } = Kernel::boot()?;
    let memory = kernel.create_memory(root)?;
    let Ok(read) = Rights::from_rights(&[Right::Read, Right::Duplicate]);
    let narrowed = kernel.duplicate(root, memory, read)?;
    let Ok(wider) = read.union(Right::Write);

    assert_eq!(kernel.duplicate(root, narrowed, wider), Err(HandleError::InvalidRights));

    Ok(())
}

#[test]
fn a_handle_value_means_nothing_in_another_process() -> Result<(), HandleError> {
    let Boot { mut kernel, root, root_job, .. } = Kernel::boot()?;
    let child = kernel.create_process(root, root_job)?;
    let memory = kernel.create_memory(root)?;

    assert_eq!(kernel.handle_rights(child.id, memory), Err(HandleError::BadHandle));

    Ok(())
}

#[test]
fn memory_runs_only_for_the_holder_of_the_resource_and_then_cannot_be_written() -> Result<(), HandleError> {
    let Boot { mut kernel, root, root_job, executable_resource } = Kernel::boot()?;
    let child = kernel.create_process(root, root_job)?;
    let theirs = kernel.create_memory(child.id)?;
    let only = bootstrap(&mut kernel, child.id)?;

    assert_eq!(
        kernel.replace_as_executable(child.id, theirs, only),
        Err(HandleError::WrongType(ObjectType::ExecutableResource))
    );

    let ours = kernel.create_memory(root)?;
    let running = kernel.replace_as_executable(root, ours, executable_resource)?;
    let granted = kernel.handle_rights(root, running)?;

    assert_eq!(granted.contains(Right::Execute), Ok(Contains::Yes));
    assert_eq!(granted.contains(Right::Write), Ok(Contains::No));
    assert_eq!(kernel.handle_rights(root, ours), Err(HandleError::BadHandle));

    Ok(())
}

#[test]
fn when_the_last_holder_of_one_end_goes_the_other_end_is_told() -> Result<(), HandleError> {
    let Boot { mut kernel, root, root_job, .. } = Kernel::boot()?;
    let child = kernel.create_process(root, root_job)?;

    kernel.kill(root, child.process)?;

    assert_eq!(kernel.write(root, child.channel, b"", &[]), Err(HandleError::PeerClosed));
    assert_eq!(kernel.read(root, child.channel), Err(HandleError::PeerClosed));

    Ok(())
}

#[test]
fn a_message_nobody_read_lets_go_of_what_it_carried() -> Result<(), HandleError> {
    let Boot { mut kernel, root, .. } = Kernel::boot()?;
    let (one, other) = kernel.create_channel(root)?;
    let (near, far) = kernel.create_channel(root)?;

    kernel.write(root, one, b"", &[far])?;
    kernel.close(root, other)?;
    kernel.close(root, one)?;

    assert_eq!(kernel.write(root, near, b"", &[]), Err(HandleError::PeerClosed));

    Ok(())
}

#[test]
fn killing_a_job_ends_every_process_in_it_and_under_it_and_nothing_else() -> Result<(), HandleError> {
    let Boot { mut kernel, root, root_job, .. } = Kernel::boot()?;
    let browser_job = kernel.create_job(root, root_job)?;
    let renderers = kernel.create_job(root, browser_job)?;
    let browser = kernel.create_process(root, browser_job)?;
    let renderer = kernel.create_process(root, renderers)?;
    let neighbour = kernel.create_process(root, root_job)?;

    kernel.kill(root, browser_job)?;

    assert_eq!(kernel.handles(browser.id), Err(HandleError::ProcessNotFound));
    assert_eq!(kernel.handles(renderer.id), Err(HandleError::ProcessNotFound));
    assert_eq!(kernel.write(root, renderer.channel, b"", &[]), Err(HandleError::PeerClosed));
    kernel.handles(neighbour.id)?;
    assert_eq!(kernel.create_process(root, renderers).map(|created| created.id), Err(HandleError::BadState));

    Ok(())
}

#[test]
fn a_job_refuses_what_its_policy_denies_and_so_does_every_job_under_it() -> Result<(), HandleError> {
    let Boot { mut kernel, root, root_job, executable_resource } = Kernel::boot()?;
    let browser_job = kernel.create_job(root, root_job)?;

    kernel.set_policy(root, browser_job, &[Condition::ReplaceAsExecutable, Condition::NewProcess])?;

    let under = kernel.create_job(root, browser_job)?;
    let renderer = kernel.create_process(root, under)?;
    let memory = kernel.create_memory(root)?;
    let Ok(lent) = Rights::from_rights(&[Right::Duplicate, Right::Transfer]);
    let resource = kernel.duplicate(root, executable_resource, lent)?;

    kernel.write(root, renderer.channel, b"", &[memory, resource])?;

    let only = bootstrap(&mut kernel, renderer.id)?;
    let message = kernel.read(renderer.id, only)?;
    let given = message.handles;
    let memory = first(&given)?;
    let resource = second(&given)?;

    assert_eq!(
        kernel.replace_as_executable(renderer.id, memory, resource),
        Err(HandleError::DeniedByPolicy(Condition::ReplaceAsExecutable))
    );
    assert_eq!(
        kernel.create_process(renderer.id, only).map(|created| created.id),
        Err(HandleError::DeniedByPolicy(Condition::NewProcess))
    );

    Ok(())
}

#[test]
fn a_policy_is_set_on_an_empty_job_or_not_at_all() -> Result<(), HandleError> {
    let Boot { mut kernel, root, root_job, .. } = Kernel::boot()?;
    let job = kernel.create_job(root, root_job)?;
    let _started = kernel.create_process(root, job)?;

    assert_eq!(kernel.set_policy(root, job, &[Condition::NewChannel]), Err(HandleError::BadState));

    Ok(())
}

#[test]
fn revoking_takes_back_every_copy_made_from_a_handle_wherever_it_went_and_keeps_the_handle() -> Result<(), HandleError> {
    let Boot { mut kernel, root, root_job, .. } = Kernel::boot()?;
    let browser = kernel.create_process(root, root_job)?;
    let microphone = kernel.create_memory(root)?;
    let Ok(lent) = Rights::from_rights(&[Right::Read, Right::Duplicate, Right::Transfer]);
    let copy = kernel.duplicate(root, microphone, lent)?;

    kernel.write(root, browser.channel, b"", &[copy])?;

    let only = bootstrap(&mut kernel, browser.id)?;
    let message = kernel.read(browser.id, only)?;
    let held = first(&message.handles)?;
    let again = kernel.duplicate(browser.id, held, lent)?;

    kernel.revoke(root, microphone)?;

    assert_eq!(kernel.handle_rights(browser.id, held), Err(HandleError::BadHandle));
    assert_eq!(kernel.handle_rights(browser.id, again), Err(HandleError::BadHandle));
    kernel.handle_rights(root, microphone)?;

    Ok(())
}
