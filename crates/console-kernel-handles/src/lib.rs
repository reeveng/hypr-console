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
//! handle is ever both.
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
//! again.
//!
//! Nothing here touches a machine. It is arithmetic over tables, `no_std` so
//! the kernel links it as it is, and tested on the host like any other crate.

#![no_std]

extern crate alloc;

mod policy;
mod rights;

#[cfg(test)]
mod tests;

pub use policy::{Condition, Policy, PolicyAction};
pub use rights::{Contains, Right, Rights};

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::vec::Vec;
use core::fmt;

use console_core_never::Never;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HandleValue(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ObjectId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct HandleId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    Channel,
    MemoryObject,
    ExecutableResource,
    Job,
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
    BadState,
    NoResources,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Message {
    pub bytes: Vec<u8>,
    pub handles: Vec<HandleValue>,
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
struct Handle {
    id: HandleId,
    object: ObjectId,
    rights: Rights,
}

struct QueuedMessage {
    bytes: Vec<u8>,
    handles: Vec<Handle>,
}

struct Endpoint {
    peer: Option<ObjectId>,
    waiting: VecDeque<QueuedMessage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JobState {
    Running,
    Killed,
}

struct Job {
    jobs: BTreeSet<ObjectId>,
    processes: BTreeSet<ProcessId>,
    policy: Policy,
    state: JobState,
}

enum Object {
    Endpoint(Endpoint),
    Memory,
    ExecutableResource,
    Job(Job),
    Process(ProcessId),
}

struct Entry {
    object: Object,
    references: u64,
}

type Handles = Vec<(HandleValue, Handle)>;

struct Process {
    handles: BTreeMap<HandleValue, Handle>,
    next_handle: u64,
    job: ObjectId,
}

pub struct Kernel {
    processes: BTreeMap<ProcessId, Process>,
    objects: BTreeMap<ObjectId, Entry>,
    derived: BTreeMap<HandleId, BTreeSet<HandleId>>,
    next_process: u64,
    next_object: u64,
    next_handle: u64,
}

fn next_id(from: u64) -> Result<u64, HandleError> {
    match from.checked_add(1) {
        Some(next) => Ok(next),
        None => Err(HandleError::NoResources),
    }
}

fn require(handle: Handle, right: Right) -> Result<(), HandleError> {
    let Ok(held) = handle.rights.contains(right);

    match held {
        Contains::Yes => Ok(()),
        Contains::No => Err(HandleError::AccessDenied(right)),
    }
}

fn rights(granted: &[Right]) -> Result<Rights, Never> {
    Rights::from_rights(granted)
}

impl Process {
    fn empty(job: ObjectId) -> Result<Process, Never> {
        Ok(Process { handles: BTreeMap::new(), next_handle: 1, job })
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
    pub fn boot() -> Result<Boot, HandleError> {
        let mut kernel = Kernel {
            processes: BTreeMap::new(),
            objects: BTreeMap::new(),
            derived: BTreeMap::new(),
            next_process: 1,
            next_object: 1,
            next_handle: 1,
        };
        let job = kernel.new_object(Object::Job(Job {
            jobs: BTreeSet::new(),
            processes: BTreeSet::new(),
            policy: Policy::ALLOW_ALL,
            state: JobState::Running,
        }))?;
        let root = kernel.new_process(job)?;
        let resource = kernel.new_object(Object::ExecutableResource)?;
        let Ok(granted) = rights(&[Right::Duplicate, Right::Transfer]);
        let executable_resource = kernel.install(root, resource, granted)?;
        let Ok(granted) = rights(&[Right::ManageJob, Right::Destroy, Right::Duplicate, Right::Transfer]);
        let root_job = kernel.install(root, job, granted)?;

        Ok(Boot { kernel, root, root_job, executable_resource })
    }

    pub fn create_job(&mut self, by: ProcessId, parent: HandleValue) -> Result<HandleValue, HandleError> {
        self.allowed(by, Condition::NewJob)?;

        let parent = self.running_job(by, parent)?;
        let above = self.job(parent)?;
        let policy = above.policy;
        let child = self.new_object(Object::Job(Job {
            jobs: BTreeSet::new(),
            processes: BTreeSet::new(),
            policy,
            state: JobState::Running,
        }))?;
        let above = self.job(parent)?;

        let _fresh = above.jobs.insert(child);

        let Ok(granted) = rights(&[Right::ManageJob, Right::Destroy, Right::Duplicate, Right::Transfer]);

        self.install(by, child, granted)
    }

    pub fn set_policy(&mut self, by: ProcessId, job: HandleValue, denied: &[Condition]) -> Result<(), HandleError> {
        let job = self.running_job(by, job)?;
        let held = self.job(job)?;

        match (held.jobs.is_empty(), held.processes.is_empty()) {
            (true, true) => {},
            (false, _) | (_, false) => return Err(HandleError::BadState),
        }

        let Ok(policy) = held.policy.denying(denied);

        held.policy = policy;

        Ok(())
    }

    pub fn create_process(&mut self, by: ProcessId, job: HandleValue) -> Result<CreatedProcess, HandleError> {
        self.allowed(by, Condition::NewProcess)?;

        let job = self.running_job(by, job)?;
        let (channel, given) = self.create_channel(by)?;
        let parent = self.process(by)?;
        let handle = parent.remove(given)?;
        let id = self.new_process(job)?;
        let started = self.process(id)?;

        started.insert(handle)?;

        let object = self.new_object(Object::Process(id))?;
        let Ok(granted) = rights(&[Right::Destroy, Right::Duplicate, Right::Transfer]);
        let process = self.install(by, object, granted)?;

        Ok(CreatedProcess { id, process, channel })
    }

    pub fn create_channel(&mut self, by: ProcessId) -> Result<(HandleValue, HandleValue), HandleError> {
        self.allowed(by, Condition::NewChannel)?;

        let one = self.new_object(Object::Endpoint(Endpoint { peer: None, waiting: VecDeque::new() }))?;
        let other = self.new_object(Object::Endpoint(Endpoint { peer: Some(one), waiting: VecDeque::new() }))?;
        let first = self.endpoint(one)?;

        first.peer = Some(other);

        let Ok(granted) = rights(&[Right::Read, Right::Write, Right::Transfer]);
        let first = self.install(by, one, granted)?;
        let second = self.install(by, other, granted)?;

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
        let handle = writer.get(through)?;

        require(handle, Right::Write)?;
        check_transfer(writer, through, moving)?;

        let endpoint = self.endpoint(handle.object)?;

        let peer = match endpoint.peer {
            Some(peer) => peer,
            None => return Err(HandleError::PeerClosed),
        };

        let writer = self.process(by)?;
        let mut handles = Vec::new();

        for value in moving {
            let handle = writer.remove(*value)?;

            handles.push(handle);
        }

        let receiving = self.endpoint(peer)?;

        receiving.waiting.push_back(QueuedMessage { bytes: bytes.to_vec(), handles });

        Ok(())
    }

    pub fn read(&mut self, by: ProcessId, through: HandleValue) -> Result<Message, HandleError> {
        let reader = self.process(by)?;
        let handle = reader.get(through)?;

        require(handle, Right::Read)?;

        let endpoint = self.endpoint(handle.object)?;

        let arrived = match (endpoint.waiting.pop_front(), endpoint.peer) {
            (Some(arrived), _) => arrived,
            (None, Some(_)) => return Err(HandleError::ShouldWait),
            (None, None) => return Err(HandleError::PeerClosed),
        };

        let reader = self.process(by)?;
        let mut handles = Vec::new();

        for handle in arrived.handles {
            let value = reader.insert(handle)?;

            handles.push(value);
        }

        Ok(Message { bytes: arrived.bytes, handles })
    }

    pub fn duplicate(&mut self, by: ProcessId, of: HandleValue, narrowed: Rights) -> Result<HandleValue, HandleError> {
        let holder = self.process(by)?;
        let handle = holder.get(of)?;

        require(handle, Right::Duplicate)?;

        let Ok(within) = narrowed.subset_of(handle.rights);

        match within {
            Contains::Yes => {},
            Contains::No => return Err(HandleError::InvalidRights),
        }

        let copy = self.install(by, handle.object, narrowed)?;
        let holder = self.process(by)?;
        let made = holder.get(copy)?;

        let _fresh = self.derived.entry(handle.id).or_default().insert(made.id);

        Ok(copy)
    }

    pub fn revoke(&mut self, by: ProcessId, from: HandleValue) -> Result<(), HandleError> {
        let holder = self.process(by)?;
        let handle = holder.get(from)?;
        let Ok(revoked) = self.descendants(handle.id);
        let Ok(taken) = self.taken_back(&revoked);

        for handle in taken {
            self.release(handle)?;
        }

        Ok(())
    }

    pub fn create_memory(&mut self, by: ProcessId) -> Result<HandleValue, HandleError> {
        self.allowed(by, Condition::NewMemoryObject)?;

        let object = self.new_object(Object::Memory)?;
        let Ok(granted) = rights(&[Right::Read, Right::Write, Right::Duplicate, Right::Transfer]);

        self.install(by, object, granted)
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

        self.require_type(granting.object, ObjectType::ExecutableResource)?;
        self.require_type(handle.object, ObjectType::MemoryObject)?;

        let Ok(unwritable) = handle.rights.difference(Right::Write);
        let Ok(executable) = unwritable.union(Right::Execute);
        let holder = self.process(by)?;

        holder.remove(memory)?;

        holder.insert(Handle { id: handle.id, object: handle.object, rights: executable })
    }

    pub fn handle_rights(&mut self, by: ProcessId, of: HandleValue) -> Result<Rights, HandleError> {
        let holder = self.process(by)?;
        let handle = holder.get(of)?;

        Ok(handle.rights)
    }

    pub fn handles(&mut self, by: ProcessId) -> Result<Vec<HandleValue>, HandleError> {
        let holder = self.process(by)?;

        Ok(holder.handles.keys().copied().collect())
    }

    pub fn close(&mut self, by: ProcessId, at: HandleValue) -> Result<(), HandleError> {
        let holder = self.process(by)?;
        let handle = holder.remove(at)?;

        self.release(handle)
    }

    pub fn kill(&mut self, by: ProcessId, task: HandleValue) -> Result<(), HandleError> {
        let holder = self.process(by)?;
        let handle = holder.get(task)?;

        require(handle, Right::Destroy)?;

        let entry = self.entry(handle.object)?;

        match entry.object {
            Object::Process(process) => self.ended(process),
            Object::Job(_) => self.killed(handle.object),
            Object::Endpoint(_) | Object::Memory | Object::ExecutableResource => {
                Err(HandleError::WrongType(ObjectType::Process))
            },
        }
    }

    pub fn exit(&mut self, process: ProcessId) -> Result<(), HandleError> {
        self.ended(process)
    }

    fn killed(&mut self, job: ObjectId) -> Result<(), HandleError> {
        let mut killing = alloc::vec![job];

        while let Some(job) = killing.pop() {
            let held = self.job(job)?;

            held.state = JobState::Killed;

            let processes: Vec<ProcessId> = held.processes.iter().copied().collect();

            killing.extend(held.jobs.iter().copied());

            for process in processes {
                self.ended(process)?;
            }
        }

        Ok(())
    }

    fn ended(&mut self, process: ProcessId) -> Result<(), HandleError> {
        let gone = match self.processes.remove(&process) {
            Some(gone) => gone,
            None => return Ok(()),
        };
        let job = self.job(gone.job)?;

        let _was_there = job.processes.remove(&process);

        for handle in gone.handles.into_values() {
            self.release(handle)?;
        }

        Ok(())
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
        let mut found = BTreeSet::new();
        let mut walking = alloc::vec![of];

        while let Some(handle) = walking.pop() {
            let children = self.derived.get(&handle).into_iter().flatten();

            for child in children {
                match found.insert(*child) {
                    true => walking.push(*child),
                    false => {},
                }
            }
        }

        Ok(found)
    }

    fn taken_back(&mut self, revoked: &BTreeSet<HandleId>) -> Result<Vec<Handle>, Never> {
        let mut taken = Vec::new();

        for process in self.processes.values_mut() {
            let (gone, kept): (Handles, Handles) =
                core::mem::take(&mut process.handles).into_iter().partition(|(_, handle)| revoked.contains(&handle.id));

            process.handles = kept.into_iter().collect();
            taken.extend(gone.into_iter().map(|(_, handle)| handle));
        }

        for entry in self.objects.values_mut() {
            match &mut entry.object {
                Object::Endpoint(endpoint) => {
                    for message in &mut endpoint.waiting {
                        let (gone, kept): (Vec<Handle>, Vec<Handle>) =
                            core::mem::take(&mut message.handles).into_iter().partition(|handle| revoked.contains(&handle.id));

                        message.handles = kept;
                        taken.extend(gone);
                    }
                },
                Object::Memory | Object::ExecutableResource | Object::Job(_) | Object::Process(_) => {},
            }
        }

        Ok(taken)
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

    fn new_object(&mut self, object: Object) -> Result<ObjectId, HandleError> {
        let id = ObjectId(self.next_object);
        let next = next_id(self.next_object)?;

        self.next_object = next;

        let _fresh = self.objects.insert(id, Entry { object, references: 0 });

        Ok(id)
    }

    fn install(&mut self, into: ProcessId, object: ObjectId, granted: Rights) -> Result<HandleValue, HandleError> {
        let id = HandleId(self.next_handle);
        let next = next_id(self.next_handle)?;

        self.next_handle = next;

        let entry = self.entry(object)?;
        let references = next_id(entry.references)?;

        entry.references = references;

        let holder = self.process(into)?;

        holder.insert(Handle { id, object, rights: granted })
    }

    fn release(&mut self, handle: Handle) -> Result<(), HandleError> {
        let mut released = alloc::vec![handle];

        while let Some(handle) = released.pop() {
            let _forgotten = self.derived.remove(&handle.id);
            let entry = self.entry(handle.object)?;

            entry.references = entry.references.saturating_sub(1);

            match (entry.references, &entry.object) {
                (0, Object::Endpoint(_)) => {},
                (_, Object::Endpoint(_) | Object::Memory | Object::ExecutableResource | Object::Job(_) | Object::Process(_)) => continue,
            }

            let endpoint = match self.objects.remove(&handle.object) {
                Some(Entry { object: Object::Endpoint(endpoint), .. }) => endpoint,
                Some(Entry { object: Object::Memory | Object::ExecutableResource | Object::Job(_) | Object::Process(_), .. }) | None => continue,
            };

            for message in endpoint.waiting {
                released.extend(message.handles);
            }

            let peer = match endpoint.peer {
                Some(peer) => self.objects.get_mut(&peer),
                None => None,
            };

            match peer {
                Some(Entry { object: Object::Endpoint(peer), .. }) => peer.peer = None,
                Some(Entry { object: Object::Memory | Object::ExecutableResource | Object::Job(_) | Object::Process(_), .. }) | None => {},
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

    fn endpoint(&mut self, id: ObjectId) -> Result<&mut Endpoint, HandleError> {
        let entry = self.entry(id)?;

        match &mut entry.object {
            Object::Endpoint(endpoint) => Ok(endpoint),
            Object::Memory | Object::ExecutableResource | Object::Job(_) | Object::Process(_) => {
                Err(HandleError::WrongType(ObjectType::Channel))
            },
        }
    }

    fn job(&mut self, id: ObjectId) -> Result<&mut Job, HandleError> {
        let entry = self.entry(id)?;

        match &mut entry.object {
            Object::Job(job) => Ok(job),
            Object::Endpoint(_) | Object::Memory | Object::ExecutableResource | Object::Process(_) => {
                Err(HandleError::WrongType(ObjectType::Job))
            },
        }
    }

    fn require_type(&mut self, id: ObjectId, wanted: ObjectType) -> Result<(), HandleError> {
        let entry = self.entry(id)?;

        let found = match entry.object {
            Object::Endpoint(_) => ObjectType::Channel,
            Object::Memory => ObjectType::MemoryObject,
            Object::ExecutableResource => ObjectType::ExecutableResource,
            Object::Job(_) => ObjectType::Job,
            Object::Process(_) => ObjectType::Process,
        };

        match found == wanted {
            true => Ok(()),
            false => Err(HandleError::WrongType(wanted)),
        }
    }
}

fn check_transfer(writer: &Process, through: HandleValue, moving: &[HandleValue]) -> Result<(), HandleError> {
    let mut named = BTreeSet::new();

    for value in moving {
        match *value == through {
            true => return Err(HandleError::SelfTransfer),
            false => {},
        }

        match named.insert(*value) {
            true => {},
            false => return Err(HandleError::RepeatedHandle),
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
            ObjectType::ExecutableResource => "the executable resource",
            ObjectType::Job => "a job",
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
            HandleError::AccessDenied(right) => write!(to, "the handle does not carry the right to {right}"),
            HandleError::DeniedByPolicy(condition) => write!(to, "its job does not let it {condition}"),
            HandleError::InvalidRights => to.write_str("a duplicate cannot carry a right the original does not"),
            HandleError::SelfTransfer => to.write_str("a channel end cannot be written into itself"),
            HandleError::RepeatedHandle => to.write_str("a handle cannot be moved twice in one message"),
            HandleError::PeerClosed => to.write_str("nobody holds the other end of the channel"),
            HandleError::ShouldWait => to.write_str("no message is waiting"),
            HandleError::BadState => to.write_str("the job has been killed, or already has something in it"),
            HandleError::NoResources => to.write_str("the kernel has run out of numbers"),
        }
    }
}
