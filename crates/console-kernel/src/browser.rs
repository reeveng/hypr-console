//! The browser walkthrough shared by the hosted runner and firmware.
//! The renderer runs synchronously under a process ID in the kernel's handle model.

#![forbid(unsafe_code)]

mod page;

use alloc::string::String;
use core::{fmt, str};

use console_browser_site_isolation::{
  Browser, Opening, Parent, Permission, Sandbox, Site, SiteIsolationError, Tab,
};
use console_core_geometry::Size;
use console_core_web_engine::{
  Event, FrameLayout, Page, Request, decoded_event, decoded_request, encoded_event,
  encoded_request,
};
use console_kernel_handles::{
  Boot, Bytes, Condition, HandleError, HandleValue, Kernel, Message, PolicyAction, ProcessId,
  Right, Rights, WAITING_MESSAGES_LIMIT,
};

use crate::BROWSER_SHARE;

const SITE_SHARE: Bytes = Bytes(960 * 1024);

const VIEWPORT: Size<u32> = Size {
  width: 640,
  height: 360,
};

const ADDRESS: &str = concat!(
  "data:text/plain,Console%20Browser%0A%0A",
  "A%20site%20job%20owns%20this%20frame.%0A",
  "Pixels%20shared%20by%20the%20kernel.",
);

const EXECUTABLE_RESOURCE: &[u8] = b"executable resource";
const BAD_STATE: SiteIsolationError = SiteIsolationError::Kernel(HandleError::BadState);

struct Renderer {
  process: ProcessId,
  channel: HandleValue,
}

pub(super) fn run(
  memory: Bytes,
  log: &mut impl FnMut(fmt::Arguments<'_>),
  present: &mut impl FnMut(Size<u32>, &[[u8; 4]]),
) -> Result<(), HandleError> {
  steps(memory, log, present).map_err(|fault| match fault {
    SiteIsolationError::Kernel(error) => error,
    SiteIsolationError::TabNotFound
    | SiteIsolationError::SiteNotFound
    | SiteIsolationError::PermissionNotFound
    | SiteIsolationError::SandboxDiffers
    | SiteIsolationError::AlreadyLent
    | SiteIsolationError::NotLent
    | SiteIsolationError::UnexpectedMessage
    | SiteIsolationError::NoResources => HandleError::BadState,
  })
}

#[cfg_attr(
  dylint_lib = "explicit066_no_heap_after_init",
  expect(
    explicit066_no_heap_after_init,
    reason = "the browser walkthrough creates its owned URL during setup, before receiving requests"
  )
)]
fn steps(
  memory: Bytes,
  log: &mut impl FnMut(fmt::Arguments<'_>),
  present: &mut impl FnMut(Size<u32>, &[[u8; 4]]),
) -> Result<(), SiteIsolationError> {
  let frame_bytes = page::frame_bytes(VIEWPORT)?;
  let Boot {
    mut kernel,
    root,
    root_job,
    executable_resource,
  } = Kernel::boot_with_memory_backing(memory, frame_bytes, 1)?;
  let root_before = kernel.budget(root, root_job)?;
  let root_objects = kernel.object_count()?;
  let (mut browser, parent) = Browser::open(
    &mut kernel,
    Opening {
      by: root,
      under: root_job,
      share: BROWSER_SHARE,
      denied: &[Condition::ReplaceAsExecutable],
    },
  )?;
  let browser_before = browser.budget(&mut kernel)?;
  let browser_objects = kernel.object_count()?;
  let Ok(lendable) = Rights::from_rights(&[Right::Duplicate, Right::Transfer]);
  let offered = kernel.duplicate(root, executable_resource, lendable)?;
  kernel.write(root, parent.channel, EXECUTABLE_RESOURCE, &[offered])?;
  let permission = browser.accept(&mut kernel)?;

  let ActiveSite {
    site,
    tab,
    renderer,
    resource,
  } = open_site(&mut kernel, &mut browser, &permission, log)?;
  let mut page = Page {
    address: String::from(ADDRESS),
    viewport: VIEWPORT,
  };

  exchange_frame(&mut kernel, &browser, &site, &renderer, &mut page, present)?;
  log(format_args!(
    "a {} by {} plain-text page reached the browser as read-only memory",
    VIEWPORT.width, VIEWPORT.height,
  ));

  exercise_sandbox(&mut kernel, &browser, &site, &renderer, resource, log)?;

  kernel.revoke(root, executable_resource)?;

  match ::core::primitive::bool::then_some(
    kernel.handle_rights(renderer.process, resource) == Err(HandleError::BadHandle),
    (),
  ) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(BAD_STATE),
  }

  log(format_args!(
    "the root took back the executable resource from the renderer"
  ));

  let replacement = browser.replace_renderer(&mut kernel, &site)?;

  match ::core::primitive::bool::then_some(
    kernel.handles(renderer.process) == Err(HandleError::ProcessNotFound),
    (),
  ) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(BAD_STATE),
  }

  let next = Renderer::bootstrap(&mut kernel, replacement)?;
  exchange_frame(&mut kernel, &browser, &site, &next, &mut page, present)?;
  log(format_args!(
    "the replacement renderer reopened the page in the same tab"
  ));

  browser.close_tab(&mut kernel, tab)?;

  check_site_closed(
    &mut kernel,
    &browser,
    replacement,
    browser_before,
    browser_objects,
  )?;

  log(format_args!(
    "closing the last tab ended the renderer and returned the site's share"
  ));

  finish_browser(
    &mut kernel,
    root,
    root_job,
    parent,
    root_before,
    root_objects,
  )
}

fn finish_browser(
  kernel: &mut Kernel,
  root: ProcessId,
  root_job: HandleValue,
  parent: Parent,
  budget_before: console_kernel_handles::Budget,
  objects_before: u64,
) -> Result<(), SiteIsolationError> {
  kernel.kill(root, parent.job)?;
  kernel.close(root, parent.channel)?;
  kernel.close(root, parent.process)?;
  kernel.close(root, parent.job)?;

  let root_budget = kernel.budget(root, root_job)?;
  let remaining_objects = kernel.object_count()?;

  match (root_budget == budget_before && remaining_objects == objects_before).then_some(()) {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => Err(BAD_STATE),
  }
}

struct ActiveSite {
  site: Site,
  tab: Tab,
  renderer: Renderer,
  resource: HandleValue,
}

fn open_site(
  kernel: &mut Kernel,
  browser: &mut Browser,
  permission: &Permission,
  log: &mut impl FnMut(fmt::Arguments<'_>),
) -> Result<ActiveSite, SiteIsolationError> {
  let site = Site(b"local document".to_vec());
  let tab = browser.open_tab(
    kernel,
    &site,
    Sandbox {
      share: SITE_SHARE,
      replace_as_executable: PolicyAction::Deny,
    },
  )?;
  let renderer_process = browser.renderer(&site)?;
  let renderer = Renderer::bootstrap(kernel, renderer_process)?;
  log(format_args!(
    "the browser opened a site in a job with {} bytes",
    SITE_SHARE.0
  ));

  let Ok(transfer) = Rights::from_rights(&[Right::Transfer]);
  browser.lend(kernel, &site, permission, transfer)?;
  let grant = kernel.read(renderer.process, renderer.channel)?;

  let resource = match grant.handles.as_slice() {
    [only] => *only,
    _ => return Err(BAD_STATE),
  };

  Ok(ActiveSite {
    site,
    tab,
    renderer,
    resource,
  })
}

fn exercise_sandbox(
  kernel: &mut Kernel,
  browser: &Browser,
  site: &Site,
  renderer: &Renderer,
  resource: HandleValue,
  log: &mut impl FnMut(fmt::Arguments<'_>),
) -> Result<(), SiteIsolationError> {
  let allocation = kernel.create_memory(renderer.process, Bytes(0))?;

  match ::core::primitive::bool::then_some(
    kernel.replace_as_executable(renderer.process, allocation, resource)
      == Err(HandleError::DeniedByPolicy(Condition::ReplaceAsExecutable)),
    (),
  ) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(BAD_STATE),
  }

  kernel.close(renderer.process, allocation)?;
  log(format_args!(
    "the site's job refused to make memory executable even with the resource"
  ));
  let Ok(reload) = encoded_request(&Request::Reload);

  for _ in 0..WAITING_MESSAGES_LIMIT {
    browser.send(kernel, site, reload.as_bytes())?;
  }

  match ::core::primitive::bool::then_some(
    browser.send(kernel, site, reload.as_bytes())
      == Err(SiteIsolationError::Kernel(HandleError::ChannelFull)),
    (),
  ) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(BAD_STATE),
  }

  log(format_args!(
    "the renderer read nothing, and the browser was stopped after {WAITING_MESSAGES_LIMIT} messages: the channel holds no more"
  ));
  let Ok(browser_process) = browser.process();

  match ::core::primitive::bool::then_some(
    kernel.create_memory(browser_process, BROWSER_SHARE) == Err(HandleError::OverBudget),
    (),
  ) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(BAD_STATE),
  }

  log(format_args!(
    "the browser asked for as much memory as its whole job was given, and was refused",
  ));

  Ok(())
}

fn check_site_closed(
  kernel: &mut Kernel,
  browser: &Browser,
  replacement: ProcessId,
  before: console_kernel_handles::Budget,
  objects: u64,
) -> Result<(), SiteIsolationError> {
  match ::core::primitive::bool::then_some(
    kernel.handles(replacement) == Err(HandleError::ProcessNotFound),
    (),
  ) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(BAD_STATE),
  }

  let budget = browser.budget(kernel)?;
  let after = kernel.object_count()?;

  match ::core::primitive::bool::then_some(budget == before && after == objects, ()) {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => Err(BAD_STATE),
  }
}

fn exchange_frame(
  kernel: &mut Kernel,
  browser: &Browser,
  site: &Site,
  renderer: &Renderer,
  page: &mut Page,
  present: &mut impl FnMut(Size<u32>, &[[u8; 4]]),
) -> Result<(), SiteIsolationError> {
  let browser_before = browser.budget(kernel)?;
  let site_before = browser.site_budget(kernel, site)?;
  let objects = kernel.object_count()?;
  let Ok(requests) = page.opening_requests();

  for request in requests {
    let Ok(line) = encoded_request(&request);
    browser.send(kernel, site, line.as_bytes())?;
  }

  let resize = renderer.request(kernel)?;
  let size = match resize {
    Some(Request::Resize(size)) => size,
    Some(_) | None => return Err(BAD_STATE),
  };
  let open = renderer.request(kernel)?;
  let address = match open {
    Some(Request::Open(address)) => address,
    Some(_) | None => return Err(BAD_STATE),
  };

  renderer.draw(kernel, size, &address)?;

  let Ok(process) = browser.process();
  let address_reply = browser.receive(kernel, site)?;
  let line = control_line(kernel, process, &address_reply)?;

  let Ok(event) = decoded_event(line);
  page.address = match event {
    Some(Event::Address(current_address)) => current_address,
    Some(_) | None => return Err(BAD_STATE),
  };

  let message = browser.receive(kernel, site)?;
  present_frame(kernel, process, message, page.viewport, present)?;

  let browser_after = browser.budget(kernel)?;
  let site_after = browser.site_budget(kernel, site)?;
  let remaining_objects = kernel.object_count()?;

  match ::core::primitive::bool::then_some(
    browser_after == browser_before && site_after == site_before && remaining_objects == objects,
    (),
  ) {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => Err(BAD_STATE),
  }
}

impl Renderer {
  fn bootstrap(kernel: &mut Kernel, process: ProcessId) -> Result<Self, HandleError> {
    let handles = kernel.handles(process)?;

    match handles.as_slice() {
      [channel] => Ok(Self {
        process,
        channel: *channel,
      }),
      _ => Err(HandleError::BadState),
    }
  }

  fn request(&self, kernel: &mut Kernel) -> Result<Option<Request>, HandleError> {
    let message = kernel.read(self.process, self.channel)?;
    let line = control_line(kernel, self.process, &message)?;
    let Ok(request) = decoded_request(line);

    Ok(request)
  }

  fn draw(&self, kernel: &mut Kernel, size: Size<u32>, address: &str) -> Result<(), HandleError> {
    let body = page::decode(address)?;
    let text = str::from_utf8(&body).map_err(|_| HandleError::BadState)?;
    let bytes = page::frame_bytes(size)?;
    let frame = kernel.create_memory(self.process, bytes)?;
    let painted = kernel
      .write_memory(self.process, frame)
      .and_then(|pixels| page::paint(size, text, pixels.as_chunks_mut().0));
    let sent = match painted {
      Ok(()) => self.send_page(kernel, size, frame, address),
      Err(error) => Err(error),
    };
    kernel.close(self.process, frame)?;

    sent
  }

  #[cfg_attr(
    dylint_lib = "explicit066_no_heap_after_init",
    expect(
      explicit066_no_heap_after_init,
      reason = "the bounded renderer protocol owns the echoed address while serializing the reply"
    )
  )]
  fn send_page(
    &self,
    kernel: &mut Kernel,
    size: Size<u32>,
    frame: HandleValue,
    address: &str,
  ) -> Result<(), HandleError> {
    let Ok(event) = encoded_event(&Event::Address(String::from(address)));
    kernel.write(self.process, self.channel, event.as_bytes(), &[])?;

    self.send_frame(kernel, size, frame)
  }

  fn send_frame(
    &self,
    kernel: &mut Kernel,
    size: Size<u32>,
    frame: HandleValue,
  ) -> Result<(), HandleError> {
    let Ok(readable) = Rights::from_rights(&[Right::Read, Right::Transfer]);
    let outgoing = kernel.duplicate(self.process, frame, readable)?;
    let Ok(event) = encoded_event(&Event::Frame(size));

    match kernel.write(self.process, self.channel, event.as_bytes(), &[outgoing]) {
      Ok(()) => Ok(()),
      Err(fault) => {
        kernel.close(self.process, outgoing)?;

        Err(fault)
      }
    }
  }
}

fn control_line<'message>(
  kernel: &mut Kernel,
  process: ProcessId,
  message: &'message Message,
) -> Result<&'message str, HandleError> {
  match ::core::primitive::bool::then_some(!message.handles.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      for handle in &message.handles {
        kernel.close(process, *handle)?;
      }

      return Err(HandleError::BadState);
    }
    ::core::option::Option::None => {}
  }

  str::from_utf8(&message.bytes).map_err(|_| HandleError::BadState)
}

fn present_frame(
  kernel: &mut Kernel,
  process: ProcessId,
  message: Message,
  expected: Size<u32>,
  present: &mut impl FnMut(Size<u32>, &[[u8; 4]]),
) -> Result<(), HandleError> {
  let presented = match message.handles.as_slice() {
    [frame] => match frame_pixels(kernel, process, *frame, &message.bytes, expected) {
      Ok(pixels) => {
        present(expected, pixels);

        Ok(())
      }
      Err(error) => Err(error),
    },
    _ => Err(HandleError::BadState),
  };

  for handle in message.handles {
    kernel.close(process, handle)?;
  }

  presented
}

fn frame_pixels<'a>(
  kernel: &'a mut Kernel,
  process: ProcessId,
  frame: HandleValue,
  message: &[u8],
  expected: Size<u32>,
) -> Result<&'a [[u8; 4]], HandleError> {
  let line = str::from_utf8(message).map_err(|_| HandleError::BadState)?;
  let Ok(event) = decoded_event(line);
  let Ok(readable) = Rights::from_rights(&[Right::Read, Right::Transfer]);

  match ::core::primitive::bool::then_some(event == Some(Event::Frame(expected)), ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(HandleError::BadState),
  }

  let actual = kernel.handle_rights(process, frame)?;

  match ::core::primitive::bool::then_some(actual == readable, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Err(HandleError::BadState),
  }

  let pixels = kernel.read_memory(process, frame)?;
  FrameLayout::rgba(expected)
    .and_then(|layout| layout.pixels(pixels))
    .map_err(|_invalid_frame| HandleError::BadState)
}

#[cfg(test)]
mod tests {
  use super::*;
  use console_kernel_handles::MessageHandles;

  const ONE_PIXEL: Size<u32> = Size {
    width: 1,
    height: 1,
  };

  #[test]
  fn the_browser_borrows_the_renderers_frame_in_place() -> Result<(), HandleError> {
    let Boot {
      mut kernel,
      root,
      root_job,
      executable_resource: _,
    } = Kernel::boot_with_memory_backing(crate::HOSTED_MEMORY, Bytes(4), 1)?;
    let created = kernel.create_process(root, root_job)?;
    let renderer = Renderer::bootstrap(&mut kernel, created.id)?;
    let before = kernel.budget(root, root_job)?;
    let objects = kernel.object_count()?;
    let frame = kernel.create_memory(renderer.process, Bytes(4))?;
    let pixels = kernel.write_memory(renderer.process, frame)?;
    let rgba = pixels.first_chunk_mut::<4>().ok_or(HandleError::BadState)?;
    *rgba = [10, 20, 30, 255];
    let address = pixels.as_ptr();
    renderer.send_frame(&mut kernel, ONE_PIXEL, frame)?;
    kernel.close(renderer.process, frame)?;

    let message = kernel.read(root, created.channel)?;
    present_frame(&mut kernel, root, message, ONE_PIXEL, &mut |size, shown| {
      assert_eq!(size, ONE_PIXEL);
      assert_eq!(shown, &[[10, 20, 30, 255]]);
      assert_eq!(shown.as_ptr().cast::<u8>(), address);
    })?;
    let after_objects = kernel.object_count()?;
    let after_budget = kernel.budget(root, root_job)?;
    assert_eq!(after_objects, objects);
    assert_eq!(after_budget, before);

    Ok(())
  }

  #[test]
  fn a_frame_with_wrong_metadata_rights_or_length_releases_all_received_handles()
  -> Result<(), HandleError> {
    let Boot {
      mut kernel,
      root,
      root_job,
      executable_resource: _,
    } = Kernel::boot_with_memory_backing(crate::HOSTED_MEMORY, Bytes(8), 2)?;
    let (sending, receiving) = kernel.create_channel(root)?;
    let before = kernel.budget(root, root_job)?;
    let objects = kernel.object_count()?;
    let Ok(readable) = Rights::from_rights(&[Right::Read, Right::Transfer]);
    let Ok(writable) = Rights::from_rights(&[Right::Read, Right::Write, Right::Transfer]);

    for (line, rights, bytes, count) in [
      ("frame 2 1", readable, Bytes(4), 1),
      ("frame 1 1", writable, Bytes(4), 1),
      ("frame 1 1", readable, Bytes(3), 1),
      ("frame 1 1", readable, Bytes(4), 2),
    ] {
      let mut outgoing = MessageHandles::new();

      for _ in 0..count {
        let frame = kernel.create_memory(root, bytes)?;
        let narrowed = kernel.duplicate(root, frame, rights)?;
        outgoing
          .try_push(narrowed)
          .map_err(|_| HandleError::BadState)?;
        kernel.close(root, frame)?;
      }

      kernel.write(root, sending, line.as_bytes(), &outgoing)?;
      let message = kernel.read(root, receiving)?;
      let shown = core::cell::Cell::new(false);
      let result = present_frame(&mut kernel, root, message, ONE_PIXEL, &mut |_, _| {
        shown.set(true);
      });

      assert_eq!(result, Err(HandleError::BadState));
      assert!(!shown.get());
      let after_objects = kernel.object_count()?;
      let after_budget = kernel.budget(root, root_job)?;
      assert_eq!(after_objects, objects);
      assert_eq!(after_budget, before);
    }

    Ok(())
  }
}
