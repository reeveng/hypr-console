//! What a process may reach: the handles it holds, and nothing else.
//!
//! This is the first piece of the kernel the backlog calls the horizon, and it
//! is the piece that decides whether the rest is worth writing. On Linux a
//! confined process is born holding everything the parent held and is then
//! stripped -- namespaces, seccomp, landlock, each bolted on after the fork --
//! so confinement is a cost paid per process and turned off first on a
//! handheld. Here a process is born empty. It holds one handle, the end of a
//! channel its parent holds the other end of, and whatever it is ever given
//! arrives down that channel. There is no path, no user id and no table of
//! things everyone can open, so there is nothing to strip, and a cheap process
//! and a confined one are the same process.
//!
//! The shape is Zircon's, because Fuchsia is the existence proof that a
//! browser runs on it: a handle is a process's own number for a kernel object,
//! it carries rights, a right can be dropped on the way to another process and
//! never added, and a handle written into a channel leaves the writer's table
//! in the same step as it enters the message. A write that cannot move every
//! handle it names moves none of them and delivers nothing.
//!
//! The one right that is added rather than dropped is `Execute`, and it is the
//! reason this file exists before a line that boots. A renderer has to write
//! code and then run it, which is the one permission a strict confinement most
//! wants to refuse. So memory is never born executable: a handle is made
//! executable by `replace_as_executable`, which asks for a handle to the executable
//! resource as well, and the root process is the only one born holding that.
//! Whoever it hands a duplicate to is the program allowed to run what it
//! wrote -- iOS's JIT entitlement and Zircon's `vmex` resource, said as a
//! handle. And the handle that comes back can run and cannot write, so no one
//! handle is ever both. Replacing a handle also seals the memory object against
//! writes through any of its other handles, including copies in flight.
//!
//! Every process is inside a job, and a job is inside a job, up to the root
//! the kernel boots with. Killing a job ends every process in it and in every
//! job under it, so a browser that started renderers is ended in one call and
//! leaves nothing running -- the promise `console-program-lifetime` makes with a
//! death signal and a drop on Linux, kept here by the shape of the tree rather
//! than by every caller remembering it. A job also carries a policy, and
//! `policy` says why it can only ever get stricter.
//!
//! A handle can be taken back. Every duplicate remembers the handle it was
//! made from, and `revoke` removes every handle made from one, however many
//! times it was duplicated after and wherever it was sent -- seL4's revoke,
//! because a permission somebody granted is only a permission if it can be
//! withdrawn. The handle revoked from is kept, so whoever granted can grant
//! again. A handle that is closed leaves its copies to the handle it was itself
//! made from, as seL4's derivation tree does. They were forgotten instead at
//! first, so a process lent a handle duplicated it, closed the one it was lent,
//! and held a copy no revoke could find.
//!
//! Taking a handle back costs what is taken back. The kernel keeps, for every
//! handle, where it is: which process's table and at what value, or which
//! end's which unread message and at what place in it, so a message has a
//! number of its own and what it carries is kept by place. Whatever moves a
//! handle says where it went -- made, duplicated, written, read, replaced as
//! executable -- and the one step every handle leaves by forgets it, so a
//! close, a process ending and an end hanging up need not each remember to.
//! Taking back reaches into the one table or message named by each index entry.
//! It records detached handles in those same entries before releasing any
//! references, so releasing a channel cannot destroy another pending copy. It used to walk every table of every
//! process and every unread message of every channel to find them, which made
//! withdrawing one permission cost the size of the kernel, and a renderer
//! lent a microphone many times an hour is the common path rather than the
//! rare one. The handle in a table is still the handle itself, found by its
//! value in one step as before: a table that held a number to look up
//! somewhere else would have made taking back one write and every use of
//! every handle one step longer, and an age on the object checked at every use
//! would have done the same and never let the number be used again. The index
//! is consulted by `revoke` and by nothing else, and what it counts on the way
//! is the steps it took, so a test can hold the number still while the kernel
//! around it grows. An index that names a place the handle is not in is a
//! fault said as `BadState`, never a copy quietly left behind.
//!
//! An object lasts as long as something names it. Memory, a channel end, the
//! executable resource and a process's own object go when the last handle to
//! them does, and a job goes when nothing holds it and nothing is in it, which
//! takes the job above with it when that was all it was waiting for. They used
//! to stay in the table for good, so making memory and closing it in a loop
//! was a way to spend the kernel without holding anything.
//!
//! A channel holds only so much. A message is at most `MESSAGE_BYTES_LIMIT`
//! bytes and `MESSAGE_HANDLES_LIMIT` handles, which are Zircon's numbers, and
//! an end holds at most `WAITING_MESSAGES_LIMIT` messages nobody has read. A
//! write past either is refused whole, with every handle it named still in the
//! writer's table, and the writer is told which it was: `ChannelFull` passes
//! when the reader reads, `MessageTooLarge` never does. Without it the queue
//! was the one thing in the kernel a process could grow by itself, so a
//! renderer that never read was a renderer that could spend the kernel's
//! memory through whoever kept writing to it -- the unbounded mailbox, which is
//! the failure BEAM is known for in production.
//!
//! Everything is paid for by a job. The root job's share is the machine's
//! memory, and a job is made with a share taken out of its parent's, so the
//! tree of jobs is the tree of accounts and a charge is one addition against
//! one job rather than a walk to the root. An object, a handle, the bytes of
//! memory and the bytes of a message nobody has read each cost their job
//! something, and a job that cannot pay is told `OverBudget` before anything
//! is made. A message is its writer's until it is read: the read gives the
//! writer its bytes back and moves the handles onto the reader's account in
//! one step, so at no moment is a message charged to nobody or to both. A job
//! is kept for as long as it still owes for something, which is what makes it
//! the one accountable owner of what its processes left behind. Holding a
//! handle was the only question the kernel asked before this, and a process
//! that may address a thing is not thereby a process that may have any amount
//! of it.
//!
//! Three things the accounting let through were found by attacking it, and
//! `tests/the_attacks.rs` is the attack. A message costs `MESSAGE_COST` whatever
//! it carries, because an empty one cost nothing, its job was reaped under it,
//! and the read then found nobody to give the bytes back to. A channel end is
//! refused the queue it would come to rest in -- its own, or one already on its
//! way into it -- because an end named only by a message in its own queue has no
//! last holder, and a confined process could leave one behind and keep its
//! job's share charged to the job above for good. An end has no `Duplicate`,
//! so it is in one place, and the refusal is a walk up the queues that place is
//! inside rather than a search for cycles. And a job nobody holds and nothing
//! runs in keeps only what it still owes: the rest of its share goes back the
//! moment it is alone, where one unread byte used to hold all of it.
//!
//! Nothing here touches a machine. It is arithmetic over tables, `no_std` so
//! the kernel links it as it is, and tested on the host like any other crate.

#![no_std]

extern crate alloc;

mod policy;
mod rights;

#[cfg(kani)]
#[path = "../proofs/authority.rs"]
mod authority_proofs;

mod accounting;
mod backing;
mod message_handles;

#[cfg(test)]
mod tests;

pub use message_handles::MessageHandles;
pub use policy::{Condition, Policy, PolicyAction};
pub use rights::{Contains, Right, Rights};

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use console_core_never::Never;
use console_core_number_conversion::fixed_index;
use core::fmt;
use core::marker::PhantomData;

use accounting::{
  DeliveryCharges, DeliveryCost, DeliveryJobs, MessageSize, charge_within, delivery_charges,
  message_cost, waiting_room,
};
use backing::{Backing, BackingLimits, Span};

pub const MESSAGE_BYTES_LIMIT: u32 = 65_536;

pub const MESSAGE_HANDLES_LIMIT: u32 = 64;

pub const WAITING_MESSAGES_LIMIT: u32 = 64;

pub const OBJECT_COST: Bytes = Bytes(256);

pub const HANDLE_COST: Bytes = Bytes(32);

pub const MESSAGE_COST: Bytes = Bytes(64);

const TABLE_STEPS: u64 = 5;

const MESSAGE_STEPS: u64 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Bytes(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
  pub share: Bytes,
  pub charged: Bytes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HandleValue(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ObjectId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct HandleId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct MessageId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Slot(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Steps(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Location {
  Table {
    process: ProcessId,
    value: HandleValue,
  },
  Message {
    endpoint: ObjectId,
    message: MessageId,
    slot: Slot,
  },
  Detached {
    object: ObjectId,
    rights: Rights,
    paid_by: ObjectId,
  },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
  Channel,
  MemoryObject,
  PhysicalFrame,
  ExecutableResource,
  TaskObj,
  Process,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleError {
  ProcessNotFound,
  BadHandle,
  WrongType(ObjectType),
  AccessDenied(Right),
  DeniedByPolicy(Condition),
  InvalidRights,
  SelfTransfer,
  RepeatedHandle,
  PeerClosed,
  ShouldWait,
  ChannelFull,
  MessageTooLarge,
  OverBudget,
  BadState,
  NoResources,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Message {
  pub bytes: Vec<u8>,
  pub handles: MessageHandles,
}

pub struct Boot {
  pub kernel: Kernel,
  pub root: ProcessId,
  pub root_job: HandleValue,
  pub executable_resource: HandleValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreatedProcess {
  pub id: ProcessId,
  pub process: HandleValue,
  pub channel: HandleValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingAccess {
  Read,
  Write,
}

pub struct FrameMapping<'kernel> {
  address: u64,
  access: MappingAccess,
  _kernel: PhantomData<&'kernel mut Kernel>,
}

impl<'kernel> FrameMapping<'kernel> {
  fn authorize(
    object: &'kernel mut Object,
    held: Rights,
    access: MappingAccess,
  ) -> Result<Self, HandleError> {
    authorize_frame_access(held, access)?;

    match object {
      Object::PhysicalFrame(address) => Ok(Self {
        address: *address,
        access,
        _kernel: PhantomData,
      }),
      Object::Endpoint(_)
      | Object::Memory(_)
      | Object::ExecutableResource
      | Object::TaskObj(_)
      | Object::Process(_) => Err(HandleError::WrongType(ObjectType::PhysicalFrame)),
    }
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "this accessor preserves the native paging callers' public mapping-token interface"
    )
  )]
  pub fn address(&self) -> u64 {
    self.address
  }

  #[must_use]
  #[cfg_attr(
    dylint_lib = "explicit002_infallible_result",
    expect(
      explicit002_infallible_result,
      reason = "this accessor preserves the native paging callers' public mapping-token interface"
    )
  )]
  pub fn access(&self) -> MappingAccess {
    self.access
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Handle {
  id: HandleId,
  object: ObjectId,
  rights: Rights,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Charged {
  handle: Handle,
  paid_by: ObjectId,
}

struct Reclaimed {
  held: Charged,
  steps: u64,
}

type Carried = BTreeMap<Slot, Handle>;

struct QueuedMessage {
  bytes: Vec<u8>,
  handles: Carried,
  paid_by: ObjectId,
  cost: u64,
}

struct Endpoint {
  peer: Option<ObjectId>,
  waiting: BTreeMap<MessageId, QueuedMessage>,
  travelling_in: Option<ObjectId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JobState {
  Running,
  Killed,
}

struct Task {
  parent: Option<ObjectId>,
  jobs: BTreeSet<ObjectId>,
  processes: BTreeSet<ProcessId>,
  policy: Policy,
  state: JobState,
  share: u64,
  charged: u64,
}

impl Task {
  fn kill(&mut self) -> Result<(), Never> {
    self.state = JobState::Killed;
    Ok(())
  }

  fn budget(&self) -> Result<Budget, Never> {
    Ok(Budget {
      share: Bytes(self.share),
      charged: Bytes(self.charged),
    })
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MemoryAccess {
  Writable,
  Sealed,
}

impl MemoryAccess {
  fn seal(&mut self) -> Result<(), Never> {
    *self = Self::Sealed;

    Ok(())
  }

  fn permits(self, right: Right) -> Result<(), HandleError> {
    match (self, right) {
      (Self::Sealed, Right::Write) => Err(HandleError::AccessDenied(Right::Write)),
      _ => Ok(()),
    }
  }

  fn authorize_executable(self, rights: Rights) -> Result<(), HandleError> {
    let Ok(write) = rights.contains(Right::Write);
    let Ok(execute) = rights.contains(Right::Execute);

    match (self, write, execute) {
      (_, Contains::Yes, _) | (Self::Sealed, _, Contains::Yes) => Ok(()),
      _ => Err(HandleError::AccessDenied(Right::Write)),
    }
  }
}

struct Memory {
  span: Span,
  access: MemoryAccess,
}

enum Object {
  Endpoint(Endpoint),
  Memory(Memory),
  PhysicalFrame(u64),
  ExecutableResource,
  TaskObj(Task),
  Process(ProcessId),
}

struct Entry {
  object: Object,
  references: u64,
  paid_by: ObjectId,
  cost: u64,
}

struct Process {
  handles: BTreeMap<HandleValue, Handle>,
  next_handle: u64,
  job: ObjectId,
}

pub struct Kernel {
  backing: Backing,
  processes: BTreeMap<ProcessId, Process>,
  objects: BTreeMap<ObjectId, Entry>,
  derived: BTreeMap<HandleId, BTreeSet<HandleId>>,
  derived_from: BTreeMap<HandleId, HandleId>,
  located: BTreeMap<HandleId, Location>,
  next_process: u64,
  next_object: u64,
  next_handle: u64,
  next_message: u64,
}

fn next_id(from: u64) -> Result<u64, HandleError> {
  match from.checked_add(1) {
    Some(next) => Ok(next),
    None => Err(HandleError::NoResources),
  }
}

fn total(costs: &[Bytes]) -> Result<u64, HandleError> {
  let added = costs
    .iter()
    .try_fold(0_u64, |sum, cost| sum.checked_add(cost.0));

  match added {
    Some(added_2) => Ok(added_2),
    None => Err(HandleError::OverBudget),
  }
}

fn handles_cost(handles: &Carried) -> Result<u64, HandleError> {
  let counted = match u64::try_from(handles.len()) {
    Ok(counted) => counted,
    Err(_more_than_the_kernel_numbers) => return Err(HandleError::NoResources),
  };

  match counted.checked_mul(HANDLE_COST.0) {
    Some(cost) => Ok(cost),
    None => Err(HandleError::OverBudget),
  }
}

fn require(handle: Handle, right: Right) -> Result<(), HandleError> {
  let Ok(held) = handle.rights.contains(right);

  match held {
    Contains::Yes => Ok(()),
    Contains::No => Err(HandleError::AccessDenied(right)),
  }
}

fn authorize_duplicate(source: Rights, narrowed: Rights) -> Result<(), HandleError> {
  let Ok(duplicate) = source.contains(Right::Duplicate);

  match duplicate {
    Contains::No => return Err(HandleError::AccessDenied(Right::Duplicate)),
    Contains::Yes => {}
  }

  let Ok(within) = narrowed.subset_of(source);

  match within {
    Contains::Yes => Ok(()),
    Contains::No => Err(HandleError::InvalidRights),
  }
}

fn executable_rights(source: Rights) -> Result<Rights, Never> {
  let Ok(unwritable) = source.difference(Right::Write);

  unwritable.union(Right::Execute)
}

fn authorize_frame_access(held: Rights, requested: MappingAccess) -> Result<(), HandleError> {
  let Ok(read) = held.contains(Right::Read);
  let Ok(write) = held.contains(Right::Write);

  match (read, requested, write) {
    (Contains::No, _, _) => Err(HandleError::AccessDenied(Right::Read)),
    (Contains::Yes, MappingAccess::Write, Contains::No) => {
      Err(HandleError::AccessDenied(Right::Write))
    }
    (Contains::Yes, MappingAccess::Read, _)
    | (Contains::Yes, MappingAccess::Write, Contains::Yes) => Ok(()),
  }
}

fn check_revocation_target(expected: HandleId, located: HandleId) -> Result<(), HandleError> {
  match ::core::primitive::bool::then_some(expected == located, ()) {
    ::core::option::Option::Some(()) => Ok(()),
    ::core::option::Option::None => Err(HandleError::BadState),
  }
}

fn rights(granted: &[Right]) -> Result<Rights, Never> {
  Rights::from_rights(granted)
}

fn empty_job(parent: Option<ObjectId>, policy: Policy, share: Bytes) -> Result<Object, Never> {
  Ok(Object::TaskObj(Task {
    parent,
    jobs: BTreeSet::new(),
    processes: BTreeSet::new(),
    policy,
    state: JobState::Running,
    share: share.0,
    charged: 0,
  }))
}

impl Process {
  fn empty(job: ObjectId) -> Result<Process, Never> {
    Ok(Process {
      handles: BTreeMap::new(),
      next_handle: 1,
      job,
    })
  }

  fn insert(&mut self, handle: Handle) -> Result<HandleValue, HandleError> {
    let value = HandleValue(self.next_handle);
    let next = next_id(self.next_handle)?;

    self.next_handle = next;

    let _fresh = self.handles.insert(value, handle);

    Ok(value)
  }

  fn get(&self, at: HandleValue) -> Result<Handle, HandleError> {
    match self.handles.get(&at) {
      Some(handle) => Ok(*handle),
      None => Err(HandleError::BadHandle),
    }
  }

  fn remove(&mut self, at: HandleValue) -> Result<Handle, HandleError> {
    match self.handles.remove(&at) {
      Some(handle) => Ok(handle),
      None => Err(HandleError::BadHandle),
    }
  }
}

impl Kernel {
  pub fn boot(memory: Bytes) -> Result<Boot, HandleError> {
    let least = OBJECT_COST.0.saturating_add(HANDLE_COST.0);
    let objects = memory
      .0
      .checked_div(least)
      .ok_or(HandleError::NoResources)?;

    Self::boot_reserved(
      memory,
      BackingLimits {
        bytes: memory,
        objects,
      },
    )
  }

  #[doc = "Boots with an explicit reserve for the contents of nonempty memory objects."]
  #[doc = "Physical frames have their own backing and use no reserve."]
  #[cfg_attr(
    dylint_lib = "explicit051_no_machine_width",
    expect(
      explicit051_no_machine_width,
      reason = "the existing public boot interface takes a native allocation count; internal sizes are fixed-width"
    )
  )]
  pub fn boot_with_memory_backing(
    memory: Bytes,
    byte_limit: Bytes,
    object_limit: usize,
  ) -> Result<Boot, HandleError> {
    let Ok(objects) = fixed_index(object_limit);
    let bytes = Bytes(byte_limit.0.min(memory.0));

    Self::boot_reserved(memory, BackingLimits { bytes, objects })
  }

  fn boot_reserved(memory: Bytes, reserve: BackingLimits) -> Result<Boot, HandleError> {
    let backing = Backing::reserve_at_boot(reserve)?;
    let mut kernel = Kernel {
      backing,
      processes: BTreeMap::new(),
      objects: BTreeMap::new(),
      derived: BTreeMap::new(),
      derived_from: BTreeMap::new(),
      located: BTreeMap::new(),
      next_process: 1,
      next_object: 2,
      next_handle: 1,
      next_message: 1,
    };
    let job = ObjectId(1);
    let Ok(empty) = empty_job(None, Policy::ALLOW_ALL, memory);
    let _fresh = kernel.objects.insert(
      job,
      Entry {
        object: empty,
        references: 0,
        paid_by: job,
        cost: OBJECT_COST.0,
      },
    );

    kernel.charge(job, OBJECT_COST.0)?;

    let root = kernel.new_process(job)?;
    let resource = kernel.new_object(Object::ExecutableResource, job, OBJECT_COST.0)?;
    let Ok(lendable) = rights(&[Right::Duplicate, Right::Transfer]);
    let executable_resource = kernel.install(root, resource, lendable)?;
    let Ok(managing) = rights(&[
      Right::ManageJob,
      Right::Destroy,
      Right::Duplicate,
      Right::Transfer,
    ]);
    let root_job = kernel.install(root, job, managing)?;

    Ok(Boot {
      kernel,
      root,
      root_job,
      executable_resource,
    })
  }

  pub fn create_job(
    &mut self,
    by: ProcessId,
    parent: HandleValue,
    share: Bytes,
  ) -> Result<HandleValue, HandleError> {
    self.allowed(by, Condition::NewJob)?;

    let above = self.running_job(by, parent)?;
    let asking = self.process(by)?;
    let paying = asking.job;
    let over = self.job(above)?;
    let policy = over.policy;
    let made = total(&[OBJECT_COST, HANDLE_COST])?;

    self.charge(above, share.0)?;

    match self.afford(paying, made) {
      Ok(()) => {}
      Err(fault) => {
        self.uncharge(above, share.0)?;

        return Err(fault);
      }
    }

    let Ok(empty) = empty_job(Some(above), policy, share);
    let child = self.new_object(empty, paying, OBJECT_COST.0)?;
    let held = self.job(above)?;
    let _fresh = held.jobs.insert(child);
    let Ok(managing) = rights(&[
      Right::ManageJob,
      Right::Destroy,
      Right::Duplicate,
      Right::Transfer,
    ]);

    self.install(by, child, managing)
  }

  pub fn set_policy(
    &mut self,
    by: ProcessId,
    job: HandleValue,
    denied: &[Condition],
  ) -> Result<(), HandleError> {
    let named = self.running_job(by, job)?;
    let held = self.job(named)?;

    match (held.jobs.is_empty(), held.processes.is_empty()) {
      (true, true) => {}
      (false, _) | (_, false) => return Err(HandleError::BadState),
    }

    let Ok(policy) = held.policy.deny(denied);

    held.policy = policy;

    Ok(())
  }

  pub fn budget(&mut self, by: ProcessId, job: HandleValue) -> Result<Budget, HandleError> {
    let holder = self.process(by)?;
    let handle = holder.get(job)?;
    let held = self.job(handle.object)?;

    let Ok(budget) = held.budget();

    Ok(budget)
  }

  pub fn create_process(
    &mut self,
    by: ProcessId,
    job: HandleValue,
  ) -> Result<CreatedProcess, HandleError> {
    self.allowed(by, Condition::NewProcess)?;
    self.allowed(by, Condition::NewChannel)?;

    let into = self.running_job(by, job)?;
    let asking = self.process(by)?;
    let paying = asking.job;
    let made = total(&[
      OBJECT_COST,
      OBJECT_COST,
      OBJECT_COST,
      HANDLE_COST,
      HANDLE_COST,
    ])?;

    match ::core::primitive::bool::then_some(paying == into, ()) {
      ::core::option::Option::Some(()) => {
        let both = total(&[Bytes(made), HANDLE_COST])?;

        self.afford(paying, both)?;
      }
      ::core::option::Option::None => {
        self.afford(paying, made)?;
        self.afford(into, HANDLE_COST.0)?;
      }
    }

    let (near, far) = self.joined_endpoints(paying)?;
    let id = self.new_process(into)?;
    let Ok(speaking) = rights(&[Right::Read, Right::Write, Right::Transfer]);
    let channel = self.install(by, near, speaking)?;
    let _bootstrap = self.install(id, far, speaking)?;
    let object = self.new_object(Object::Process(id), paying, OBJECT_COST.0)?;
    let Ok(ending) = rights(&[Right::Destroy, Right::Duplicate, Right::Transfer]);
    let process = self.install(by, object, ending)?;

    Ok(CreatedProcess {
      id,
      process,
      channel,
    })
  }

  pub fn create_channel(
    &mut self,
    by: ProcessId,
  ) -> Result<(HandleValue, HandleValue), HandleError> {
    self.allowed(by, Condition::NewChannel)?;

    let asking = self.process(by)?;
    let paying = asking.job;
    let made = total(&[OBJECT_COST, OBJECT_COST, HANDLE_COST, HANDLE_COST])?;

    self.afford(paying, made)?;

    let (one, other) = self.joined_endpoints(paying)?;
    let Ok(speaking) = rights(&[Right::Read, Right::Write, Right::Transfer]);
    let first = self.install(by, one, speaking)?;
    let second = self.install(by, other, speaking)?;

    Ok((first, second))
  }

  pub fn write(
    &mut self,
    by: ProcessId,
    through: HandleValue,
    bytes: &[u8],
    moving: &[HandleValue],
  ) -> Result<(), HandleError> {
    let writer = self.process(by)?;
    let paid_by = writer.job;
    let handle = writer.get(through)?;

    require(handle, Right::Write)?;
    let Ok(length) = fixed_index(bytes.len());
    let Ok(count) = fixed_index(moving.len());
    let cost = message_cost(MessageSize {
      bytes: length,
      handles: count,
    })?;
    check_transfer(writer, through, moving)?;

    let peer = self.peer_with_room(handle.object)?;

    self.check_rests_elsewhere(by, moving, peer)?;

    let message = self.new_message()?;

    self.charge(paid_by, cost)?;

    let handles = self.lifted(by, moving)?;

    for (slot, moved) in &handles {
      let _was_held = self.located.insert(
        moved.id,
        Location::Message {
          endpoint: peer,
          message,
          slot: *slot,
        },
      );
      let Ok(()) = self.travel(moved.object, Some(peer));
    }

    let receiving = self.endpoint(peer)?;
    let _fresh = receiving.waiting.insert(
      message,
      QueuedMessage {
        bytes: bytes.to_vec(),
        handles,
        paid_by,
        cost,
      },
    );

    Ok(())
  }

  pub fn read(&mut self, by: ProcessId, through: HandleValue) -> Result<Message, HandleError> {
    let reader = self.process(by)?;
    let receiving = reader.job;
    let handle = reader.get(through)?;

    require(handle, Right::Read)?;

    let endpoint = self.endpoint(handle.object)?;

    let (paid_by, cost, carried, handle_count) =
      match (endpoint.waiting.first_key_value(), endpoint.peer) {
        (Some((_, arrived)), _) => (
          arrived.paid_by,
          arrived.cost,
          handles_cost(&arrived.handles),
          u64::try_from(arrived.handles.len()),
        ),
        (None, Some(_)) => return Err(HandleError::ShouldWait),
        (None, None) => return Err(HandleError::PeerClosed),
      };

    let sender_job = self.job(paid_by)?;
    let Ok(sender) = sender_job.budget();
    let jobs = match bool::then_some(paid_by == receiving, ()) {
      Some(()) => DeliveryJobs::SameJob,
      None => {
        let receiver_job = self.job(receiving)?;
        let Ok(receiver) = receiver_job.budget();

        DeliveryJobs::DifferentJobs(receiver)
      }
    };
    let charge = carried?;
    let charges = delivery_charges(
      sender,
      jobs,
      DeliveryCost {
        message: cost,
        handles: charge,
      },
    )?;
    let count = handle_count.map_err(|_too_many| HandleError::NoResources)?;
    let receiver = self.process(by)?;
    let _next_handle = receiver
      .next_handle
      .checked_add(count)
      .ok_or(HandleError::NoResources)?;

    match charges {
      DeliveryCharges::SameJob(charged) => {
        let paying = self.job(paid_by)?;

        paying.charged = charged;
      }
      DeliveryCharges::DifferentJobs {
        sending,
        receiving: received,
      } => {
        let paying = self.job(paid_by)?;
        paying.charged = sending;

        let accepting = self.job(receiving)?;
        accepting.charged = received;
      }
    }

    let emptying = self.endpoint(handle.object)?;

    let arrived = match emptying.waiting.pop_first() {
      Some((_, arrived)) => arrived,
      None => return Err(HandleError::BadState),
    };

    let mut handles = MessageHandles::new();

    for moved in arrived.handles.into_values() {
      let value = self.hold(by, moved)?;

      handles.try_push(value).map_err(|_| HandleError::BadState)?;
    }

    self.reap(paid_by)?;

    Ok(Message {
      bytes: arrived.bytes,
      handles,
    })
  }

  pub fn duplicate(
    &mut self,
    by: ProcessId,
    of: HandleValue,
    narrowed: Rights,
  ) -> Result<HandleValue, HandleError> {
    let holder = self.process(by)?;
    let handle = holder.get(of)?;

    authorize_duplicate(handle.rights, narrowed)?;

    let copy = self.install(by, handle.object, narrowed)?;
    let copier = self.process(by)?;
    let made = copier.get(copy)?;

    let _fresh = self.derived.entry(handle.id).or_default().insert(made.id);
    let _first = self.derived_from.insert(made.id, handle.id);

    Ok(copy)
  }

  pub fn revoke(&mut self, by: ProcessId, from: HandleValue) -> Result<(), HandleError> {
    let _counted = self.revoked(by, from)?;

    Ok(())
  }

  pub fn create_memory(
    &mut self,
    by: ProcessId,
    size: Bytes,
  ) -> Result<HandleValue, HandleError> {
    self.allowed(by, Condition::NewMemoryObject)?;

    let asking = self.process(by)?;
    let paying = asking.job;
    let _next_local = next_id(asking.next_handle)?;
    let _next_handle = next_id(self.next_handle)?;
    let _next_object = next_id(self.next_object)?;
    let cost = total(&[OBJECT_COST, size])?;
    let made = total(&[Bytes(cost), HANDLE_COST])?;

    self.afford(paying, made)?;

    let span = self.backing.claim(size)?;
    let memory = Memory {
      span,
      access: MemoryAccess::Writable,
    };
    let object = self.new_object(Object::Memory(memory), paying, cost)?;
    let Ok(owning) = rights(&[Right::Read, Right::Write, Right::Duplicate, Right::Transfer]);

    self.install(by, object, owning)
  }

  #[doc = "Borrows the bytes of a memory object for the lifetime of the kernel borrow."]
  pub fn read_memory(&mut self, by: ProcessId, at: HandleValue) -> Result<&[u8], HandleError> {
    let contents = self.memory_contents(by, at, Right::Read)?;

    Ok(contents)
  }

  #[doc = "Registers one boot-reserved physical data page as a handle object."]
  #[doc = ""]
  #[doc = "# Safety"]
  #[doc = "The caller must own the whole page until every derived handle and CPU or device"]
  #[doc = "mapping is gone. The page must be initialized before registration and must have"]
  #[doc = "no other live registration or executable alias."]
  pub unsafe fn register_frame(
    &mut self,
    by: ProcessId,
    address: u64,
  ) -> Result<HandleValue, HandleError> {
    self.allowed(by, Condition::NewMemoryObject)?;

    match ::core::primitive::bool::then_some(
      address != 0 && address & 0x000f_ffff_ffff_f000 == address,
      (),
    ) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(HandleError::BadState),
    }

    let owner = self.process(by)?;
    let paying = owner.job;
    let _next_local = next_id(owner.next_handle)?;
    let _next_handle = next_id(self.next_handle)?;
    let _next_object = next_id(self.next_object)?;
    let cost = total(&[OBJECT_COST, Bytes(4096)])?;
    let made = total(&[Bytes(cost), HANDLE_COST])?;

    self.afford(paying, made)?;

    let object = self.new_object(Object::PhysicalFrame(address), paying, cost)?;
    let Ok(owning) = rights(&[Right::Read, Right::Write, Right::Duplicate, Right::Transfer]);

    self.install(by, object, owning)
  }

  pub fn authorize_mapping(
    &mut self,
    by: ProcessId,
    at: HandleValue,
    access: MappingAccess,
  ) -> Result<FrameMapping<'_>, HandleError> {
    let holder = self.process(by)?;
    let handle = holder.get(at)?;
    let entry = self.entry(handle.object)?;
    FrameMapping::authorize(&mut entry.object, handle.rights, access)
  }

  #[doc = "Borrows the bytes of a memory object exclusively through a writable handle."]
  pub fn write_memory(
    &mut self,
    by: ProcessId,
    at: HandleValue,
  ) -> Result<&mut [u8], HandleError> {
    self.memory_contents(by, at, Right::Write)
  }

  pub fn replace_as_executable(
    &mut self,
    by: ProcessId,
    memory: HandleValue,
    resource: HandleValue,
  ) -> Result<HandleValue, HandleError> {
    self.allowed(by, Condition::ReplaceAsExecutable)?;

    let holder = self.process(by)?;
    let granting = holder.get(resource)?;
    let handle = holder.get(memory)?;
    let next = holder.next_handle;

    self.require_type(granting.object, ObjectType::ExecutableResource)?;
    let previous = self.memory_object(handle.object)?;

    previous.access.authorize_executable(handle.rights)?;
    let _next = next_id(next)?;

    let Ok(executable) = executable_rights(handle.rights);
    let replacing = self.process(by)?;

    replacing.remove(memory)?;

    let replacement = self.hold(
      by,
      Handle {
        id: handle.id,
        object: handle.object,
        rights: executable,
      },
    )?;
    let sealed = self.memory_object(handle.object)?;
    let Ok(()) = sealed.access.seal();

    Ok(replacement)
  }

  pub fn handle_rights(&mut self, by: ProcessId, of: HandleValue) -> Result<Rights, HandleError> {
    let holder = self.process(by)?;
    let handle = holder.get(of)?;

    Ok(handle.rights)
  }

  pub fn object_count(&self) -> Result<u64, HandleError> {
    match u64::try_from(self.objects.len()) {
      Ok(count) => Ok(count),
      Err(_more_than_the_kernel_numbers) => Err(HandleError::NoResources),
    }
  }

  pub fn handles(&mut self, by: ProcessId) -> Result<Vec<HandleValue>, HandleError> {
    let holder = self.process(by)?;

    Ok(holder.handles.keys().copied().collect())
  }

  pub fn close(&mut self, by: ProcessId, at: HandleValue) -> Result<(), HandleError> {
    let holder = self.process(by)?;
    let paid_by = holder.job;
    let handle = holder.remove(at)?;

    self.release(Charged { handle, paid_by })
  }

  pub fn kill(&mut self, by: ProcessId, task: HandleValue) -> Result<(), HandleError> {
    let holder = self.process(by)?;
    let handle = holder.get(task)?;

    require(handle, Right::Destroy)?;

    let entry = self.entry(handle.object)?;

    match entry.object {
      Object::Process(process) => self.end(process),
      Object::TaskObj(_) => self.kill_job(handle.object),
      Object::Endpoint(_)
      | Object::Memory(_)
      | Object::PhysicalFrame(_)
      | Object::ExecutableResource => Err(HandleError::WrongType(ObjectType::Process)),
    }
  }

  pub fn exit(&mut self, process: ProcessId) -> Result<(), HandleError> {
    self.end(process)
  }

  fn joined_endpoints(&mut self, paid_by: ObjectId) -> Result<(ObjectId, ObjectId), HandleError> {
    let one = self.new_object(
      Object::Endpoint(Endpoint {
        peer: None,
        waiting: BTreeMap::new(),
        travelling_in: None,
      }),
      paid_by,
      OBJECT_COST.0,
    )?;
    let other = self.new_object(
      Object::Endpoint(Endpoint {
        peer: Some(one),
        waiting: BTreeMap::new(),
        travelling_in: None,
      }),
      paid_by,
      OBJECT_COST.0,
    )?;
    let first = self.endpoint(one)?;

    first.peer = Some(other);

    Ok((one, other))
  }

  fn peer_with_room(&mut self, of: ObjectId) -> Result<ObjectId, HandleError> {
    let endpoint = self.endpoint(of)?;

    let peer = match endpoint.peer {
      Some(peer) => peer,
      None => return Err(HandleError::PeerClosed),
    };

    let receiving = self.endpoint(peer)?;

    let Ok(waiting) = fixed_index(receiving.waiting.len());
    waiting_room(waiting)?;

    Ok(peer)
  }

  fn afford(&mut self, job: ObjectId, amount: u64) -> Result<(), HandleError> {
    let held = self.job(job)?;
    let Ok(budget) = held.budget();

    charge_within(budget, amount).map(|_charged| ())
  }

  fn charge(&mut self, job: ObjectId, amount: u64) -> Result<(), HandleError> {
    let held = self.job(job)?;
    let Ok(budget) = held.budget();
    let checked_value = charge_within(budget, amount)?;
    held.charged = checked_value;

    Ok(())
  }

  fn uncharge(&mut self, job: ObjectId, amount: u64) -> Result<(), HandleError> {
    let held = self.job(job)?;

    held.charged = held.charged.saturating_sub(amount);

    Ok(())
  }

  fn kill_job(&mut self, job: ObjectId) -> Result<(), HandleError> {
    self.worked_through(job, Self::killed)
  }

  fn worked_through<T, F>(&mut self, first: T, mut each: F) -> Result<(), HandleError>
  where
    F: FnMut(&mut Self, T, &mut Vec<T>) -> Result<(), HandleError>,
  {
    #[cfg_attr(
      dylint_lib = "explicit066_no_heap_after_init",
      expect(
        explicit066_no_heap_after_init,
        reason = "iteration starts without allocating; pending work is bounded by the jobs and handles charged to the boot memory budget"
      )
    )]
    let mut pending = Vec::new();
    let Ok(objects) = fixed_index(self.objects.len());
    let limit = objects.saturating_mul(2).saturating_add(1);
    let mut next = first;

    for _ in 0..limit {
      each(self, next, &mut pending)?;

      next = match pending.pop() {
        Some(remaining) => remaining,
        None => return Ok(()),
      };
    }

    Err(HandleError::BadState)
  }

  fn killed(&mut self, job: ObjectId, waiting: &mut Vec<ObjectId>) -> Result<(), HandleError> {
    let held = match self.job(job) {
      Ok(held) => held,
      Err(HandleError::BadHandle) => return Ok(()),
      Err(fault) => return Err(fault),
    };

    let Ok(()) = held.kill();

    let processes: Vec<ProcessId> = held.processes.iter().copied().collect();
    waiting.extend(held.jobs.iter().copied());

    for process in processes {
      self.end(process)?;
    }

    self.reap(job)
  }

  fn end(&mut self, process: ProcessId) -> Result<(), HandleError> {
    let gone = match self.processes.remove(&process) {
      Some(gone) => gone,
      None => return Ok(()),
    };
    let job = self.job(gone.job)?;

    let _was_there = job.processes.remove(&process);

    for handle in gone.handles.into_values() {
      self.release(Charged {
        handle,
        paid_by: gone.job,
      })?;
    }

    self.reap(gone.job)
  }

  fn reap(&mut self, job: ObjectId) -> Result<(), HandleError> {
    self.worked_through(job, Self::reaped)
  }

  fn check_rests_elsewhere(
    &mut self,
    by: ProcessId,
    moving: &[HandleValue],
    queue: ObjectId,
  ) -> Result<(), HandleError> {
    let mut above = BTreeSet::new();
    let mut current = Some(queue);
    let Ok(objects) = fixed_index(self.objects.len());

    for _ in 0..objects {
      let end = match current {
        Some(end) => end,
        None => break,
      };
      let _fresh = above.insert(end);
      let Ok(next) = self.travelling_in(end);

      current = next;
    }

    match current {
      Some(_) => return Err(HandleError::BadState),
      None => {}
    }

    let sending = self.process(by)?;

    for value in moving {
      let named = sending.get(*value)?;

      match ::core::primitive::bool::then_some(above.contains(&named.object), ()) {
        ::core::option::Option::Some(()) => return Err(HandleError::SelfTransfer),
        ::core::option::Option::None => {}
      }
    }

    Ok(())
  }

  #[must_use = "where an end is travelling is the only thing this answers"]
  fn travelling_in(&self, end: ObjectId) -> Result<Option<ObjectId>, Never> {
    Ok(match self.objects.get(&end) {
      Some(Entry {
        object: Object::Endpoint(endpoint),
        ..
      }) => endpoint.travelling_in,
      Some(Entry {
        object:
          Object::Memory(_)
          | Object::PhysicalFrame(_)
          | Object::ExecutableResource
          | Object::TaskObj(_)
          | Object::Process(_),
        ..
      })
      | None => None,
    })
  }

  fn travel(&mut self, end: ObjectId, queue: Option<ObjectId>) -> Result<(), Never> {
    match self.objects.get_mut(&end) {
      Some(Entry {
        object: Object::Endpoint(endpoint),
        ..
      }) => endpoint.travelling_in = queue,
      Some(Entry {
        object:
          Object::Memory(_)
          | Object::PhysicalFrame(_)
          | Object::ExecutableResource
          | Object::TaskObj(_)
          | Object::Process(_),
        ..
      })
      | None => {}
    }

    Ok(())
  }

  fn reaped(&mut self, job: ObjectId, waiting: &mut Vec<ObjectId>) -> Result<(), HandleError> {
    let (parent, share, owed) = match self.objects.get(&job) {
      Some(Entry {
        object: Object::TaskObj(held),
        references: 0,
        ..
      }) => match (held.jobs.is_empty(), held.processes.is_empty()) {
        (true, true) => (held.parent, held.share, held.charged),
        (false, _) | (_, false) => return Ok(()),
      },
      Some(Entry { .. }) | None => return Ok(()),
    };

    match owed {
      0 => self.forgotten(job, parent, share, waiting),
      1.. => self.narrowed(job, parent, Bytes(share), Bytes(owed)),
    }
  }

  fn narrowed(
    &mut self,
    job: ObjectId,
    parent: Option<ObjectId>,
    share: Bytes,
    owed: Bytes,
  ) -> Result<(), HandleError> {
    let held = self.job(job)?;

    held.share = owed.0;

    match parent {
      Some(above) => {
        self.uncharge(above, share.0.saturating_sub(owed.0))?;
      }
      None => {}
    }

    Ok(())
  }

  fn forgotten(
    &mut self,
    job: ObjectId,
    parent: Option<ObjectId>,
    share: u64,
    waiting: &mut Vec<ObjectId>,
  ) -> Result<(), HandleError> {
    let paid_by = self.discard(job)?;

    match parent {
      Some(above) => {
        let held = self.job(above)?;
        let _was_there = held.jobs.remove(&job);
        held.charged = held.charged.saturating_sub(share);
        waiting.push(above);
      }
      None => {}
    }

    waiting.push(paid_by);

    Ok(())
  }

  fn discard(&mut self, object: ObjectId) -> Result<ObjectId, HandleError> {
    let gone = match self.objects.remove(&object) {
      Some(gone) => gone,
      None => return Err(HandleError::BadHandle),
    };

    match gone.object {
      Object::Memory(memory) => self.backing.release(memory.span)?,
      Object::Endpoint(_)
      | Object::PhysicalFrame(_)
      | Object::ExecutableResource
      | Object::TaskObj(_)
      | Object::Process(_) => {}
    }

    match ::core::primitive::bool::then_some(gone.paid_by == object, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        self.uncharge(gone.paid_by, gone.cost)?;
      }
    }

    Ok(gone.paid_by)
  }

  fn allowed(&mut self, by: ProcessId, condition: Condition) -> Result<(), HandleError> {
    let process = self.process(by)?;
    let job = process.job;
    let held = self.job(job)?;
    let Ok(action) = held.policy.action(condition);

    match action {
      PolicyAction::Allow => Ok(()),
      PolicyAction::Deny => Err(HandleError::DeniedByPolicy(condition)),
    }
  }

  fn running_job(&mut self, by: ProcessId, at: HandleValue) -> Result<ObjectId, HandleError> {
    let holder = self.process(by)?;
    let handle = holder.get(at)?;

    require(handle, Right::ManageJob)?;

    let job = self.job(handle.object)?;

    match job.state {
      JobState::Running => Ok(handle.object),
      JobState::Killed => Err(HandleError::BadState),
    }
  }

  fn descendants(&self, of: HandleId) -> Result<BTreeSet<HandleId>, Never> {
    use core::ops::Bound::{Excluded, Unbounded};

    let mut found = BTreeSet::new();
    let mut current = of;

    for _ in of.0..self.next_handle {
      match self.derived.get(&current) {
        Some(children) => found.extend(children.range((Excluded(current), Unbounded)).copied()),
        None => {}
      }

      current = match found.range((Excluded(current), Unbounded)).next() {
        Some(next) => *next,
        None => break,
      };
    }

    Ok(found)
  }

  fn revoked(&mut self, by: ProcessId, from: HandleValue) -> Result<Steps, HandleError> {
    let holder = self.process(by)?;
    let handle = holder.get(from)?;
    let Ok(under) = self.descendants(handle.id);
    let mut steps = 0_u64;
    let mut failure = None;

    for id in &under {
      match self.taken(*id) {
        Ok(found) => {
          let Charged {
            handle: handle_2,
            paid_by,
          } = found.held;
          let _prior = self.located.insert(
            *id,
            Location::Detached {
              object: handle_2.object,
              rights: handle_2.rights,
              paid_by,
            },
          );
          steps = steps.saturating_add(found.steps);
        }
        Err(fault) => {
          failure = Some(fault);
          break;
        }
      }
    }

    for id in under {
      match self.located.get(&id).copied() {
        Some(Location::Detached {
          object,
          rights,
          paid_by,
        }) => {
          let result = self.release(Charged {
            handle: Handle { id, object, rights },
            paid_by,
          });

          match result {
            Ok(()) => {}
            Err(fault) => failure = failure.or(Some(fault)),
          }
        }
        Some(Location::Table { .. } | Location::Message { .. }) | None => {}
      }
    }

    match failure {
      Some(fault) => Err(fault),
      None => Ok(Steps(steps)),
    }
  }

  fn taken(&mut self, id: HandleId) -> Result<Reclaimed, HandleError> {
    let location = match self.located.get(&id) {
      Some(location) => *location,
      None => return Err(HandleError::BadState),
    };

    match location {
      Location::Table { process, value } => {
        let holder = self.process(process)?;
        let paid_by = holder.job;
        let selected = holder.get(value)?;
        check_revocation_target(id, selected.id)?;
        let handle = holder.remove(value)?;

        Ok(Reclaimed {
          held: Charged { handle, paid_by },
          steps: TABLE_STEPS,
        })
      }
      Location::Message {
        endpoint,
        message,
        slot,
      } => {
        let receiving = self.endpoint(endpoint)?;

        let queued = match receiving.waiting.get_mut(&message) {
          Some(queued) => queued,
          None => return Err(HandleError::BadState),
        };

        let located = queued.handles.get(&slot).ok_or(HandleError::BadState)?;
        check_revocation_target(id, located.id)?;

        match queued.handles.remove(&slot) {
          Some(handle) => Ok(Reclaimed {
            held: Charged {
              handle,
              paid_by: queued.paid_by,
            },
            steps: MESSAGE_STEPS,
          }),
          None => Err(HandleError::BadState),
        }
      }
      Location::Detached { .. } => Err(HandleError::BadState),
    }
  }

  fn new_message(&mut self) -> Result<MessageId, HandleError> {
    let id = MessageId(self.next_message);
    let next = next_id(self.next_message)?;

    self.next_message = next;

    Ok(id)
  }

  fn lifted(&mut self, by: ProcessId, moving: &[HandleValue]) -> Result<Carried, HandleError> {
    let sending = self.process(by)?;
    let mut handles = BTreeMap::new();

    for (slot, value) in (0..MESSAGE_HANDLES_LIMIT).zip(moving) {
      let moved = sending.remove(*value)?;
      let _fresh = handles.insert(Slot(slot), moved);
    }

    Ok(handles)
  }

  fn hold(&mut self, into: ProcessId, handle: Handle) -> Result<HandleValue, HandleError> {
    let holder = self.process(into)?;
    let value = holder.insert(handle)?;
    let _moved = self.located.insert(
      handle.id,
      Location::Table {
        process: into,
        value,
      },
    );
    let Ok(()) = self.travel(handle.object, None);

    Ok(value)
  }

  fn new_process(&mut self, job: ObjectId) -> Result<ProcessId, HandleError> {
    let id = ProcessId(self.next_process);
    let next = next_id(self.next_process)?;

    self.next_process = next;

    let Ok(empty) = Process::empty(job);
    let _fresh = self.processes.insert(id, empty);
    let held = self.job(job)?;
    let _joined = held.processes.insert(id);

    Ok(id)
  }

  fn new_object(
    &mut self,
    object: Object,
    paid_by: ObjectId,
    cost: u64,
  ) -> Result<ObjectId, HandleError> {
    self.charge(paid_by, cost)?;

    let id = ObjectId(self.next_object);
    let next = next_id(self.next_object)?;

    self.next_object = next;

    let _fresh = self.objects.insert(
      id,
      Entry {
        object,
        references: 0,
        paid_by,
        cost,
      },
    );

    Ok(id)
  }

  fn install(
    &mut self,
    into: ProcessId,
    object: ObjectId,
    granted: Rights,
  ) -> Result<HandleValue, HandleError> {
    let asking = self.process(into)?;
    let paying = asking.job;

    self.charge(paying, HANDLE_COST.0)?;

    let id = HandleId(self.next_handle);
    let next = next_id(self.next_handle)?;

    self.next_handle = next;

    let entry = self.entry(object)?;
    let references = next_id(entry.references)?;

    entry.references = references;

    self.hold(
      into,
      Handle {
        id,
        object,
        rights: granted,
      },
    )
  }

  fn release(&mut self, held: Charged) -> Result<(), HandleError> {
    self.worked_through(held, Self::released)
  }

  fn released(&mut self, held: Charged, waiting: &mut Vec<Charged>) -> Result<(), HandleError> {
    let Ok(()) = self.disinherit(held.handle.id);
    let _was_there = self.located.remove(&held.handle.id);

    self.uncharge(held.paid_by, HANDLE_COST.0)?;

    self.dereferenced(held.handle.object, waiting)?;

    self.reap(held.paid_by)
  }

  fn dereferenced(
    &mut self,
    object: ObjectId,
    waiting: &mut Vec<Charged>,
  ) -> Result<(), HandleError> {
    let entry = self.entry(object)?;

    entry.references = entry.references.saturating_sub(1);

    match (entry.references, &entry.object) {
      (0, Object::Endpoint(_)) => self.hung_up(object, waiting),
      (
        0,
        Object::Memory(_)
        | Object::PhysicalFrame(_)
        | Object::ExecutableResource
        | Object::Process(_),
      ) => {
        let paid_by = self.discard(object)?;

        self.reap(paid_by)
      }
      (0, Object::TaskObj(_)) => self.reap(object),
      (
        1..,
        Object::Endpoint(_)
        | Object::Memory(_)
        | Object::PhysicalFrame(_)
        | Object::ExecutableResource
        | Object::TaskObj(_)
        | Object::Process(_),
      ) => Ok(()),
    }
  }

  fn hung_up(&mut self, object: ObjectId, waiting: &mut Vec<Charged>) -> Result<(), HandleError> {
    let endpoint = match self.objects.get_mut(&object) {
      Some(Entry {
        object: Object::Endpoint(endpoint),
        ..
      }) => core::mem::replace(
        endpoint,
        Endpoint {
          peer: None,
          waiting: BTreeMap::new(),
          travelling_in: None,
        },
      ),
      Some(Entry {
        object:
          Object::Memory(_)
          | Object::PhysicalFrame(_)
          | Object::ExecutableResource
          | Object::TaskObj(_)
          | Object::Process(_),
        ..
      })
      | None => {
        return Err(HandleError::WrongType(ObjectType::Channel));
      }
    };

    let paid_by = self.discard(object)?;

    for message in endpoint.waiting.into_values() {
      self.uncharge(message.paid_by, message.cost)?;

      let sender = message.paid_by;

      waiting.extend(message.handles.into_values().map(|handle| Charged {
        handle,
        paid_by: sender,
      }));

      self.reap(sender)?;
    }

    let peer = match endpoint.peer {
      Some(peer) => self.objects.get_mut(&peer),
      None => None,
    };

    match peer {
      Some(Entry {
        object: Object::Endpoint(other),
        ..
      }) => other.peer = None,
      Some(Entry {
        object:
          Object::Memory(_)
          | Object::PhysicalFrame(_)
          | Object::ExecutableResource
          | Object::TaskObj(_)
          | Object::Process(_),
        ..
      })
      | None => {}
    }

    self.reap(paid_by)
  }

  fn disinherit(&mut self, gone: HandleId) -> Result<(), Never> {
    let orphans = match self.derived.remove(&gone) {
      Some(orphans) => orphans,
      None => BTreeSet::new(),
    };

    match self.derived_from.remove(&gone) {
      Some(above) => {
        for orphan in &orphans {
          let _was_gone = self.derived_from.insert(*orphan, above);
        }

        let siblings = self.derived.entry(above).or_default();
        let _was_there = siblings.remove(&gone);

        siblings.extend(orphans);

        match ::core::primitive::bool::then_some(siblings.is_empty(), ()) {
          ::core::option::Option::Some(()) => {
            let _emptied = self.derived.remove(&above);
          }
          ::core::option::Option::None => {}
        }
      }
      None => {
        for orphan in &orphans {
          let _was_gone = self.derived_from.remove(orphan);
        }
      }
    }

    Ok(())
  }

  fn process(&mut self, id: ProcessId) -> Result<&mut Process, HandleError> {
    match self.processes.get_mut(&id) {
      Some(process) => Ok(process),
      None => Err(HandleError::ProcessNotFound),
    }
  }

  fn entry(&mut self, id: ObjectId) -> Result<&mut Entry, HandleError> {
    match self.objects.get_mut(&id) {
      Some(entry) => Ok(entry),
      None => Err(HandleError::BadHandle),
    }
  }

  fn memory_contents(
    &mut self,
    by: ProcessId,
    at: HandleValue,
    right: Right,
  ) -> Result<&mut [u8], HandleError> {
    let holder = self.process(by)?;
    let handle = holder.get(at)?;

    require(handle, right)?;

    let memory = self.memory_object(handle.object)?;
    memory.access.permits(right)?;
    let span = memory.span;

    self.backing.contents(span)
  }

  fn memory_object(&mut self, id: ObjectId) -> Result<&mut Memory, HandleError> {
    let entry = self.entry(id)?;

    match &mut entry.object {
      Object::Memory(memory) => Ok(memory),
      Object::PhysicalFrame(_)
      | Object::Endpoint(_)
      | Object::ExecutableResource
      | Object::TaskObj(_)
      | Object::Process(_) => Err(HandleError::WrongType(ObjectType::MemoryObject)),
    }
  }

  fn endpoint(&mut self, id: ObjectId) -> Result<&mut Endpoint, HandleError> {
    let entry = self.entry(id)?;

    match &mut entry.object {
      Object::Endpoint(endpoint) => Ok(endpoint),
      Object::Memory(_)
      | Object::PhysicalFrame(_)
      | Object::ExecutableResource
      | Object::TaskObj(_)
      | Object::Process(_) => Err(HandleError::WrongType(ObjectType::Channel)),
    }
  }

  fn job(&mut self, id: ObjectId) -> Result<&mut Task, HandleError> {
    let entry = self.entry(id)?;

    match &mut entry.object {
      Object::TaskObj(job) => Ok(job),
      Object::Endpoint(_)
      | Object::Memory(_)
      | Object::PhysicalFrame(_)
      | Object::ExecutableResource
      | Object::Process(_) => Err(HandleError::WrongType(ObjectType::TaskObj)),
    }
  }

  fn require_type(&mut self, id: ObjectId, wanted: ObjectType) -> Result<(), HandleError> {
    let entry = self.entry(id)?;

    let found = match entry.object {
      Object::Endpoint(_) => ObjectType::Channel,
      Object::Memory(_) => ObjectType::MemoryObject,
      Object::PhysicalFrame(_) => ObjectType::PhysicalFrame,
      Object::ExecutableResource => ObjectType::ExecutableResource,
      Object::TaskObj(_) => ObjectType::TaskObj,
      Object::Process(_) => ObjectType::Process,
    };

    match ::core::primitive::bool::then_some(found == wanted, ()) {
      ::core::option::Option::Some(()) => Ok(()),
      ::core::option::Option::None => Err(HandleError::WrongType(wanted)),
    }
  }
}

fn check_transfer(
  writer: &Process,
  through: HandleValue,
  moving: &[HandleValue],
) -> Result<(), HandleError> {
  let mut named = BTreeSet::new();

  for value in moving {
    match ::core::primitive::bool::then_some(*value == through, ()) {
      ::core::option::Option::Some(()) => return Err(HandleError::SelfTransfer),
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(named.insert(*value), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => return Err(HandleError::RepeatedHandle),
    }

    let handle = writer.get(*value)?;

    require(handle, Right::Transfer)?;
  }

  Ok(())
}

impl fmt::Display for ObjectType {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    to.write_str(match self {
      ObjectType::Channel => "a channel",
      ObjectType::MemoryObject => "memory",
      ObjectType::PhysicalFrame => "a physical frame",
      ObjectType::ExecutableResource => "the executable resource",
      ObjectType::TaskObj => "a job",
      ObjectType::Process => "a process",
    })
  }
}

impl fmt::Display for HandleError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      HandleError::ProcessNotFound => to.write_str("there is no such process"),
      HandleError::BadHandle => to.write_str("the process holds no such handle"),
      HandleError::WrongType(kind) => write!(to, "the handle is not to {kind}"),
      HandleError::AccessDenied(right) => {
        write!(to, "the handle does not carry the right to {right}")
      }
      HandleError::DeniedByPolicy(condition) => write!(to, "its job does not let it {condition}"),
      HandleError::InvalidRights => {
        to.write_str("a duplicate cannot carry a right the original does not")
      }
      HandleError::SelfTransfer => to.write_str(
        "a channel end cannot be written into itself, or into a queue that is on its way into it",
      ),
      HandleError::RepeatedHandle => {
        to.write_str("a handle cannot be moved twice in one message")
      }
      HandleError::PeerClosed => to.write_str("nobody holds the other end of the channel"),
      HandleError::ShouldWait => to.write_str("no message is waiting"),
      HandleError::ChannelFull => {
        to.write_str("the other end has not read what it was already sent")
      }
      HandleError::MessageTooLarge => {
        to.write_str("the message is larger than a channel carries")
      }
      HandleError::OverBudget => to.write_str("its job has no budget left for it"),
      HandleError::BadState => {
        to.write_str("the job has been killed, or already has something in it")
      }
      HandleError::NoResources => to.write_str("the kernel has run out of numbers"),
    }
  }
}
