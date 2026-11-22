//! Experiment E0's attack list, pressed against the handle model from outside.
//!
//! The design note asks for one answer from this file: which invariant breaks
//! first. So every test here is written as the promise the crate's head makes
//! and never as what the code happens to do, and a promise the model does not
//! keep stays here as the shortest sequence that shows it, ignored, with the
//! reason in the attribute, until the model keeps it. Nothing is weakened to
//! pass. The first run found three, the crate's head says what each became,
//! and none is ignored now.
//!
//! Only the public calls are used, which decides how a thing is observed. A
//! leak is the object count and the root job's budget not coming back to what
//! they were at boot. A handle that outlived a revoke is the memory it names
//! still being there after the one handle revoked from is closed. What a
//! handle is to is read off its rights, which is why the generator below never
//! takes `Duplicate` off a copy: a channel end is the one handle born without
//! it.
//!
//! The generator keeps one rule of its own, and the rule is the first finding.
//! A channel end is only ever written into the channel a process was born
//! with, whose own ends it never moves, so no end can come to rest in a queue
//! that only it can reach. Lifted, the books stopped balancing on most seeds;
//! the kernel now refuses the write that ties the knot, and the test beside
//! the rule runs with it lifted and balances.
//!
//! The long tests are a seeded generator walked over many seeds, so a failure
//! names a seed and a step and is the same failure on the next run. A refused
//! call is held to two things at every step: the fault is one the call is
//! documented to give, and nothing the caller can see has changed.

use std::collections::BTreeSet;

use console_core_never::Never;
use console_kernel_handles::{
  Boot, Budget, Bytes, Contains, CreatedProcess, HANDLE_COST, HandleError, HandleValue, Kernel,
  MESSAGE_BYTES_LIMIT, MESSAGE_COST, Message, OBJECT_COST, ProcessId, Right, Rights,
  WAITING_MESSAGES_LIMIT,
};

const MACHINE: Bytes = Bytes(268_435_456);

const SHARE: Bytes = Bytes(1_048_576);

const PAGE: Bytes = Bytes(4096);

const QUARTER: Bytes = Bytes(16_384);

const TOP_SHARE: Bytes = Bytes(262_144);

const WINDOW: u64 = 4_194_304;

const SEEDS: u64 = 192;

const STEPS: u32 = 384;

const MULTIPLIER: u64 = 6_364_136_223_846_793_005;

const INCREMENT: u64 = 1_442_695_040_888_963_407;

const LENGTHS: [u64; 5] = [0, 1, 9, 300, 5000];

const SIZES: [u64; 4] = [0, 1, 4096, 100_000];

const SHARES: [u64; 5] = [0, 300, 4096, 65_536, 1_000_000];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Books {
  objects: u64,
  root: Budget,
}

#[derive(Debug, PartialEq, Eq)]
struct Seen {
  handles: Vec<HandleValue>,
  books: Books,
}

#[derive(Debug, Clone, Copy)]
enum Attack {
  NewJob,
  NewProcess,
  NewChannel,
  NewMemory,
  Write,
  Read,
  Duplicate,
  Close,
  Revoke,
  Kill,
  Exit,
}

const ATTACKS: [Attack; 24] = [
  Attack::NewJob,
  Attack::NewJob,
  Attack::NewChannel,
  Attack::Write,
  Attack::Write,
  Attack::Read,
  Attack::Read,
  Attack::Close,
  Attack::NewProcess,
  Attack::NewProcess,
  Attack::NewChannel,
  Attack::NewMemory,
  Attack::NewMemory,
  Attack::Write,
  Attack::Write,
  Attack::Write,
  Attack::Read,
  Attack::Read,
  Attack::Duplicate,
  Attack::Duplicate,
  Attack::Close,
  Attack::Revoke,
  Attack::Kill,
  Attack::Exit,
];

#[derive(Debug, Clone, Copy)]
enum Narrowing {
  Whole,
  Unwritable,
  Unmovable,
}

const NARROWINGS: [Narrowing; 3] = [
  Narrowing::Whole,
  Narrowing::Unwritable,
  Narrowing::Unmovable,
];

#[derive(Debug, Clone, Copy)]
enum Ends {
  ThroughBootstrap,
  Anywhere,
}

#[derive(Debug, Clone, Copy)]
enum Shortest {
  OneByte,
  NoBytes,
}

#[derive(Debug, Clone, Copy)]
struct Rules {
  ends: Ends,
  shortest: Shortest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
  Channel,
  JobObject,
  Process,
  Memory,
  Resource,
}

struct Dice(u64);

struct World {
  kernel: Kernel,
  root: ProcessId,
  root_job: HandleValue,
  home: BTreeSet<HandleValue>,
  lent: HandleValue,
  tops: Vec<HandleValue>,
  kept: BTreeSet<HandleValue>,
  pinned: BTreeSet<(ProcessId, HandleValue)>,
  actors: Vec<ProcessId>,
  before: Books,
  rules: Rules,
  seed: u64,
  step: u32,
}

struct Sorted {
  by: ProcessId,
  free: Vec<HandleValue>,
  channels: Vec<HandleValue>,
  jobs: Vec<HandleValue>,
  tasks: Vec<HandleValue>,
  copies: Vec<(HandleValue, Rights)>,
}

struct Hand {
  by: ProcessId,
  channel: Option<HandleValue>,
  job: Option<HandleValue>,
  task: Option<HandleValue>,
  copied: Option<(HandleValue, Rights)>,
  any: Option<HandleValue>,
  moving: Vec<HandleValue>,
}

struct Refusal {
  attack: Attack,
  fault: HandleError,
  by: ProcessId,
  before: Seen,
}

impl Dice {
  fn below(&mut self, limit: u64) -> Result<u64, HandleError> {
    self.0 = self.0.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT);

    match self.0.wrapping_shr(33).checked_rem(limit) {
      Some(rolled) => Ok(rolled),
      None => Err(HandleError::BadState),
    }
  }

  fn one_of<T: Copy>(&mut self, from: &[T]) -> Result<Option<T>, HandleError> {
    let counted = match u64::try_from(from.len()) {
      Ok(counted) => counted,
      Err(_more_than_the_dice_count) => return Err(HandleError::NoResources),
    };

    match counted {
      0 => Ok(None),
      1.. => {
        let at = self.below(counted)?;

        Ok(
          (0..counted)
            .zip(from)
            .find(|(place, _)| *place == at)
            .map(|(_, found)| *found),
        )
      }
    }
  }
}

fn first(handles: &[HandleValue]) -> Result<HandleValue, HandleError> {
  match handles.first() {
    Some(handle) => Ok(*handle),
    None => Err(HandleError::BadHandle),
  }
}

fn bootstrap(kernel: &mut Kernel, process: ProcessId) -> Result<HandleValue, HandleError> {
  let held = kernel.handles(process)?;

  first(&held)
}

fn received(
  kernel: &mut Kernel,
  by: ProcessId,
  through: HandleValue,
) -> Result<HandleValue, HandleError> {
  let message = kernel.read(by, through)?;

  first(&message.handles)
}

fn books(
  kernel: &mut Kernel,
  root: ProcessId,
  root_job: HandleValue,
) -> Result<Books, HandleError> {
  let objects = kernel.object_count()?;
  let budget = kernel.budget(root, root_job)?;

  Ok(Books {
    objects,
    root: budget,
  })
}

fn plus(budget: Budget, more: u64) -> Result<Budget, Never> {
  Ok(Budget {
    share: budget.share,
    charged: Bytes(budget.charged.0.saturating_add(more)),
  })
}

fn minus(budget: Budget, less: u64) -> Result<Budget, Never> {
  Ok(Budget {
    share: budget.share,
    charged: Bytes(budget.charged.0.saturating_sub(less)),
  })
}

fn kind(rights: Rights) -> Result<Kind, Never> {
  let Ok(copies) = rights.contains(Right::Duplicate);
  let Ok(manages) = rights.contains(Right::ManageJob);
  let Ok(ends) = rights.contains(Right::Destroy);
  let Ok(reads) = rights.contains(Right::Read);

  Ok(match (copies, manages, ends, reads) {
    (Contains::No, _, _, _) => Kind::Channel,
    (Contains::Yes, Contains::Yes, _, _) => Kind::JobObject,
    (Contains::Yes, Contains::No, Contains::Yes, _) => Kind::Process,
    (Contains::Yes, Contains::No, Contains::No, Contains::Yes) => Kind::Memory,
    (Contains::Yes, Contains::No, Contains::No, Contains::No) => Kind::Resource,
  })
}

fn excused(attack: Attack) -> Result<&'static [HandleError], Never> {
  Ok(match attack {
    Attack::NewJob | Attack::NewProcess => &[HandleError::OverBudget, HandleError::BadState],
    Attack::NewChannel | Attack::NewMemory | Attack::Duplicate => &[HandleError::OverBudget],
    Attack::Write => &[
      HandleError::ChannelFull,
      HandleError::PeerClosed,
      HandleError::OverBudget,
      HandleError::AccessDenied(Right::Transfer),
      HandleError::SelfTransfer,
    ],
    Attack::Read => &[
      HandleError::ShouldWait,
      HandleError::PeerClosed,
      HandleError::OverBudget,
    ],
    Attack::Close | Attack::Revoke | Attack::Kill | Attack::Exit => &[],
  })
}

fn founded(seed: u64, rules: Rules) -> Result<World, HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let before = books(&mut kernel, root, root_job)?;
  let born_with = kernel.handles(root)?;
  let home: BTreeSet<HandleValue> = born_with.into_iter().collect();
  let lent = kernel.create_memory(root, PAGE)?;
  let mut kept = home.clone();
  let _fresh = kept.insert(lent);
  let mut world = World {
    kernel,
    root,
    root_job,
    home,
    lent,
    tops: Vec::new(),
    kept,
    pinned: BTreeSet::new(),
    actors: vec![root],
    before,
    rules,
    seed,
    step: 0,
  };

  for _top in 0..3_u8 {
    planted(&mut world)?;
  }

  Ok(world)
}

fn planted(world: &mut World) -> Result<(), HandleError> {
  let top = world
    .kernel
    .create_job(world.root, world.root_job, TOP_SHARE)?;
  let managing = world.kernel.handle_rights(world.root, top)?;
  let _spare = world.kernel.duplicate(world.root, top, managing)?;
  let _fresh = world.kept.insert(top);

  world.tops.push(top);

  for _process in 0..2_u8 {
    let started = world.kernel.create_process(world.root, top)?;
    let owning = world.kernel.handle_rights(world.root, world.lent)?;
    let copy = world.kernel.duplicate(world.root, world.lent, owning)?;

    world
      .kernel
      .write(world.root, started.channel, b"lent", &[copy])?;

    joined_up(world, world.root, started)?;
  }

  Ok(())
}

fn joined_up(
  world: &mut World,
  by: ProcessId,
  started: CreatedProcess,
) -> Result<(), HandleError> {
  let only = bootstrap(&mut world.kernel, started.id)?;
  let _near = world.pinned.insert((by, started.channel));
  let _far = world.pinned.insert((started.id, only));

  world.actors.push(started.id);

  Ok(())
}

fn sorted(
  world: &mut World,
  by: ProcessId,
  held: Vec<HandleValue>,
) -> Result<Sorted, HandleError> {
  let mut sorted = Sorted {
    by,
    free: Vec::new(),
    channels: Vec::new(),
    jobs: Vec::new(),
    tasks: Vec::new(),
    copies: Vec::new(),
  };

  let kept = match ::core::primitive::bool::then_some(by == world.root, ()) {
    ::core::option::Option::Some(()) => world.kept.clone(),
    ::core::option::Option::None => BTreeSet::new(),
  };

  for handle in held.into_iter().filter(|handle| !kept.contains(handle)) {
    let granted = world.kernel.handle_rights(by, handle)?;
    let Ok(found) = kind(granted);

    sorted.free.push(handle);

    match found {
      Kind::Channel => sorted.channels.push(handle),
      Kind::JobObject => {
        sorted.jobs.push(handle);
        sorted.tasks.push(handle);
        sorted.copies.push((handle, granted));
      }
      Kind::Process => {
        sorted.tasks.push(handle);
        sorted.copies.push((handle, granted));
      }
      Kind::Memory | Kind::Resource => sorted.copies.push((handle, granted)),
    }
  }

  Ok(sorted)
}

fn carried(
  world: &World,
  dice: &mut Dice,
  sorted: &Sorted,
  through: Option<HandleValue>,
) -> Result<Vec<HandleValue>, HandleError> {
  let fixed = through.map(|end| world.pinned.contains(&(sorted.by, end)));

  let movable: Vec<HandleValue> = match (world.rules.ends, fixed) {
    (Ends::ThroughBootstrap, Some(true)) => sorted
      .free
      .iter()
      .copied()
      .filter(|handle| !world.pinned.contains(&(sorted.by, *handle)))
      .collect(),
    (Ends::ThroughBootstrap, Some(false) | None) => {
      sorted.copies.iter().map(|(handle, _)| *handle).collect()
    }
    (Ends::Anywhere, _) => sorted
      .free
      .iter()
      .copied()
      .filter(|handle| Some(*handle) != through)
      .collect(),
  };
  let mut moving = Vec::new();

  for handle in movable {
    let rolled = dice.below(6)?;

    match (rolled, moving.len() < 3) {
      (0, true) => moving.push(handle),
      (0, false) | (1.., _) => {}
    }
  }

  Ok(moving)
}

fn dealt(world: &mut World, dice: &mut Dice, by: ProcessId) -> Result<Option<Hand>, HandleError> {
  let held = match world.kernel.handles(by) {
    Ok(held) => held,
    Err(HandleError::ProcessNotFound) => return Ok(None),
    Err(fault) => return Err(fault),
  };
  let sorted = sorted(world, by, held)?;
  let channel = dice.one_of(&sorted.channels)?;
  let job = dice.one_of(&sorted.jobs)?;
  let task = dice.one_of(&sorted.tasks)?;
  let copied = dice.one_of(&sorted.copies)?;
  let any = dice.one_of(&sorted.free)?;
  let moving = carried(world, dice, &sorted, channel)?;

  Ok(Some(Hand {
    by,
    channel,
    job,
    task,
    copied,
    any,
    moving,
  }))
}

fn new_job(world: &mut World, dice: &mut Dice, hand: &Hand) -> Result<(), HandleError> {
  let picked = dice.one_of(&SHARES)?;

  match (hand.job, picked) {
    (Some(parent), Some(share)) => {
      let above = world.kernel.budget(hand.by, parent)?;
      let _made = world.kernel.create_job(hand.by, parent, Bytes(share))?;
      let after = world.kernel.budget(hand.by, parent)?;
      let Ok(least) = plus(above, share);

      assert!(
        after.charged >= least.charged,
        "seed {}, step {}: a share did not come out of the job above",
        world.seed,
        world.step
      );

      Ok(())
    }
    (None, _) | (_, None) => Ok(()),
  }
}

fn new_process(world: &mut World, hand: &Hand) -> Result<(), HandleError> {
  match hand.job {
    Some(into) => {
      let started = world.kernel.create_process(hand.by, into)?;

      joined_up(world, hand.by, started)
    }
    None => Ok(()),
  }
}

fn new_memory(world: &mut World, dice: &mut Dice, hand: &Hand) -> Result<(), HandleError> {
  let picked = dice.one_of(&SIZES)?;

  match picked {
    Some(size) => {
      let _made = world.kernel.create_memory(hand.by, Bytes(size))?;

      Ok(())
    }
    None => Ok(()),
  }
}

fn written(world: &mut World, dice: &mut Dice, hand: &Hand) -> Result<(), HandleError> {
  let picked = dice.one_of(&LENGTHS)?;

  let length = match (picked, world.rules.shortest) {
    (Some(length), Shortest::OneByte) => length.max(1),
    (Some(length), Shortest::NoBytes) => length,
    (None, _) => return Ok(()),
  };
  let bytes: Vec<u8> = (0..length).map(|_place| 7_u8).collect();

  match hand.channel {
    Some(through) => world.kernel.write(hand.by, through, &bytes, &hand.moving),
    None => Ok(()),
  }
}

fn read(world: &mut World, hand: &Hand) -> Result<(), HandleError> {
  match hand.channel {
    Some(through) => {
      let _arrived = world.kernel.read(hand.by, through)?;

      Ok(())
    }
    None => Ok(()),
  }
}

fn duplicated(world: &mut World, dice: &mut Dice, hand: &Hand) -> Result<(), HandleError> {
  let picked = dice.one_of(&NARROWINGS)?;

  match (hand.copied, picked) {
    (Some((of, granted)), Some(narrowing)) => {
      let Ok(narrowed) = match narrowing {
        Narrowing::Whole => Ok(granted),
        Narrowing::Unwritable => granted.difference(Right::Write),
        Narrowing::Unmovable => granted.difference(Right::Transfer),
      };
      let _copy = world.kernel.duplicate(hand.by, of, narrowed)?;

      Ok(())
    }
    (None, _) | (_, None) => Ok(()),
  }
}

fn exited(world: &mut World, dice: &mut Dice) -> Result<(), HandleError> {
  let picked = dice.one_of(&world.actors)?;

  match picked {
    Some(leaving) => match ::core::primitive::bool::then_some(leaving == world.root, ()) {
      ::core::option::Option::Some(()) => Ok(()),
      ::core::option::Option::None => world.kernel.exit(leaving),
    },
    None => Ok(()),
  }
}

fn attempted(
  world: &mut World,
  dice: &mut Dice,
  hand: &Hand,
  attack: Attack,
) -> Result<(), HandleError> {
  match (attack, hand.any, hand.task) {
    (Attack::NewJob, _, _) => new_job(world, dice, hand),
    (Attack::NewProcess, _, _) => new_process(world, hand),
    (Attack::NewChannel, _, _) => world.kernel.create_channel(hand.by).map(|_ends| ()),
    (Attack::NewMemory, _, _) => new_memory(world, dice, hand),
    (Attack::Write, _, _) => written(world, dice, hand),
    (Attack::Read, _, _) => read(world, hand),
    (Attack::Duplicate, _, _) => duplicated(world, dice, hand),
    (Attack::Close, Some(at), _) => world.kernel.close(hand.by, at),
    (Attack::Revoke, Some(from), _) => world.kernel.revoke(hand.by, from),
    (Attack::Kill, _, Some(task)) => world.kernel.kill(hand.by, task),
    (Attack::Exit, _, _) => exited(world, dice),
    (Attack::Close | Attack::Revoke, None, _) | (Attack::Kill, _, None) => Ok(()),
  }
}

fn seen(world: &mut World, by: ProcessId) -> Result<Seen, HandleError> {
  let handles = world.kernel.handles(by)?;
  let books = books(&mut world.kernel, world.root, world.root_job)?;

  Ok(Seen { handles, books })
}

fn refused(world: &mut World, refusal: &Refusal) -> Result<(), HandleError> {
  let Ok(excused) = excused(refusal.attack);

  assert!(
    excused.contains(&refusal.fault),
    "seed {}, step {}: {:?} was refused with {:?}",
    world.seed,
    world.step,
    refusal.attack,
    refusal.fault
  );

  let after = seen(world, refusal.by)?;

  assert_eq!(
    after, refusal.before,
    "seed {}, step {}: a refused {:?} left something changed",
    world.seed, world.step, refusal.attack
  );

  Ok(())
}

fn pressed(world: &mut World, dice: &mut Dice) -> Result<(), HandleError> {
  let chosen = dice.one_of(&world.actors)?;

  let by = match chosen {
    Some(by) => by,
    None => return Err(HandleError::ProcessNotFound),
  };
  let offered = dealt(world, dice, by)?;
  let picked = dice.one_of(&ATTACKS)?;

  match (offered, picked) {
    (Some(hand), Some(attack)) => {
      let before = seen(world, by)?;

      match attempted(world, dice, &hand, attack) {
        Ok(()) => Ok(()),
        Err(fault) => refused(
          world,
          &Refusal {
            attack,
            fault,
            by,
            before,
          },
        ),
      }
    }
    (None, _) | (_, None) => Ok(()),
  }
}

fn within(world: &mut World, by: ProcessId) -> Result<u64, HandleError> {
  let held = match world.kernel.handles(by) {
    Ok(held) => held,
    Err(HandleError::ProcessNotFound) => return Ok(0),
    Err(fault) => return Err(fault),
  };

  let mut to_something_else = 0_u64;

  for handle in held {
    match world.kernel.budget(by, handle) {
      Ok(budget) => {
        assert!(
          budget.charged <= budget.share,
          "seed {}, step {}: a job is charged {:?}",
          world.seed,
          world.step,
          budget
        );
      }
      Err(HandleError::WrongType(_not_a_job)) => {
        to_something_else = to_something_else.saturating_add(1)
      }
      Err(fault) => return Err(fault),
    }
  }

  Ok(to_something_else)
}

fn audited(world: &mut World) -> Result<(), HandleError> {
  let root = world.kernel.budget(world.root, world.root_job)?;
  let mut shared = 0_u64;

  for top in world.tops.clone() {
    let budget = world.kernel.budget(world.root, top)?;

    shared = shared.saturating_add(budget.share.0);
  }

  assert!(
    shared <= root.charged.0,
    "seed {}, step {}: the root is charged less than it shared out",
    world.seed,
    world.step
  );

  for actor in world.actors.clone() {
    let _to_something_else = within(world, actor)?;
  }

  Ok(())
}

fn settled(world: &mut World) -> Result<(), HandleError> {
  world.kernel.revoke(world.root, world.lent)?;

  let with = world.kernel.object_count()?;

  world.kernel.close(world.root, world.lent)?;

  assert_eq!(
    world.kernel.object_count(),
    Ok(with.saturating_sub(1)),
    "seed {}: a copy of what was lent outlived its revoke",
    world.seed
  );

  for top in world.tops.clone() {
    world.kernel.kill(world.root, top)?;
  }

  for actor in world.actors.clone() {
    match ::core::primitive::bool::then_some(actor == world.root, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => assert_eq!(
        world.kernel.handles(actor),
        Err(HandleError::ProcessNotFound),
        "seed {}: a process outlived its job",
        world.seed
      ),
    }
  }

  let held = world.kernel.handles(world.root)?;

  for handle in held {
    match ::core::primitive::bool::then_some(world.home.contains(&handle), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        world.kernel.close(world.root, handle)?;
      }
    }
  }

  let after = books(&mut world.kernel, world.root, world.root_job)?;

  assert_eq!(
    after, world.before,
    "seed {}: the books did not come back to what they were at boot",
    world.seed
  );

  Ok(())
}

fn walked(seed: u64, rules: Rules) -> Result<(), HandleError> {
  let mut world = founded(seed, rules)?;
  let mut dice = Dice(seed);

  for step in 0..STEPS {
    world.step = step;

    pressed(&mut world, &mut dice)?;
    audited(&mut world)?;
  }

  settled(&mut world)
}

fn every_seed(rules: Rules) -> Result<(), HandleError> {
  for seed in 0..SEEDS {
    assert_eq!(walked(seed, rules), Ok(()), "seed {seed}");
  }

  Ok(())
}

#[test]
fn whatever_is_done_in_whatever_order_the_books_balance_when_everything_is_let_go()
-> Result<(), HandleError> {
  every_seed(Rules {
    ends: Ends::ThroughBootstrap,
    shortest: Shortest::OneByte,
  })
}

#[test]
fn the_books_balance_when_a_message_may_say_nothing() -> Result<(), HandleError> {
  every_seed(Rules {
    ends: Ends::ThroughBootstrap,
    shortest: Shortest::NoBytes,
  })
}

#[test]
fn the_books_balance_when_a_channel_end_may_be_written_into_any_channel()
-> Result<(), HandleError> {
  every_seed(Rules {
    ends: Ends::Anywhere,
    shortest: Shortest::OneByte,
  })
}

#[test]
fn a_revoke_reaches_every_copy_however_it_travelled_and_wherever_it_is_waiting()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    executable_resource,
  } = Kernel::boot(MACHINE)?;
  let job = kernel.create_job(root, root_job, SHARE)?;
  let borrower = kernel.create_process(root, job)?;
  let lent = kernel.create_memory(root, PAGE)?;
  let owning = kernel.handle_rights(root, lent)?;

  let given = kernel.duplicate(root, lent, owning)?;
  kernel.write(root, borrower.channel, b"", &[given])?;
  let only = bootstrap(&mut kernel, borrower.id)?;
  let theirs = received(&mut kernel, borrower.id, only)?;
  let kept = kernel.duplicate(borrower.id, theirs, owning)?;
  kernel.close(borrower.id, theirs)?;
  let returned = kernel.duplicate(borrower.id, kept, owning)?;
  kernel.write(borrower.id, only, b"", &[returned])?;

  let (outer, outer_far) = kernel.create_channel(root)?;
  let (inner, inner_far) = kernel.create_channel(root)?;
  let buried = kernel.duplicate(root, lent, owning)?;
  kernel.write(root, inner, b"", &[buried])?;
  kernel.write(root, outer, b"", &[inner_far])?;

  let copied = kernel.duplicate(root, lent, owning)?;
  let running = kernel.replace_as_executable(root, copied, executable_resource)?;
  let with = kernel.object_count()?;

  kernel.revoke(root, lent)?;

  assert_eq!(
    kernel.handle_rights(borrower.id, kept),
    Err(HandleError::BadHandle)
  );
  assert_eq!(
    kernel.handle_rights(root, running),
    Err(HandleError::BadHandle)
  );
  assert_eq!(
    kernel.read(root, borrower.channel),
    Ok(Message {
      bytes: Vec::new(),
      handles: Default::default()
    })
  );
  assert_eq!(kernel.object_count(), Ok(with));
  kernel.close(root, lent)?;
  assert_eq!(kernel.object_count(), Ok(with.saturating_sub(1)));
  let carrier = received(&mut kernel, root, outer_far)?;
  assert_eq!(
    kernel.read(root, carrier),
    Ok(Message {
      bytes: Vec::new(),
      handles: Default::default()
    })
  );

  Ok(())
}

#[derive(Debug, Clone, Copy)]
enum FrameStep {
  Receive,
  Access,
  Revoke,
}

#[test]
fn a_frame_is_readable_only_between_receipt_and_revocation_in_every_order()
-> Result<(), HandleError> {
  use FrameStep::{Access, Receive, Revoke};

  for order in [
    [Receive, Access, Revoke],
    [Receive, Revoke, Access],
    [Access, Receive, Revoke],
    [Access, Revoke, Receive],
    [Revoke, Receive, Access],
    [Revoke, Access, Receive],
  ] {
    frame_transfer(order)?;
  }

  Ok(())
}

fn frame_transfer(order: [FrameStep; 3]) -> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    executable_resource: _,
  } = Kernel::boot(MACHINE)?;
  let before = books(&mut kernel, root, root_job)?;
  let job = kernel.create_job(root, root_job, SHARE)?;
  let renderer = kernel.create_process(root, job)?;
  let channel = bootstrap(&mut kernel, renderer.id)?;
  let frame = kernel.create_memory(renderer.id, PAGE)?;

  let contents = kernel.write_memory(renderer.id, frame)?;
  contents.fill(0xa5);

  let Ok(readable) = Rights::from_rights(&[Right::Read, Right::Transfer]);
  let loan = kernel.duplicate(renderer.id, frame, readable)?;

  kernel.write(renderer.id, channel, b"frame", &[loan])?;

  let browser_handles = kernel.handles(root)?;
  let mut received = None;
  let mut revoked = false;

  for step in order {
    match step {
      FrameStep::Receive => {
        let message = kernel.read(root, renderer.channel)?;

        assert_eq!(message.bytes, b"frame", "{order:?}");
        let Ok(count) = console_core_number_conversion::fixed_index(message.handles.len());
        assert_eq!(count, u64::from(!revoked), "{order:?}");

        received = message.handles.first().copied();
      }
      FrameStep::Access => match (received, revoked) {
        (Some(handle), false) => {
          let pixels = kernel.read_memory(root, handle)?;

          assert_eq!(u64::try_from(pixels.len()), Ok(PAGE.0), "{order:?}");
          assert!(pixels.iter().all(|byte| *byte == 0xa5), "{order:?}");
          assert_eq!(
            kernel.write_memory(root, handle),
            Err(HandleError::AccessDenied(Right::Write)),
            "{order:?}"
          );
        }
        (Some(handle), true) => {
          assert_eq!(
            kernel.read_memory(root, handle),
            Err(HandleError::BadHandle),
            "{order:?}"
          );
        }
        (None, _) => {
          let retained = kernel.handles(root)?;
          assert_eq!(retained, browser_handles, "{order:?}");
        }
      },
      FrameStep::Revoke => {
        kernel.revoke(renderer.id, frame)?;

        revoked = true;
      }
    }
  }

  let retained = kernel.handles(root)?;
  assert_eq!(retained, browser_handles, "{order:?}");

  kernel.close(renderer.id, frame)?;
  kernel.kill(root, job)?;
  kernel.close(root, renderer.channel)?;
  kernel.close(root, renderer.process)?;
  kernel.close(root, job)?;

  assert_eq!(books(&mut kernel, root, root_job), Ok(before), "{order:?}");

  Ok(())
}

#[test]
fn a_reader_that_never_reads_costs_its_writer_one_window_and_no_more() -> Result<(), HandleError>
{
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let writers = kernel.create_job(root, root_job, Bytes(16_777_216))?;
  let writer = kernel.create_process(root, writers)?;
  let out = bootstrap(&mut kernel, writer.id)?;
  let quiet = kernel.budget(root, writers)?;
  let longest = [7_u8; 65_536];
  let mut accepted = 0_u64;
  let mut full = 0_u64;

  for _attempt in 0..256_u16 {
    match kernel.write(writer.id, out, &longest, &[]) {
      Ok(()) => accepted = accepted.saturating_add(1),
      Err(HandleError::ChannelFull) => full = full.saturating_add(1),
      Err(fault) => return Err(fault),
    }
  }

  let Ok(most) = plus(
    quiet,
    WINDOW.saturating_add(u64::from(WAITING_MESSAGES_LIMIT).saturating_mul(MESSAGE_COST.0)),
  );

  assert_eq!(
    u64::from(WAITING_MESSAGES_LIMIT).saturating_mul(u64::from(MESSAGE_BYTES_LIMIT)),
    WINDOW
  );
  assert_eq!(accepted, u64::from(WAITING_MESSAGES_LIMIT));
  assert_eq!(full, 192);
  assert_eq!(kernel.budget(root, writers), Ok(most));

  Ok(())
}

#[test]
fn a_writer_in_a_small_job_is_stopped_by_its_budget_or_the_window_and_keeps_every_handle()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let writers = kernel.create_job(root, root_job, Bytes(200_000))?;
  let writer = kernel.create_process(root, writers)?;
  let out = bootstrap(&mut kernel, writer.id)?;
  let longest = [7_u8; 65_536];
  let mut over_budget = 0_u64;
  let mut accepted = 0_u64;

  for _attempt in 0..256_u16 {
    let memory = kernel.create_memory(writer.id, Bytes(0))?;
    let held = kernel.handles(writer.id)?;
    let paid = kernel.budget(root, writers)?;

    match kernel.write(writer.id, out, &longest, &[memory]) {
      Ok(()) => accepted = accepted.saturating_add(1),
      Err(HandleError::OverBudget) => {
        over_budget = over_budget.saturating_add(1);

        assert_eq!(kernel.handles(writer.id), Ok(held));
        assert_eq!(kernel.budget(root, writers), Ok(paid));
        kernel.close(writer.id, memory)?;
      }
      Err(fault) => return Err(fault),
    }
  }

  assert_eq!(accepted, 3);
  assert_eq!(over_budget, 253);

  Ok(())
}

#[test]
fn a_reader_that_never_reads_holds_no_more_than_one_window_of_a_dead_writers_parent()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let quiet = kernel.budget(root, root_job)?;
  let writers = kernel.create_job(root, root_job, Bytes(67_108_864))?;
  let writer = kernel.create_process(root, writers)?;
  let out = bootstrap(&mut kernel, writer.id)?;

  kernel.write(writer.id, out, b"!", &[])?;
  kernel.kill(root, writers)?;
  kernel.close(root, writers)?;
  kernel.close(root, writer.process)?;

  let held = kernel.budget(root, root_job)?;
  let reading = OBJECT_COST.0.saturating_add(HANDLE_COST.0);
  let Ok(most) = plus(quiet, WINDOW.saturating_add(reading));

  assert!(
    held.charged <= most.charged,
    "the job above is still charged {:?} of {:?}",
    held.charged,
    most.charged
  );

  Ok(())
}

#[test]
fn a_channel_end_waiting_inside_another_channel_goes_when_the_outer_one_does()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let before = books(&mut kernel, root, root_job)?;
  let (inner, inner_far) = kernel.create_channel(root)?;
  let (outer, outer_far) = kernel.create_channel(root)?;
  let memory = kernel.create_memory(root, PAGE)?;

  kernel.write(root, inner, b"inner", &[memory])?;
  kernel.write(root, outer, b"outer", &[inner_far])?;
  kernel.close(root, inner)?;
  kernel.close(root, outer)?;
  kernel.close(root, outer_far)?;

  assert_eq!(books(&mut kernel, root, root_job), Ok(before));

  Ok(())
}

#[test]
fn a_channel_end_is_refused_its_own_queue_and_stays_with_its_holder() -> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let before = books(&mut kernel, root, root_job)?;
  let (near, far) = kernel.create_channel(root)?;

  assert_eq!(
    kernel.write(root, near, b"", &[far]),
    Err(HandleError::SelfTransfer)
  );
  kernel.close(root, near)?;
  kernel.close(root, far)?;

  assert_eq!(books(&mut kernel, root, root_job), Ok(before));

  Ok(())
}

#[test]
fn the_second_of_two_ends_is_refused_the_queue_that_is_already_on_its_way_into_it()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let before = books(&mut kernel, root, root_job)?;
  let (one, one_far) = kernel.create_channel(root)?;
  let (other, other_far) = kernel.create_channel(root)?;

  kernel.write(root, one, b"", &[other_far])?;

  assert_eq!(
    kernel.write(root, other, b"", &[one_far]),
    Err(HandleError::SelfTransfer)
  );
  kernel.close(root, one)?;
  kernel.close(root, other)?;
  kernel.close(root, one_far)?;

  assert_eq!(books(&mut kernel, root, root_job), Ok(before));

  Ok(())
}

#[test]
fn a_job_cannot_tie_a_channel_in_a_knot_and_gives_its_share_back_when_it_is_killed()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let before = books(&mut kernel, root, root_job)?;
  let job = kernel.create_job(root, root_job, SHARE)?;
  let hostile = kernel.create_process(root, job)?;
  let (near, far) = kernel.create_channel(hostile.id)?;

  assert_eq!(
    kernel.write(hostile.id, near, b"", &[far]),
    Err(HandleError::SelfTransfer)
  );
  kernel.kill(root, job)?;
  kernel.close(root, job)?;
  kernel.close(root, hostile.process)?;
  kernel.close(root, hostile.channel)?;

  assert_eq!(books(&mut kernel, root, root_job), Ok(before));

  Ok(())
}

#[test]
fn a_message_that_says_nothing_is_still_read_after_the_job_that_wrote_it_is_gone()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let job = kernel.create_job(root, root_job, SHARE)?;
  let writer = kernel.create_process(root, job)?;
  let out = bootstrap(&mut kernel, writer.id)?;

  kernel.write(writer.id, out, b"", &[])?;
  kernel.exit(writer.id)?;
  kernel.close(root, job)?;

  assert_eq!(
    kernel.read(root, writer.channel),
    Ok(Message {
      bytes: Vec::new(),
      handles: Default::default()
    })
  );

  Ok(())
}

#[test]
fn a_channel_holding_nothing_from_a_job_that_is_gone_can_still_be_closed()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let before = books(&mut kernel, root, root_job)?;
  let job = kernel.create_job(root, root_job, SHARE)?;
  let writer = kernel.create_process(root, job)?;
  let out = bootstrap(&mut kernel, writer.id)?;

  kernel.write(writer.id, out, b"", &[])?;
  kernel.exit(writer.id)?;
  kernel.close(root, job)?;
  kernel.close(root, writer.process)?;

  assert_eq!(kernel.close(root, writer.channel), Ok(()));
  assert_eq!(books(&mut kernel, root, root_job), Ok(before));

  Ok(())
}

fn joined(
  kernel: &mut Kernel,
  root: ProcessId,
  pair: (CreatedProcess, CreatedProcess),
) -> Result<(HandleValue, HandleValue), HandleError> {
  let (inside, outside) = pair;
  let (near, far) = kernel.create_channel(root)?;

  kernel.write(root, inside.channel, b"", &[near])?;
  kernel.write(root, outside.channel, b"", &[far])?;

  let inside_only = bootstrap(kernel, inside.id)?;
  let outside_only = bootstrap(kernel, outside.id)?;
  let theirs = received(kernel, inside.id, inside_only)?;
  let ours = received(kernel, outside.id, outside_only)?;

  Ok((theirs, ours))
}

#[test]
fn a_job_killed_with_messages_in_flight_both_ways_leaves_what_it_sent_and_nothing_else()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let before = books(&mut kernel, root, root_job)?;
  let killed = kernel.create_job(root, root_job, SHARE)?;
  let beside = kernel.create_job(root, root_job, SHARE)?;
  let inside = kernel.create_process(root, killed)?;
  let outside = kernel.create_process(root, beside)?;
  let (theirs, ours) = joined(&mut kernel, root, (inside, outside))?;
  let leaving = kernel.create_memory(inside.id, PAGE)?;
  let arriving = kernel.create_memory(outside.id, PAGE)?;
  let quiet = kernel.budget(root, beside)?;

  kernel.write(inside.id, theirs, b"out", &[leaving])?;
  kernel.write(outside.id, ours, b"in", &[arriving])?;
  kernel.kill(root, killed)?;
  kernel.kill(root, killed)?;
  kernel.kill(root, inside.process)?;

  assert_eq!(kernel.handles(inside.id), Err(HandleError::ProcessNotFound));
  assert_eq!(
    kernel.create_process(root, killed),
    Err(HandleError::BadState)
  );
  assert_eq!(
    kernel.write(outside.id, ours, b"late", &[]),
    Err(HandleError::PeerClosed)
  );
  let Ok(lighter) = minus(
    quiet,
    HANDLE_COST
      .0
      .saturating_add(OBJECT_COST.0)
      .saturating_add(PAGE.0),
  );
  assert_eq!(kernel.budget(root, beside), Ok(lighter));
  let owed = OBJECT_COST
    .0
    .saturating_add(PAGE.0)
    .saturating_add(HANDLE_COST.0)
    .saturating_add(3)
    .saturating_add(MESSAGE_COST.0);
  assert_shared_charge(&mut kernel, root, killed, Bytes(owed))?;

  let survived = received(&mut kernel, outside.id, ours)?;
  assert_eq!(kernel.read(outside.id, ours), Err(HandleError::PeerClosed));
  assert_shared_charge(&mut kernel, root, killed, Bytes(OBJECT_COST.0.saturating_add(PAGE.0)))?;
  kernel.close(outside.id, survived)?;
  assert_shared_charge(&mut kernel, root, killed, Bytes(0))?;

  kernel.kill(root, beside)?;
  kernel.close(root, killed)?;
  kernel.close(root, beside)?;
  kernel.close(root, inside.process)?;
  kernel.close(root, inside.channel)?;
  kernel.close(root, outside.process)?;
  kernel.close(root, outside.channel)?;

  assert_eq!(books(&mut kernel, root, root_job), Ok(before));

  Ok(())
}

fn assert_shared_charge(
  kernel: &mut Kernel,
  by: ProcessId,
  job: HandleValue,
  charged: Bytes,
) -> Result<(), HandleError> {
  let actual = kernel.budget(by, job)?;

  assert_eq!(actual, Budget { share: SHARE, charged });

  Ok(())
}

#[test]
fn a_killed_job_whose_handle_arrives_later_in_a_message_is_still_a_killed_job()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let before = books(&mut kernel, root, root_job)?;
  let job = kernel.create_job(root, root_job, SHARE)?;
  let managing = kernel.handle_rights(root, job)?;
  let lent = kernel.duplicate(root, job, managing)?;
  let inside = kernel.create_process(root, job)?;
  let only = bootstrap(&mut kernel, inside.id)?;

  kernel.write(root, inside.channel, b"", &[lent])?;
  let theirs = received(&mut kernel, inside.id, only)?;
  kernel.write(inside.id, only, b"yours", &[theirs])?;
  kernel.kill(root, job)?;
  kernel.close(root, job)?;

  let late = received(&mut kernel, root, inside.channel)?;

  assert_eq!(
    kernel.budget(root, late),
    Ok(Budget {
      share: SHARE,
      charged: Bytes(0)
    })
  );
  assert_eq!(
    kernel.create_process(root, late),
    Err(HandleError::BadState)
  );
  assert_eq!(
    kernel.create_job(root, late, PAGE),
    Err(HandleError::BadState)
  );
  kernel.kill(root, late)?;
  kernel.close(root, late)?;
  kernel.close(root, inside.process)?;
  kernel.close(root, inside.channel)?;

  assert_eq!(books(&mut kernel, root, root_job), Ok(before));

  Ok(())
}

#[test]
fn a_job_whose_only_handle_is_waiting_for_a_process_inside_it_goes_with_that_process()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let before = books(&mut kernel, root, root_job)?;
  let job = kernel.create_job(root, root_job, SHARE)?;
  let inside = kernel.create_process(root, job)?;

  kernel.write(root, inside.channel, b"take it", &[job])?;
  kernel.kill(root, inside.process)?;
  kernel.kill(root, inside.process)?;
  kernel.close(root, inside.process)?;
  kernel.close(root, inside.channel)?;

  assert_eq!(books(&mut kernel, root, root_job), Ok(before));

  Ok(())
}

#[test]
fn a_job_with_no_share_can_be_made_and_can_hold_nothing() -> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let before = books(&mut kernel, root, root_job)?;
  let empty = kernel.create_job(root, root_job, Bytes(0))?;
  let made = books(&mut kernel, root, root_job)?;

  assert_eq!(
    kernel.create_process(root, empty),
    Err(HandleError::OverBudget)
  );
  assert_eq!(
    kernel.create_job(root, empty, Bytes(1)),
    Err(HandleError::OverBudget)
  );
  assert_eq!(books(&mut kernel, root, root_job), Ok(made));
  assert_eq!(
    kernel.budget(root, empty),
    Ok(Budget {
      share: Bytes(0),
      charged: Bytes(0)
    })
  );

  let under = kernel.create_job(root, empty, Bytes(0))?;

  assert_eq!(
    kernel.create_process(root, under),
    Err(HandleError::OverBudget)
  );
  kernel.kill(root, empty)?;
  kernel.close(root, under)?;
  kernel.close(root, empty)?;

  assert_eq!(books(&mut kernel, root, root_job), Ok(before));

  Ok(())
}

#[test]
fn a_child_can_be_given_everything_its_parent_has_left_and_not_a_byte_more()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let before = kernel.budget(root, root_job)?;
  let making = OBJECT_COST.0.saturating_add(HANDLE_COST.0);
  let left = before
    .share
    .0
    .saturating_sub(before.charged.0)
    .saturating_sub(making);

  assert_eq!(
    kernel.create_job(root, root_job, Bytes(left.saturating_add(1))),
    Err(HandleError::OverBudget)
  );
  assert_eq!(kernel.budget(root, root_job), Ok(before));

  let everything = kernel.create_job(root, root_job, Bytes(left))?;

  assert_eq!(
    kernel.budget(root, root_job),
    Ok(Budget {
      share: before.share,
      charged: before.share
    })
  );
  assert_eq!(
    kernel.create_memory(root, Bytes(0)),
    Err(HandleError::OverBudget)
  );
  assert_eq!(kernel.create_channel(root), Err(HandleError::OverBudget));
  assert_eq!(
    kernel.create_process(root, everything),
    Err(HandleError::OverBudget)
  );
  assert_eq!(
    kernel.budget(root, root_job),
    Ok(Budget {
      share: before.share,
      charged: before.share
    })
  );

  kernel.close(root, everything)?;

  assert_eq!(kernel.budget(root, root_job), Ok(before));

  Ok(())
}

#[test]
fn a_child_given_its_parents_whole_share_leaves_the_parent_room_for_nothing()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let before = books(&mut kernel, root, root_job)?;
  let above = kernel.create_job(root, root_job, SHARE)?;
  let under = kernel.create_job(root, above, SHARE)?;

  assert_eq!(
    kernel.budget(root, above),
    Ok(Budget {
      share: SHARE,
      charged: SHARE
    })
  );
  assert_eq!(
    kernel.create_process(root, above),
    Err(HandleError::OverBudget)
  );
  assert_eq!(
    kernel.create_job(root, above, Bytes(1)),
    Err(HandleError::OverBudget)
  );

  let started = kernel.create_process(root, under)?;

  kernel.kill(root, above)?;
  assert_eq!(
    kernel.handles(started.id),
    Err(HandleError::ProcessNotFound)
  );
  kernel.close(root, above)?;
  kernel.close(root, under)?;
  kernel.close(root, started.process)?;
  kernel.close(root, started.channel)?;

  assert_eq!(books(&mut kernel, root, root_job), Ok(before));

  Ok(())
}

#[test]
fn a_reader_that_cannot_afford_what_a_message_carries_leaves_it_whole_and_its_writer_paying()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let writers = kernel.create_job(root, root_job, SHARE)?;
  let cramped = kernel.create_job(root, root_job, HANDLE_COST)?;
  let writer = kernel.create_process(root, writers)?;
  let only = bootstrap(&mut kernel, writer.id)?;

  kernel.write(root, writer.channel, b"", &[cramped])?;

  let readers = received(&mut kernel, writer.id, only)?;
  let reader = kernel.create_process(writer.id, readers)?;
  let into = bootstrap(&mut kernel, reader.id)?;
  let one = kernel.create_memory(writer.id, PAGE)?;
  let other = kernel.create_memory(writer.id, PAGE)?;

  kernel.write(writer.id, reader.channel, b"eight by", &[one, other])?;

  let paying = kernel.budget(root, writers)?;

  assert_eq!(kernel.read(reader.id, into), Err(HandleError::OverBudget));
  assert_eq!(kernel.read(reader.id, into), Err(HandleError::OverBudget));
  assert_eq!(kernel.handles(reader.id), Ok(vec![into]));
  assert_eq!(kernel.budget(root, writers), Ok(paying));
  assert_eq!(
    kernel.budget(writer.id, readers),
    Ok(Budget {
      share: HANDLE_COST,
      charged: HANDLE_COST
    })
  );

  kernel.kill(writer.id, reader.process)?;

  let each = HANDLE_COST
    .0
    .saturating_add(OBJECT_COST.0)
    .saturating_add(PAGE.0);
  let dropped = each
    .saturating_mul(2)
    .saturating_add(OBJECT_COST.0)
    .saturating_add(8)
    .saturating_add(MESSAGE_COST.0);
  let Ok(lighter) = minus(paying, dropped);

  assert_eq!(kernel.budget(root, writers), Ok(lighter));
  assert_eq!(
    kernel.budget(writer.id, readers),
    Ok(Budget {
      share: HANDLE_COST,
      charged: Bytes(0)
    })
  );

  Ok(())
}

#[test]
fn a_dead_job_is_kept_for_as_long_as_another_job_holds_memory_it_paid_for()
-> Result<(), HandleError> {
  let Boot {
    mut kernel,
    root,
    root_job,
    ..
  } = Kernel::boot(MACHINE)?;
  let before = books(&mut kernel, root, root_job)?;
  let above = kernel.create_job(root, root_job, SHARE)?;
  let under = kernel.create_job(root, above, QUARTER)?;
  let maker = kernel.create_process(root, under)?;
  let only = bootstrap(&mut kernel, maker.id)?;
  let made = kernel.create_memory(maker.id, PAGE)?;

  kernel.write(maker.id, only, b"keep", &[made])?;
  let ours = received(&mut kernel, root, maker.channel)?;
  kernel.kill(root, above)?;
  kernel.close(root, maker.process)?;
  kernel.close(root, maker.channel)?;

  let owed = OBJECT_COST.0.saturating_add(PAGE.0);

  assert_eq!(
    kernel.budget(root, under),
    Ok(Budget {
      share: QUARTER,
      charged: Bytes(owed)
    })
  );
  kernel.close(root, under)?;
  kernel.close(root, above)?;
  assert_eq!(kernel.object_count(), Ok(before.objects.saturating_add(3)));
  kernel.handle_rights(root, ours)?;
  kernel.close(root, ours)?;

  assert_eq!(books(&mut kernel, root, root_job), Ok(before));

  Ok(())
}
