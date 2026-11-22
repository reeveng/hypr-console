use console_browser_site_isolation::{
  Browser, Opening, Parent, Permission, Sandbox, Site, SiteIsolationError,
};
use console_core_never::Never;
use console_kernel_handles::{
  Boot, Bytes, Condition, Contains, HANDLE_COST, HandleError, HandleValue, Kernel, MESSAGE_COST,
  OBJECT_COST, ObjectType, PolicyAction, ProcessId, Right, Rights, WAITING_MESSAGES_LIMIT,
};

const MACHINE: Bytes = Bytes(268_435_456);

const BROWSER_SHARE: Bytes = Bytes(16_777_216);

const SITE_SHARE: Bytes = Bytes(1_048_576);

const PAGE: Bytes = Bytes(4096);

const FRAME: &[u8] = b"a frame";

const FRAME_BYTES: u64 = 7;

const MICROPHONE: &[u8] = b"the microphone";

const EXECUTABLE_RESOURCE: &[u8] = b"the executable resource";

struct System {
  kernel: Kernel,
  root: ProcessId,
  executable_resource: HandleValue,
  browser: Browser,
  parent: Parent,
}

fn opened(denied: &[Condition]) -> Result<System, SiteIsolationError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    executable_resource,
  } = Kernel::boot(MACHINE)?;
  let (browser, parent) = Browser::open(
    &mut kernel,
    Opening {
      by: root,
      under: root_job,
      share: BROWSER_SHARE,
      denied,
    },
  )?;

  Ok(System {
    kernel,
    root,
    executable_resource,
    browser,
    parent,
  })
}

fn site(named: &str) -> Result<Site, Never> {
  Ok(Site(named.as_bytes().to_vec()))
}

fn sandbox(replace_as_executable: PolicyAction) -> Result<Sandbox, Never> {
  Ok(Sandbox {
    share: SITE_SHARE,
    replace_as_executable,
  })
}

fn bootstrap(kernel: &mut Kernel, process: ProcessId) -> Result<HandleValue, SiteIsolationError> {
  let held = kernel.handles(process)?;

  match held.first() {
    Some(handle) => Ok(*handle),
    None => Err(SiteIsolationError::UnexpectedMessage),
  }
}

fn received(
  kernel: &mut Kernel,
  renderer: ProcessId,
  named: &[u8],
) -> Result<HandleValue, SiteIsolationError> {
  let through = bootstrap(kernel, renderer)?;
  let message = kernel.read(renderer, through)?;

  assert_eq!(message.bytes, named.to_vec());

  match message.handles.as_slice() {
    [handle] => Ok(*handle),
    _ => Err(SiteIsolationError::UnexpectedMessage),
  }
}

fn lent_by_the_system(
  system: &mut System,
  named: &[u8],
  handle: HandleValue,
  narrowed: Rights,
) -> Result<Permission, SiteIsolationError> {
  let copy = system.kernel.duplicate(system.root, handle, narrowed)?;

  system
    .kernel
    .write(system.root, system.parent.channel, named, &[copy])?;

  system.browser.accept(&mut system.kernel)
}

fn lent_the_executable_resource(system: &mut System) -> Result<Permission, SiteIsolationError> {
  let Ok(lendable) = Rights::from_rights(&[Right::Duplicate, Right::Transfer]);

  lent_by_the_system(
    system,
    EXECUTABLE_RESOURCE,
    system.executable_resource,
    lendable,
  )
}

fn lent_the_microphone(
  system: &mut System,
) -> Result<(HandleValue, Permission), SiteIsolationError> {
  let microphone = system.kernel.create_memory(system.root, PAGE)?;
  let Ok(listening) = Rights::from_rights(&[Right::Read, Right::Duplicate, Right::Transfer]);
  let permission = lent_by_the_system(system, MICROPHONE, microphone, listening)?;

  Ok((microphone, permission))
}

fn run_written_code(
  kernel: &mut Kernel,
  renderer: ProcessId,
  resource: HandleValue,
) -> Result<HandleValue, HandleError> {
  let written = kernel.create_memory(renderer, PAGE)?;

  kernel.replace_as_executable(renderer, written, resource)
}

#[test]
fn the_browser_is_opened_in_a_job_of_its_own_whose_share_comes_out_of_the_job_it_was_given()
-> Result<(), SiteIsolationError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let before = kernel.budget(root, root_job)?;
  let objects = kernel.object_count()?;

  let (browser, parent) = Browser::open(
    &mut kernel,
    Opening {
      by: root,
      under: root_job,
      share: BROWSER_SHARE,
      denied: &[],
    },
  )?;

  let own = browser.budget(&mut kernel)?;
  assert_eq!(own.share, BROWSER_SHARE);
  let during = kernel.budget(root, root_job)?;
  assert!(during.charged.0 >= before.charged.0.saturating_add(BROWSER_SHARE.0));

  kernel.kill(root, parent.job)?;
  kernel.close(root, parent.process)?;
  kernel.close(root, parent.channel)?;
  kernel.close(root, parent.job)?;

  assert_eq!(
    browser.budget(&mut kernel),
    Err(SiteIsolationError::Kernel(HandleError::ProcessNotFound))
  );
  assert_eq!(kernel.budget(root, root_job), Ok(before));
  assert_eq!(kernel.object_count(), Ok(objects));

  Ok(())
}

#[test]
fn a_browser_that_cannot_be_opened_leaves_nothing_behind() -> Result<(), SiteIsolationError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let before = kernel.budget(root, root_job)?;
  let objects = kernel.object_count()?;

  let refused = match Browser::open(
    &mut kernel,
    Opening {
      by: root,
      under: root_job,
      share: Bytes(0),
      denied: &[],
    },
  ) {
    Ok(_opened) => Ok(()),
    Err(fault) => Err(fault),
  };

  assert_eq!(
    refused,
    Err(SiteIsolationError::Kernel(HandleError::OverBudget))
  );
  assert_eq!(kernel.budget(root, root_job), Ok(before));
  assert_eq!(kernel.object_count(), Ok(objects));

  Ok(())
}

#[test]
fn the_first_tab_on_a_site_starts_a_renderer_in_a_job_with_its_own_share_that_may_start_nothing()
-> Result<(), SiteIsolationError> {
  let mut system = opened(&[])?;
  let Ok(news) = site("https://news.example");
  let Ok(strict) = sandbox(PolicyAction::Deny);
  let before = system.browser.budget(&mut system.kernel)?;

  system.browser.open_tab(&mut system.kernel, &news, strict)?;

  let renderer = system.browser.renderer(&news)?;
  let own = system.browser.site_budget(&mut system.kernel, &news)?;
  assert_eq!(own.share, SITE_SHARE);
  let after = system.browser.budget(&mut system.kernel)?;
  assert!(after.charged.0 >= before.charged.0.saturating_add(SITE_SHARE.0));
  let only = bootstrap(&mut system.kernel, renderer)?;
  assert_eq!(system.kernel.handles(renderer), Ok(vec![only]));
  assert_eq!(
    system.kernel.create_job(renderer, only, PAGE),
    Err(HandleError::DeniedByPolicy(Condition::NewJob))
  );
  assert_eq!(
    system.kernel.create_process(renderer, only),
    Err(HandleError::DeniedByPolicy(Condition::NewProcess))
  );

  Ok(())
}

#[test]
fn a_second_tab_on_a_site_is_drawn_by_the_renderer_the_first_one_started()
-> Result<(), SiteIsolationError> {
  let mut system = opened(&[])?;
  let Ok(news) = site("https://news.example");
  let Ok(strict) = sandbox(PolicyAction::Deny);
  let first = system.browser.open_tab(&mut system.kernel, &news, strict)?;
  let renderer = system.browser.renderer(&news)?;
  let before = system.browser.budget(&mut system.kernel)?;
  let objects = system.kernel.object_count()?;

  let second = system.browser.open_tab(&mut system.kernel, &news, strict)?;

  assert_ne!(first, second);
  assert_eq!(system.browser.renderer(&news), Ok(renderer));
  assert_eq!(system.browser.budget(&mut system.kernel), Ok(before));
  assert_eq!(system.kernel.object_count(), Ok(objects));

  Ok(())
}

#[test]
fn a_second_tab_that_asks_for_another_sandbox_than_the_site_has_is_refused()
-> Result<(), SiteIsolationError> {
  let mut system = opened(&[])?;
  let Ok(news) = site("https://news.example");
  let Ok(strict) = sandbox(PolicyAction::Deny);
  let Ok(looser) = sandbox(PolicyAction::Allow);

  system.browser.open_tab(&mut system.kernel, &news, strict)?;

  assert_eq!(
    system.browser.open_tab(&mut system.kernel, &news, looser),
    Err(SiteIsolationError::SandboxDiffers)
  );

  Ok(())
}

#[test]
fn a_tab_on_another_site_is_paid_for_and_ended_apart_from_the_first()
-> Result<(), SiteIsolationError> {
  let mut system = opened(&[])?;
  let Ok(news) = site("https://news.example");
  let Ok(bank) = site("https://bank.example");
  let Ok(strict) = sandbox(PolicyAction::Deny);
  let reading = system.browser.open_tab(&mut system.kernel, &news, strict)?;

  system.browser.open_tab(&mut system.kernel, &bank, strict)?;

  let news_renderer = system.browser.renderer(&news)?;
  let bank_renderer = system.browser.renderer(&bank)?;
  assert_ne!(news_renderer, bank_renderer);

  let untouched = system.browser.site_budget(&mut system.kernel, &bank)?;
  let before = system.browser.site_budget(&mut system.kernel, &news)?;
  system.kernel.create_memory(news_renderer, PAGE)?;
  let after = system.browser.site_budget(&mut system.kernel, &news)?;
  assert!(after.charged > before.charged);
  assert_eq!(
    system.browser.site_budget(&mut system.kernel, &bank),
    Ok(untouched)
  );

  system.browser.close_tab(&mut system.kernel, reading)?;

  assert_eq!(
    system.kernel.handles(news_renderer),
    Err(HandleError::ProcessNotFound)
  );
  assert_eq!(system.browser.renderer(&bank), Ok(bank_renderer));
  system.browser.send(&mut system.kernel, &bank, FRAME)?;
  assert_eq!(
    received_bytes(&mut system.kernel, bank_renderer),
    Ok(FRAME.to_vec())
  );

  Ok(())
}

fn received_bytes(
  kernel: &mut Kernel,
  renderer: ProcessId,
) -> Result<Vec<u8>, SiteIsolationError> {
  let through = bootstrap(kernel, renderer)?;
  let message = kernel.read(renderer, through)?;

  Ok(message.bytes)
}

#[test]
fn written_code_runs_where_the_browser_passed_the_resource_on_and_not_in_the_site_beside_it()
-> Result<(), SiteIsolationError> {
  let mut system = opened(&[])?;
  let Ok(maps) = site("https://maps.example");
  let Ok(news) = site("https://news.example");
  let Ok(compiling) = sandbox(PolicyAction::Allow);
  let Ok(passed_on) = Rights::from_rights(&[Right::Transfer]);
  let entitlement = lent_the_executable_resource(&mut system)?;

  system
    .browser
    .open_tab(&mut system.kernel, &maps, compiling)?;
  system
    .browser
    .open_tab(&mut system.kernel, &news, compiling)?;
  system
    .browser
    .lend(&mut system.kernel, &maps, &entitlement, passed_on)?;

  let entitled = system.browser.renderer(&maps)?;
  let resource = received(&mut system.kernel, entitled, EXECUTABLE_RESOURCE)?;
  let running = run_written_code(&mut system.kernel, entitled, resource)?;
  let granted = system.kernel.handle_rights(entitled, running)?;
  assert_eq!(granted.contains(Right::Execute), Ok(Contains::Yes));
  assert_eq!(granted.contains(Right::Write), Ok(Contains::No));

  let neighbour = system.browser.renderer(&news)?;
  let only = bootstrap(&mut system.kernel, neighbour)?;
  assert_eq!(system.kernel.handles(neighbour), Ok(vec![only]));
  assert_eq!(
    system.kernel.read(neighbour, only),
    Err(HandleError::ShouldWait)
  );
  assert_eq!(
    run_written_code(&mut system.kernel, neighbour, only),
    Err(HandleError::WrongType(ObjectType::ExecutableResource))
  );

  Ok(())
}

#[test]
fn a_site_whose_job_denies_it_is_refused_even_holding_the_resource()
-> Result<(), SiteIsolationError> {
  let mut system = opened(&[])?;
  let Ok(news) = site("https://news.example");
  let Ok(strict) = sandbox(PolicyAction::Deny);
  let Ok(passed_on) = Rights::from_rights(&[Right::Transfer]);
  let entitlement = lent_the_executable_resource(&mut system)?;

  system.browser.open_tab(&mut system.kernel, &news, strict)?;
  system
    .browser
    .lend(&mut system.kernel, &news, &entitlement, passed_on)?;

  let renderer = system.browser.renderer(&news)?;
  let resource = received(&mut system.kernel, renderer, EXECUTABLE_RESOURCE)?;
  system.kernel.handle_rights(renderer, resource)?;
  assert_eq!(
    run_written_code(&mut system.kernel, renderer, resource),
    Err(HandleError::DeniedByPolicy(Condition::ReplaceAsExecutable))
  );

  Ok(())
}

#[test]
fn a_browser_whose_own_job_denies_it_can_entitle_no_site() -> Result<(), SiteIsolationError> {
  let mut system = opened(&[Condition::ReplaceAsExecutable])?;
  let Ok(maps) = site("https://maps.example");
  let Ok(compiling) = sandbox(PolicyAction::Allow);
  let Ok(passed_on) = Rights::from_rights(&[Right::Transfer]);
  let entitlement = lent_the_executable_resource(&mut system)?;

  system
    .browser
    .open_tab(&mut system.kernel, &maps, compiling)?;
  system
    .browser
    .lend(&mut system.kernel, &maps, &entitlement, passed_on)?;

  let renderer = system.browser.renderer(&maps)?;
  let resource = received(&mut system.kernel, renderer, EXECUTABLE_RESOURCE)?;
  assert_eq!(
    run_written_code(&mut system.kernel, renderer, resource),
    Err(HandleError::DeniedByPolicy(Condition::ReplaceAsExecutable))
  );

  Ok(())
}

#[test]
fn a_browser_that_was_lent_nothing_has_nothing_to_pass_on() -> Result<(), SiteIsolationError> {
  let mut system = opened(&[])?;
  let Ok(maps) = site("https://maps.example");
  let Ok(compiling) = sandbox(PolicyAction::Allow);
  let Ok(passed_on) = Rights::from_rights(&[Right::Transfer]);
  let never_lent = Permission(EXECUTABLE_RESOURCE.to_vec());

  system
    .browser
    .open_tab(&mut system.kernel, &maps, compiling)?;

  assert_eq!(
    system
      .browser
      .lend(&mut system.kernel, &maps, &never_lent, passed_on),
    Err(SiteIsolationError::PermissionNotFound)
  );
  let renderer = system.browser.renderer(&maps)?;
  let only = bootstrap(&mut system.kernel, renderer)?;
  assert_eq!(
    system.kernel.read(renderer, only),
    Err(HandleError::ShouldWait)
  );

  Ok(())
}

#[test]
fn the_system_taking_the_resource_back_takes_it_from_the_browser_and_from_every_renderer()
-> Result<(), SiteIsolationError> {
  let mut system = opened(&[])?;
  let Ok(maps) = site("https://maps.example");
  let Ok(mail) = site("https://mail.example");
  let Ok(compiling) = sandbox(PolicyAction::Allow);
  let Ok(passed_on) = Rights::from_rights(&[Right::Transfer]);
  let entitlement = lent_the_executable_resource(&mut system)?;

  system
    .browser
    .open_tab(&mut system.kernel, &maps, compiling)?;
  let reading = system
    .browser
    .open_tab(&mut system.kernel, &mail, compiling)?;
  system
    .browser
    .lend(&mut system.kernel, &maps, &entitlement, passed_on)?;
  system
    .browser
    .lend(&mut system.kernel, &mail, &entitlement, passed_on)?;

  let first = system.browser.renderer(&maps)?;
  let second = system.browser.renderer(&mail)?;
  let held_by_first = received(&mut system.kernel, first, EXECUTABLE_RESOURCE)?;
  let held_by_second = received(&mut system.kernel, second, EXECUTABLE_RESOURCE)?;
  run_written_code(&mut system.kernel, first, held_by_first)?;

  system
    .kernel
    .revoke(system.root, system.executable_resource)?;

  assert_eq!(
    run_written_code(&mut system.kernel, first, held_by_first),
    Err(HandleError::BadHandle)
  );
  assert_eq!(
    run_written_code(&mut system.kernel, second, held_by_second),
    Err(HandleError::BadHandle)
  );
  assert_eq!(
    system
      .browser
      .lend(&mut system.kernel, &maps, &entitlement, passed_on),
    Err(SiteIsolationError::AlreadyLent)
  );
  system
    .browser
    .withdraw(&mut system.kernel, &maps, &entitlement)?;
  assert_eq!(
    system
      .browser
      .lend(&mut system.kernel, &maps, &entitlement, passed_on),
    Err(SiteIsolationError::Kernel(HandleError::BadHandle))
  );
  system.browser.close_tab(&mut system.kernel, reading)?;
  assert_eq!(
    system.kernel.handles(second),
    Err(HandleError::ProcessNotFound)
  );

  Ok(())
}

#[test]
fn a_permission_withdrawn_leaves_the_renderer_nothing_however_it_copied_what_it_was_lent()
-> Result<(), SiteIsolationError> {
  let mut system = opened(&[])?;
  let Ok(calls) = site("https://calls.example");
  let Ok(news) = site("https://news.example");
  let Ok(strict) = sandbox(PolicyAction::Deny);
  let Ok(listening) = Rights::from_rights(&[Right::Read, Right::Duplicate, Right::Transfer]);
  let Ok(hidden) = Rights::from_rights(&[Right::Read, Right::Transfer]);
  let (microphone, permission) = lent_the_microphone(&mut system)?;

  system
    .browser
    .open_tab(&mut system.kernel, &calls, strict)?;
  system.browser.open_tab(&mut system.kernel, &news, strict)?;
  system
    .browser
    .lend(&mut system.kernel, &calls, &permission, listening)?;

  let renderer = system.browser.renderer(&calls)?;
  let lent = received(&mut system.kernel, renderer, MICROPHONE)?;
  assert_eq!(system.kernel.handle_rights(renderer, lent), Ok(listening));
  let copy = system.kernel.duplicate(renderer, lent, listening)?;
  let second = system.kernel.duplicate(renderer, copy, hidden)?;
  system.kernel.close(renderer, lent)?;
  let (near, far) = system.kernel.create_channel(renderer)?;
  system
    .kernel
    .write(renderer, near, b"put away", &[second])?;
  let only = bootstrap(&mut system.kernel, renderer)?;

  system
    .browser
    .withdraw(&mut system.kernel, &calls, &permission)?;

  assert_eq!(
    system.kernel.handle_rights(renderer, copy),
    Err(HandleError::BadHandle)
  );
  assert_eq!(system.kernel.handles(renderer), Ok(vec![only, near, far]));
  let put_away = system.kernel.read(renderer, far)?;
  assert!(put_away.handles.is_empty());

  let neighbour = system.browser.renderer(&news)?;
  let theirs = bootstrap(&mut system.kernel, neighbour)?;
  assert_eq!(system.kernel.handles(neighbour), Ok(vec![theirs]));
  assert_eq!(
    system.kernel.read(neighbour, theirs),
    Err(HandleError::ShouldWait)
  );
  system.kernel.handle_rights(system.root, microphone)?;

  Ok(())
}

#[test]
fn a_permission_withdrawn_from_one_site_stays_with_the_other_and_can_be_lent_again()
-> Result<(), SiteIsolationError> {
  let mut system = opened(&[])?;
  let Ok(calls) = site("https://calls.example");
  let Ok(notes) = site("https://notes.example");
  let Ok(strict) = sandbox(PolicyAction::Deny);
  let Ok(listening) = Rights::from_rights(&[Right::Read, Right::Transfer]);
  let (_microphone, permission) = lent_the_microphone(&mut system)?;

  system
    .browser
    .open_tab(&mut system.kernel, &calls, strict)?;
  system
    .browser
    .open_tab(&mut system.kernel, &notes, strict)?;

  let before = system.browser.budget(&mut system.kernel)?;
  system
    .browser
    .lend(&mut system.kernel, &calls, &permission, listening)?;
  system
    .browser
    .lend(&mut system.kernel, &notes, &permission, listening)?;
  assert_eq!(
    system
      .browser
      .lend(&mut system.kernel, &notes, &permission, listening),
    Err(SiteIsolationError::AlreadyLent)
  );
  let taken_from = system.browser.renderer(&calls)?;
  let left_with = system.browser.renderer(&notes)?;
  let taken = received(&mut system.kernel, taken_from, MICROPHONE)?;
  let left = received(&mut system.kernel, left_with, MICROPHONE)?;

  system
    .browser
    .withdraw(&mut system.kernel, &calls, &permission)?;

  assert_eq!(
    system.kernel.handle_rights(taken_from, taken),
    Err(HandleError::BadHandle)
  );
  assert_eq!(system.kernel.handle_rights(left_with, left), Ok(listening));
  assert_eq!(
    system
      .browser
      .withdraw(&mut system.kernel, &calls, &permission),
    Err(SiteIsolationError::NotLent)
  );

  system
    .browser
    .withdraw(&mut system.kernel, &notes, &permission)?;

  assert_eq!(system.browser.budget(&mut system.kernel), Ok(before));

  system
    .browser
    .lend(&mut system.kernel, &calls, &permission, listening)?;

  let again = received(&mut system.kernel, taken_from, MICROPHONE)?;
  assert_eq!(
    system.kernel.handle_rights(taken_from, again),
    Ok(listening)
  );

  Ok(())
}

#[test]
fn closing_one_of_two_tabs_on_a_site_leaves_its_renderer_running()
-> Result<(), SiteIsolationError> {
  let mut system = opened(&[])?;
  let Ok(news) = site("https://news.example");
  let Ok(strict) = sandbox(PolicyAction::Deny);
  let first = system.browser.open_tab(&mut system.kernel, &news, strict)?;
  let second = system.browser.open_tab(&mut system.kernel, &news, strict)?;
  let renderer = system.browser.renderer(&news)?;

  system.browser.close_tab(&mut system.kernel, first)?;

  assert_eq!(system.browser.renderer(&news), Ok(renderer));
  system.kernel.handles(renderer)?;
  assert_eq!(
    system.browser.close_tab(&mut system.kernel, first),
    Err(SiteIsolationError::TabNotFound)
  );

  system.browser.close_tab(&mut system.kernel, second)?;

  assert_eq!(
    system.browser.renderer(&news),
    Err(SiteIsolationError::SiteNotFound)
  );
  assert_eq!(
    system.kernel.handles(renderer),
    Err(HandleError::ProcessNotFound)
  );

  Ok(())
}

#[test]
fn replacing_a_renderer_keeps_the_tabs_but_retires_its_messages_and_grants()
-> Result<(), SiteIsolationError> {
  let mut system = opened(&[])?;
  let Ok(calls) = site("https://calls.example");
  let Ok(strict) = sandbox(PolicyAction::Deny);
  let (_microphone, permission) = lent_the_microphone(&mut system)?;
  let first = system
    .browser
    .open_tab(&mut system.kernel, &calls, strict)?;
  let second = system
    .browser
    .open_tab(&mut system.kernel, &calls, strict)?;
  let old = system.browser.renderer(&calls)?;
  let old_channel = bootstrap(&mut system.kernel, old)?;
  let browser_budget = system.browser.budget(&mut system.kernel)?;
  let site_budget = system.browser.site_budget(&mut system.kernel, &calls)?;
  let objects = system.kernel.object_count()?;

  let Ok(listening) = Rights::from_rights(&[Right::Read, Right::Transfer]);
  let grant_in_browser = queue_renderer_messages(
    &mut system,
    &calls,
    &permission,
    listening,
    old,
    old_channel,
  )?;
  let replacement = system
    .browser
    .replace_renderer(&mut system.kernel, &calls)?;
  let next_channel = bootstrap(&mut system.kernel, replacement)?;
  let Ok(browser) = system.browser.process();

  assert_ne!(replacement, old);
  assert_eq!(
    system.kernel.handles(old),
    Err(HandleError::ProcessNotFound)
  );
  assert_eq!(system.kernel.handles(replacement), Ok(vec![next_channel]));
  assert_eq!(
    system.kernel.read(replacement, next_channel),
    Err(HandleError::ShouldWait)
  );
  assert_eq!(
    system.browser.receive(&mut system.kernel, &calls),
    Err(SiteIsolationError::from(HandleError::ShouldWait))
  );
  assert_eq!(
    system.kernel.handle_rights(browser, grant_in_browser),
    Err(HandleError::BadHandle)
  );
  assert_eq!(
    system.browser.budget(&mut system.kernel),
    Ok(browser_budget)
  );
  assert_eq!(
    system.browser.site_budget(&mut system.kernel, &calls),
    Ok(site_budget)
  );
  assert_eq!(system.kernel.object_count(), Ok(objects));

  system
    .browser
    .lend(&mut system.kernel, &calls, &permission, listening)?;
  received(&mut system.kernel, replacement, MICROPHONE)?;
  system.browser.close_tab(&mut system.kernel, first)?;
  assert_eq!(system.browser.renderer(&calls), Ok(replacement));
  system.browser.close_tab(&mut system.kernel, second)?;
  assert_eq!(
    system.kernel.handles(replacement),
    Err(HandleError::ProcessNotFound)
  );

  Ok(())
}

fn queue_renderer_messages(
  system: &mut System,
  site: &Site,
  permission: &Permission,
  listening: Rights,
  renderer: ProcessId,
  channel: HandleValue,
) -> Result<HandleValue, SiteIsolationError> {
  system
    .browser
    .lend(&mut system.kernel, site, permission, listening)?;

  let granted = received(&mut system.kernel, renderer, MICROPHONE)?;

  system
    .kernel
    .write(renderer, channel, b"returned grant", &[granted])?;

  let returned = system.browser.receive(&mut system.kernel, site)?;
  let grant_in_browser = match returned.handles.as_slice() {
    [handle] => *handle,
    _ => return Err(SiteIsolationError::UnexpectedMessage),
  };

  system
    .kernel
    .write(renderer, channel, b"outgoing frame", &[])?;
  system
    .browser
    .send(&mut system.kernel, site, b"pending input")?;

  Ok(grant_in_browser)
}

#[test]
fn a_replacement_that_cannot_start_leaves_the_current_renderer_running()
-> Result<(), SiteIsolationError> {
  let mut system = opened(&[])?;
  let Ok(news) = site("https://news.example");
  let Ok(strict) = sandbox(PolicyAction::Deny);
  let tab = system.browser.open_tab(&mut system.kernel, &news, strict)?;
  let current = system.browser.renderer(&news)?;
  let budget = system.browser.site_budget(&mut system.kernel, &news)?;
  let bytes = budget
    .share
    .0
    .checked_sub(budget.charged.0)
    .and_then(|available| available.checked_sub(OBJECT_COST.0))
    .and_then(|after_object| after_object.checked_sub(HANDLE_COST.0))
    .ok_or(HandleError::OverBudget)?;
  let memory = system.kernel.create_memory(current, Bytes(bytes))?;
  let before = system.browser.budget(&mut system.kernel)?;
  let objects = system.kernel.object_count()?;

  assert_eq!(
    system.browser.replace_renderer(&mut system.kernel, &news),
    Err(SiteIsolationError::from(HandleError::OverBudget))
  );
  assert_eq!(system.browser.renderer(&news), Ok(current));
  assert_eq!(system.browser.budget(&mut system.kernel), Ok(before));
  assert_eq!(system.kernel.object_count(), Ok(objects));
  system.browser.send(&mut system.kernel, &news, FRAME)?;

  let frame = received_bytes(&mut system.kernel, current)?;

  assert_eq!(frame, FRAME);

  system.kernel.close(current, memory)?;
  let replacement = system.browser.replace_renderer(&mut system.kernel, &news)?;
  assert_ne!(replacement, current);
  system.browser.close_tab(&mut system.kernel, tab)?;

  Ok(())
}

#[test]
fn closing_the_last_tab_on_a_site_ends_its_job_and_the_browser_has_what_it_had_before_the_tab()
-> Result<(), SiteIsolationError> {
  let mut system = opened(&[])?;
  let Ok(calls) = site("https://calls.example");
  let Ok(strict) = sandbox(PolicyAction::Deny);
  let Ok(listening) = Rights::from_rights(&[Right::Read, Right::Duplicate, Right::Transfer]);
  let (_microphone, permission) = lent_the_microphone(&mut system)?;
  let before = system.browser.budget(&mut system.kernel)?;
  let objects = system.kernel.object_count()?;

  let tab = system
    .browser
    .open_tab(&mut system.kernel, &calls, strict)?;
  let renderer = system.browser.renderer(&calls)?;
  let only = bootstrap(&mut system.kernel, renderer)?;
  let kept = system.kernel.create_memory(renderer, PAGE)?;
  let sent = system.kernel.create_memory(renderer, PAGE)?;
  system
    .kernel
    .write(renderer, only, b"nobody reads this", &[sent])?;
  system.browser.send(&mut system.kernel, &calls, FRAME)?;
  system
    .browser
    .lend(&mut system.kernel, &calls, &permission, listening)?;
  let lent = received_after_a_frame(&mut system.kernel, renderer)?;
  system.kernel.duplicate(renderer, lent, listening)?;
  system.kernel.handle_rights(renderer, kept)?;
  let during = system.browser.budget(&mut system.kernel)?;
  assert!(during.charged > before.charged);

  system.browser.close_tab(&mut system.kernel, tab)?;

  assert_eq!(
    system.kernel.handles(renderer),
    Err(HandleError::ProcessNotFound)
  );
  assert_eq!(system.browser.budget(&mut system.kernel), Ok(before));
  assert_eq!(system.kernel.object_count(), Ok(objects));

  Ok(())
}

fn received_after_a_frame(
  kernel: &mut Kernel,
  renderer: ProcessId,
) -> Result<HandleValue, SiteIsolationError> {
  let frame = received_bytes(kernel, renderer)?;

  assert_eq!(frame, FRAME.to_vec());

  received(kernel, renderer, MICROPHONE)
}

#[test]
fn a_tab_the_browser_cannot_start_a_renderer_for_leaves_nothing_behind()
-> Result<(), SiteIsolationError> {
  let mut system = opened(&[])?;
  let Ok(news) = site("https://news.example");
  let before = system.browser.budget(&mut system.kernel)?;
  let objects = system.kernel.object_count()?;
  let nothing = Sandbox {
    share: Bytes(0),
    replace_as_executable: PolicyAction::Deny,
  };
  let everything = Sandbox {
    share: BROWSER_SHARE,
    replace_as_executable: PolicyAction::Deny,
  };

  assert_eq!(
    system.browser.open_tab(&mut system.kernel, &news, nothing),
    Err(SiteIsolationError::Kernel(HandleError::OverBudget))
  );
  assert_eq!(
    system
      .browser
      .open_tab(&mut system.kernel, &news, everything),
    Err(SiteIsolationError::Kernel(HandleError::OverBudget))
  );

  assert_eq!(
    system.browser.renderer(&news),
    Err(SiteIsolationError::SiteNotFound)
  );
  assert_eq!(system.browser.budget(&mut system.kernel), Ok(before));
  assert_eq!(system.kernel.object_count(), Ok(objects));

  Ok(())
}

#[test]
fn a_renderer_that_never_reads_stops_the_browser_at_the_channel_bound_and_costs_it_no_more()
-> Result<(), SiteIsolationError> {
  let mut system = opened(&[])?;
  let Ok(deaf) = site("https://deaf.example");
  let Ok(news) = site("https://news.example");
  let Ok(strict) = sandbox(PolicyAction::Deny);
  let Ok(listening) = Rights::from_rights(&[Right::Read, Right::Transfer]);
  let (_microphone, permission) = lent_the_microphone(&mut system)?;

  system.browser.open_tab(&mut system.kernel, &deaf, strict)?;
  system.browser.open_tab(&mut system.kernel, &news, strict)?;

  let before = system.browser.budget(&mut system.kernel)?;

  for _ in 0..WAITING_MESSAGES_LIMIT {
    system.browser.send(&mut system.kernel, &deaf, FRAME)?;
  }

  let full = Err(SiteIsolationError::Kernel(HandleError::ChannelFull));
  let spent =
    u64::from(WAITING_MESSAGES_LIMIT).saturating_mul(FRAME_BYTES.saturating_add(MESSAGE_COST.0));
  let bound = Bytes(before.charged.0.saturating_add(spent));

  assert_eq!(system.browser.send(&mut system.kernel, &deaf, FRAME), full);
  assert_eq!(
    system
      .browser
      .lend(&mut system.kernel, &deaf, &permission, listening),
    full
  );
  let stopped = system.browser.budget(&mut system.kernel)?;
  assert_eq!(stopped.charged, bound);

  system.browser.send(&mut system.kernel, &news, FRAME)?;

  Ok(())
}

#[test]
fn a_renderer_that_asks_for_more_than_its_sites_share_is_refused_and_the_site_beside_it_is_not()
-> Result<(), SiteIsolationError> {
  let mut system = opened(&[])?;
  let Ok(greedy) = site("https://greedy.example");
  let Ok(news) = site("https://news.example");
  let Ok(strict) = sandbox(PolicyAction::Deny);

  system
    .browser
    .open_tab(&mut system.kernel, &greedy, strict)?;
  system.browser.open_tab(&mut system.kernel, &news, strict)?;

  let asking = system.browser.renderer(&greedy)?;
  let neighbour = system.browser.renderer(&news)?;
  let theirs = system.browser.site_budget(&mut system.kernel, &news)?;
  let browsers = system.browser.budget(&mut system.kernel)?;

  assert_eq!(
    system.kernel.create_memory(asking, SITE_SHARE),
    Err(HandleError::OverBudget)
  );

  assert_eq!(
    system.browser.site_budget(&mut system.kernel, &news),
    Ok(theirs)
  );
  assert_eq!(system.browser.budget(&mut system.kernel), Ok(browsers));
  system.kernel.create_memory(neighbour, PAGE)?;

  Ok(())
}
