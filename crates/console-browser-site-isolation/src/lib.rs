//! Which site goes in which cage, and what it is lent while it is in there.
//!
//! A browser on Linux is a second operating system, and not out of ambition.
//! The system under it knows users and paths, and a page is neither, so the
//! browser writes its own process model, its own sandbox bolted on after the
//! fork, its own IPC, and its own prompts for the camera, each one a thing
//! the kernel has no word for. Here the kernel has the words. A job is a cage:
//! what is in it may start only what its policy lets it start, may reach only
//! what it was handed, and may have only its share. So the split is that the
//! cage is the system's and the choice of prisoner is the browser's, and this
//! crate is the second half, which is small because it is only a choice: a
//! table from a site to the job its renderer runs in, from a tab to its site,
//! and from a permission to the handle that was lent for it. Every refusal a
//! test here asks for is the kernel's refusal. Nothing in this file checks
//! anything a renderer does, because a check the browser makes is a check a
//! renderer that has taken the browser's word for it can skip.
//!
//! A site is whatever the caller says it is. The web's answer is a scheme and
//! a registrable domain, which needs a URL parsed and the public suffix list
//! read, and both are the web's to compute. What isolation needs of a site is
//! that two of them can be told apart, so a `Site` is bytes that are compared
//! and never read.
//!
//! One site, one job, one renderer. The first tab on a site makes the job
//! under the browser's own, with a share out of the browser's share and a
//! policy that denies starting a job or a process, and starts the renderer in
//! it. A second tab on the same site is drawn by that renderer. A process a
//! tab inside the same job was the other shape and was put down: the job is
//! the cage and both processes would be the same site inside it, so the second
//! one keeps nothing from the first that the first could not already reach,
//! and it is paid for in objects and handles out of a share the site was given
//! in order to be limited to. Every permission would also be lent once a
//! process and taken back once a process, and a withdrawal that misses one of
//! two is the fault this design exists to not have. What a process a tab does
//! buy is that one tab's crash is not the other's, which is a question about
//! restarting and belongs to whoever supervises, not to who is kept from whom.
//!
//! A sandbox is decided once, when the cage is built, because a policy is set
//! on an empty job or not at all. A second tab that names a different sandbox
//! for a site that has one is refused rather than quietly given the first:
//! the caller believed something about the cage that is not true of it.
//!
//! Running written code takes two things and either alone is refused. The
//! site's job must not deny `ReplaceAsExecutable`, which is the sandbox and is
//! for good; and its renderer must hold the executable resource, which is a
//! handle and can be taken back. The browser can pass that resource on only
//! if the system lent it one, so the entitlement is a chain of three choices
//! -- the system's, the browser's, the sandbox's -- and the kernel holds every
//! link. They are two knobs rather than one because they end differently: a
//! site whose just-in-time compiler is withdrawn halfway through its life
//! keeps its job and loses a handle.
//!
//! A permission is a handle, and the browser does not know what to. The
//! system writes a handle down the browser's channel with a name in the bytes
//! beside it -- the microphone, the executable resource -- and `accept` keeps
//! it under that name. `lend` makes a copy of it that stays here, makes a
//! narrowed copy of that, and writes the narrowed one down the site's
//! channel; `withdraw` is `revoke` from the copy that stayed, after which the
//! renderer holds nothing, however often it copied what it was sent and
//! wherever it put the copies. The copy that stays is one a site rather than
//! the browser's own handle because a revoke takes back everything made from
//! the handle it is asked of: lent straight from the one handle, taking the
//! microphone from one site took it from all of them. The executable resource
//! goes the same way with no line of its own, which is the argument that an
//! entitlement and a permission are one thing.
//!
//! Closing the last tab on a site kills its job, and then lets go of what the
//! browser held for it. Everything the renderer made was paid for by its job,
//! and everything the browser spent on it was a handle, an object or a message
//! nobody read, so afterwards the browser's budget and the kernel's count of
//! objects are what they were before the tab was opened. A step that fails
//! halfway through opening one undoes the steps before it for the same reason.
//!
//! A renderer is not trusted to be polite. One that never reads stops the
//! browser's writes at the channel's bound and costs it the bytes of those
//! messages and no more; one that asks for more than its site's share is told
//! `OverBudget` by its own job, and the job beside it never hears of it.
//!
//! `replace_renderer` replaces a site's process and channel while keeping
//! its job and tabs. The replacement starts with its bootstrap channel and
//! needs fresh grants. `receive` reads the site's next message. The caller
//! owns any handles it receives.
//!
//! Two things are the nearest that could be written rather than what was
//! wanted. A renderer cannot ask what kind of object a handle names, so the
//! name of a permission rides in the bytes of the message that carries it. And
//! `open` finds the handle the browser was born with by asking the kernel for
//! every handle that process holds, a question nobody had to hold anything to
//! ask.
//!
//! It is not a `console-core-state-machine`, though it is a state and the
//! requests that change it. That crate offers effects for a runtime to carry
//! out later, and here the effect is the call into a kernel that was handed
//! in, whose refusal is the answer; it also needs `std`, and this links into
//! whatever links the kernel.

#![no_std]

extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use core::fmt;

use console_core_never::Never;
use console_kernel_handles::{
  Budget, Bytes, Condition, CreatedProcess, HandleError, HandleValue, Kernel, Message,
  PolicyAction, ProcessId, Right, Rights,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Site(pub Vec<u8>);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Permission(pub Vec<u8>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Tab(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sandbox {
  pub share: Bytes,
  pub replace_as_executable: PolicyAction,
}

#[derive(Debug, Clone, Copy)]
pub struct Opening<'denied> {
  pub by: ProcessId,
  pub under: HandleValue,
  pub share: Bytes,
  pub denied: &'denied [Condition],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parent {
  pub job: HandleValue,
  pub process: HandleValue,
  pub channel: HandleValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteIsolationError {
  Kernel(HandleError),
  TabNotFound,
  SiteNotFound,
  PermissionNotFound,
  SandboxDiffers,
  AlreadyLent,
  NotLent,
  UnexpectedMessage,
  NoResources,
}

enum SandboxMatch {
  Same,
  Different,
}

impl From<bool> for SandboxMatch {
  fn from(same: bool) -> Self {
    match bool::then_some(same, ()) {
      Some(()) => Self::Same,
      None => Self::Different,
    }
  }
}

enum SiteTabs {
  Empty,
  Open,
}

impl From<bool> for SiteTabs {
  fn from(empty: bool) -> Self {
    match bool::then_some(empty, ()) {
      Some(()) => Self::Empty,
      None => Self::Open,
    }
  }
}

struct Isolated {
  sandbox: Sandbox,
  job: HandleValue,
  renderer: CreatedProcess,
  tabs: BTreeSet<Tab>,
  lent: BTreeMap<Permission, HandleValue>,
}

pub struct Browser {
  process: ProcessId,
  bootstrap: HandleValue,
  job: HandleValue,
  permissions: BTreeMap<Permission, HandleValue>,
  sites: BTreeMap<Site, Isolated>,
  tabs: BTreeMap<Tab, Site>,
  next_tab: u64,
}

impl Sandbox {
  fn denied(self) -> Result<&'static [Condition], Never> {
    Ok(match self.replace_as_executable {
      PolicyAction::Allow => &[Condition::NewJob, Condition::NewProcess],
      PolicyAction::Deny => &[
        Condition::NewJob,
        Condition::NewProcess,
        Condition::ReplaceAsExecutable,
      ],
    })
  }
}

impl Browser {
  pub fn open(
    kernel: &mut Kernel,
    opening: Opening<'_>,
  ) -> Result<(Browser, Parent), SiteIsolationError> {
    let job = kernel.create_job(opening.by, opening.under, opening.share)?;

    match Browser::start(kernel, opening, job) {
      Ok(started) => Ok(started),
      Err(fault) => {
        kernel.kill(opening.by, job)?;
        kernel.close(opening.by, job)?;

        Err(fault)
      }
    }
  }

  fn start(
    kernel: &mut Kernel,
    opening: Opening<'_>,
    job: HandleValue,
  ) -> Result<(Browser, Parent), SiteIsolationError> {
    kernel.set_policy(opening.by, job, opening.denied)?;

    let created = kernel.create_process(opening.by, job)?;
    let parent = Parent {
      job,
      process: created.process,
      channel: created.channel,
    };

    match Browser::born(kernel, opening.by, created.id, parent) {
      Ok(browser) => Ok((browser, parent)),
      Err(fault) => {
        kernel.close(opening.by, parent.process)?;
        kernel.close(opening.by, parent.channel)?;

        Err(fault)
      }
    }
  }

  fn born(
    kernel: &mut Kernel,
    by: ProcessId,
    process: ProcessId,
    parent: Parent,
  ) -> Result<Browser, SiteIsolationError> {
    let held = kernel.handles(process)?;

    let bootstrap = match held.first() {
      Some(bootstrap) => *bootstrap,
      None => return Err(SiteIsolationError::UnexpectedMessage),
    };

    let Ok(managing) = Rights::from_rights(&[Right::ManageJob, Right::Transfer]);
    let lent = kernel.duplicate(by, parent.job, managing)?;

    match kernel.write(by, parent.channel, &[], &[lent]) {
      Ok(()) => {}
      Err(fault) => {
        kernel.close(by, lent)?;

        return Err(SiteIsolationError::Kernel(fault));
      }
    }

    let message = kernel.read(process, bootstrap)?;

    match message.handles.as_slice() {
      [job] => Ok(Browser {
        process,
        bootstrap,
        job: *job,
        permissions: BTreeMap::new(),
        sites: BTreeMap::new(),
        tabs: BTreeMap::new(),
        next_tab: 1,
      }),
      _ => Err(SiteIsolationError::UnexpectedMessage),
    }
  }

  pub fn accept(&mut self, kernel: &mut Kernel) -> Result<Permission, SiteIsolationError> {
    let message = kernel.read(self.process, self.bootstrap)?;

    match message.handles.as_slice() {
      [handle] => {
        let permission = Permission(message.bytes);

        match self.permissions.insert(permission.clone(), *handle) {
          Some(replaced) => {
            kernel.close(self.process, replaced)?;
          }
          None => {}
        }

        Ok(permission)
      }
      _ => {
        for handle in &message.handles {
          kernel.close(self.process, *handle)?;
        }

        Err(SiteIsolationError::UnexpectedMessage)
      }
    }
  }

  pub fn open_tab(
    &mut self,
    kernel: &mut Kernel,
    site: &Site,
    sandbox: Sandbox,
  ) -> Result<Tab, SiteIsolationError> {
    let tab = Tab(self.next_tab);

    let next = match self.next_tab.checked_add(1) {
      Some(next) => next,
      None => return Err(SiteIsolationError::NoResources),
    };

    match self.sites.get_mut(site) {
      Some(isolated) => match SandboxMatch::from(isolated.sandbox == sandbox) {
        SandboxMatch::Same => {
          let _fresh = isolated.tabs.insert(tab);
        }
        SandboxMatch::Different => return Err(SiteIsolationError::SandboxDiffers),
      },
      None => {
        let mut isolated = Browser::isolate(kernel, self.process, self.job, sandbox)?;
        let _fresh = isolated.tabs.insert(tab);
        let _first = self.sites.insert(site.clone(), isolated);
      }
    }

    self.next_tab = next;

    let _fresh = self.tabs.insert(tab, site.clone());

    Ok(tab)
  }

  fn isolate(
    kernel: &mut Kernel,
    by: ProcessId,
    under: HandleValue,
    sandbox: Sandbox,
  ) -> Result<Isolated, SiteIsolationError> {
    let job = kernel.create_job(by, under, sandbox.share)?;
    let Ok(denied) = sandbox.denied();

    let rendering = match kernel.set_policy(by, job, denied) {
      Ok(()) => kernel.create_process(by, job),
      Err(fault) => Err(fault),
    };

    match rendering {
      Ok(renderer) => Ok(Isolated {
        sandbox,
        job,
        renderer,
        tabs: BTreeSet::new(),
        lent: BTreeMap::new(),
      }),
      Err(fault) => {
        kernel.kill(by, job)?;
        kernel.close(by, job)?;

        Err(SiteIsolationError::Kernel(fault))
      }
    }
  }

  pub fn close_tab(&mut self, kernel: &mut Kernel, tab: Tab) -> Result<(), SiteIsolationError> {
    let site = match self.tabs.get(&tab) {
      Some(site) => site.clone(),
      None => return Err(SiteIsolationError::TabNotFound),
    };

    let isolated = match self.sites.get_mut(&site) {
      Some(isolated) => isolated,
      None => return Err(SiteIsolationError::SiteNotFound),
    };

    let _open = isolated.tabs.remove(&tab);

    match SiteTabs::from(isolated.tabs.is_empty()) {
      SiteTabs::Empty => match Browser::end(kernel, self.process, isolated) {
        Ok(()) => {
          let _ended = self.sites.remove(&site);
        }
        Err(fault) => {
          let _still_open = isolated.tabs.insert(tab);

          return Err(fault);
        }
      },
      SiteTabs::Open => {}
    }

    let _closed = self.tabs.remove(&tab);

    Ok(())
  }

  pub fn replace_renderer(
    &mut self,
    kernel: &mut Kernel,
    site: &Site,
  ) -> Result<ProcessId, SiteIsolationError> {
    let isolated = self
      .sites
      .get_mut(site)
      .ok_or(SiteIsolationError::SiteNotFound)?;
    let next = kernel.create_process(self.process, isolated.job)?;

    let retired = match kernel.kill(self.process, isolated.renderer.process) {
      Ok(()) => Browser::release_renderer(kernel, self.process, isolated),
      Err(fault) => Err(fault),
    };

    match retired {
      Ok(()) => {}
      Err(fault) => {
        kernel.kill(self.process, next.process)?;
        Browser::close_renderer_handles(kernel, self.process, next)?;

        return Err(SiteIsolationError::from(fault));
      }
    }

    isolated.renderer = next;
    isolated.lent.clear();

    Ok(next.id)
  }

  fn end(
    kernel: &mut Kernel,
    by: ProcessId,
    isolated: &Isolated,
  ) -> Result<(), SiteIsolationError> {
    kernel.kill(by, isolated.job)?;
    Browser::release_renderer(kernel, by, isolated)?;
    kernel.close(by, isolated.job)?;

    Ok(())
  }

  fn release_renderer(
    kernel: &mut Kernel,
    by: ProcessId,
    isolated: &Isolated,
  ) -> Result<(), HandleError> {
    for kept in isolated.lent.values() {
      Browser::take_back(kernel, by, *kept)?;
    }

    Browser::close_renderer_handles(kernel, by, isolated.renderer)
  }

  fn close_renderer_handles(
    kernel: &mut Kernel,
    by: ProcessId,
    created: CreatedProcess,
  ) -> Result<(), HandleError> {
    kernel.close(by, created.process)?;
    kernel.close(by, created.channel)?;

    Ok(())
  }

  fn take_back(kernel: &mut Kernel, by: ProcessId, kept: HandleValue) -> Result<(), HandleError> {
    match kernel.revoke(by, kept) {
      Ok(()) => kernel.close(by, kept),
      Err(HandleError::BadHandle) => Ok(()),
      Err(fault) => Err(fault),
    }
  }

  pub fn lend(
    &mut self,
    kernel: &mut Kernel,
    site: &Site,
    permission: &Permission,
    narrowed: Rights,
  ) -> Result<(), SiteIsolationError> {
    let held = match self.permissions.get(permission) {
      Some(held) => *held,
      None => return Err(SiteIsolationError::PermissionNotFound),
    };

    let isolated = match self.sites.get_mut(site) {
      Some(isolated) => isolated,
      None => return Err(SiteIsolationError::SiteNotFound),
    };

    match isolated.lent.get(permission) {
      Some(_) => return Err(SiteIsolationError::AlreadyLent),
      None => {}
    }

    let whole = kernel.handle_rights(self.process, held)?;
    let kept = kernel.duplicate(self.process, held, whole)?;

    let passed = match kernel.duplicate(self.process, kept, narrowed) {
      Ok(copy) => kernel.write(
        self.process,
        isolated.renderer.channel,
        &permission.0,
        &[copy],
      ),
      Err(fault) => Err(fault),
    };

    match passed {
      Ok(()) => {
        let _first = isolated.lent.insert(permission.clone(), kept);

        Ok(())
      }
      Err(fault) => {
        kernel.revoke(self.process, kept)?;
        kernel.close(self.process, kept)?;

        Err(SiteIsolationError::Kernel(fault))
      }
    }
  }

  pub fn withdraw(
    &mut self,
    kernel: &mut Kernel,
    site: &Site,
    permission: &Permission,
  ) -> Result<(), SiteIsolationError> {
    let isolated = match self.sites.get_mut(site) {
      Some(isolated) => isolated,
      None => return Err(SiteIsolationError::SiteNotFound),
    };

    let kept = match isolated.lent.get(permission) {
      Some(kept) => *kept,
      None => return Err(SiteIsolationError::NotLent),
    };

    Browser::take_back(kernel, self.process, kept)?;

    let _lent = isolated.lent.remove(permission);

    Ok(())
  }

  pub fn send(
    &self,
    kernel: &mut Kernel,
    site: &Site,
    bytes: &[u8],
  ) -> Result<(), SiteIsolationError> {
    let isolated = self.isolated(site)?;

    kernel.write(self.process, isolated.renderer.channel, bytes, &[])?;

    Ok(())
  }

  pub fn receive(&self, kernel: &mut Kernel, site: &Site) -> Result<Message, SiteIsolationError> {
    let isolated = self.isolated(site)?;
    let message = kernel.read(self.process, isolated.renderer.channel)?;

    Ok(message)
  }

  pub fn process(&self) -> Result<ProcessId, Never> {
    Ok(self.process)
  }

  pub fn renderer(&self, site: &Site) -> Result<ProcessId, SiteIsolationError> {
    let isolated = self.isolated(site)?;

    Ok(isolated.renderer.id)
  }

  pub fn budget(&self, kernel: &mut Kernel) -> Result<Budget, SiteIsolationError> {
    let budget = kernel.budget(self.process, self.job)?;

    Ok(budget)
  }

  pub fn site_budget(
    &self,
    kernel: &mut Kernel,
    site: &Site,
  ) -> Result<Budget, SiteIsolationError> {
    let isolated = self.isolated(site)?;
    let budget = kernel.budget(self.process, isolated.job)?;

    Ok(budget)
  }

  fn isolated(&self, site: &Site) -> Result<&Isolated, SiteIsolationError> {
    match self.sites.get(site) {
      Some(isolated) => Ok(isolated),
      None => Err(SiteIsolationError::SiteNotFound),
    }
  }
}

impl From<HandleError> for SiteIsolationError {
  fn from(fault: HandleError) -> SiteIsolationError {
    SiteIsolationError::Kernel(fault)
  }
}

impl fmt::Display for SiteIsolationError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      SiteIsolationError::Kernel(fault) => write!(to, "the kernel refused: {fault}"),
      SiteIsolationError::TabNotFound => to.write_str("there is no such tab"),
      SiteIsolationError::SiteNotFound => to.write_str("no tab is open on that site"),
      SiteIsolationError::PermissionNotFound => {
        to.write_str("the browser was never lent that permission")
      }
      SiteIsolationError::SandboxDiffers => {
        to.write_str("the site already has a sandbox, and it is not the one asked for")
      }
      SiteIsolationError::AlreadyLent => {
        to.write_str("the site has already been lent that permission")
      }
      SiteIsolationError::NotLent => to.write_str("the site was not lent that permission"),
      SiteIsolationError::UnexpectedMessage => {
        to.write_str("the message did not carry the one handle it was read for")
      }
      SiteIsolationError::NoResources => {
        to.write_str("the browser has run out of numbers for tabs")
      }
    }
  }
}
