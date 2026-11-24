#![forbid(unsafe_code)]
#[cfg(feature = "bitdepth_8")]
use crate::include::common::bitdepth::BitDepth8;
#[cfg(feature = "bitdepth_16")]
use crate::include::common::bitdepth::BitDepth16;
use crate::include::common::intops::iclip;
use crate::include::dav1d::dav1d::Rav1dEventFlags;
use crate::include::dav1d::headers::Rav1dPixelLayout;
use crate::include::dav1d::picture::Rav1dPicture;
use crate::src::decode::rav1d_decode_frame_exit;
use crate::src::decode::rav1d_decode_frame_init;
use crate::src::decode::rav1d_decode_frame_init_cdf;
use crate::src::decode::rav1d_decode_tile_sbrow;
use crate::src::decode::rav1d_publish_tile_cdf;
use crate::src::disjoint_mut::TryResizableWith;
use crate::src::error::Rav1dError;
use crate::src::error::Rav1dError::ENOMEM;
use crate::src::error::Rav1dError::{EAGAIN, EGeneric, EINVAL};
use crate::src::error::Rav1dResult;
use crate::src::fg_apply::rav1d_apply_grain_row;
use crate::src::fg_apply::rav1d_prep_grain;
use crate::src::filmgrain::FG_BLOCK_SIZE;
use crate::src::internal::DecoderStopState;
use crate::src::internal::DelayedGrainStage;
use crate::src::internal::FrameVisibilityPolicy;
use crate::src::internal::Grain;
use crate::src::internal::Rav1dBitDepthDSPContext;
use crate::src::internal::Rav1dContext;
use crate::src::internal::Rav1dFrameContext;
use crate::src::internal::Rav1dFrameContextTaskThread;
use crate::src::internal::Rav1dFrameData;
use crate::src::internal::Rav1dState;
use crate::src::internal::Rav1dTask;
use crate::src::internal::Rav1dTaskContext;
use crate::src::internal::TaskType;
use crate::src::internal::{Rav1dTaskContextTaskThread, Rav1dTileState};
use crate::src::internal::{TaskThreadData, TaskThreadDataDelayedFg};
use crate::src::iter::wrapping_iter;
use crate::src::relaxed_atomic::RelaxedAtomic;
use atomig::Atom;
use atomig::Atomic;
use console_core_synchronization::nonpoison::{
  Mutex, MutexGuard, RwLock, RwLockReadGuard, unpoison, wait_while,
};
use std::cmp;
use std::ffi::c_int;
use std::mem;
use std::num::NonZeroU32;
use std::ops::Add;
use std::ops::AddAssign;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
#[cfg(test)]
use std::thread;

pub const FRAME_ERROR: u32 = u32::MAX - 1;
pub const TILE_ERROR: i32 = i32::MAX - 1;

fn publish_frame_completion(
  context: &Rav1dContext,
  frame: &Rav1dFrameContext,
  outcome: Rav1dResult,
) -> Result<(), console_core_never::Never> {
  let _completion_stored_in_frame = rav1d_decode_frame_exit(context, frame, outcome);
  frame.task_thread.cond.notify_one();
  Ok(())
}

fn worker_frame_outcome(code: c_int) -> Rav1dResult {
  match code {
    0 => Ok(()),
    1 => Err(EINVAL),
    _ => Err(ENOMEM),
  }
}

pub(crate) fn advance_completed_frame(
  control: &TaskThreadData,
  count: u32,
) -> Result<(), console_core_never::Never> {
  let first = control.first.load(Ordering::SeqCst);

  match first.wrapping_add(1).cmp(&count) {
    std::cmp::Ordering::Less => {
      let _previous_first = control.first.fetch_add(1, Ordering::SeqCst);
    }
    std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => {
      control.first.store(0, Ordering::SeqCst)
    }
  }

  let expected = NonZeroU32::new(first.wrapping_add(1));
  let _reset_superseded_by_another_task =
    control
      .reset_task_cur
      .compare_exchange(expected, None, Ordering::SeqCst, Ordering::SeqCst);
  let Ok(current) = control.cur.get();

  match (current > 0 && current < count).then_some(()) {
    Some(()) => {
      let Ok(()) = control.cur.set(current.wrapping_sub(1));
    }
    None => {}
  }

  Ok(())
}

pub(crate) struct FrameReservation<'a> {
  pub(crate) frame: &'a Rav1dFrameContext,
  pub(crate) index: u32,
  pub(crate) scheduler: MutexGuard<'a, ()>,
}

fn wait_for_frame_slot<'a>(
  decoder: &'a Rav1dContext,
  frame: &Rav1dFrameContext,
) -> Rav1dResult<MutexGuard<'a, ()>> {
  let control = &decoder.task_thread;
  let scheduler = control.lock.lock();
  let scheduler_2 = wait_while(&frame.task_thread.cond, scheduler, |_schedule| {
    !frame.task_thread.finished.load(Ordering::SeqCst) && !control.panicked.load(Ordering::SeqCst)
  });

  match ::core::primitive::bool::then_some(frame.task_thread.finished.load(Ordering::SeqCst), ())
  {
    ::core::option::Option::Some(()) => Ok(scheduler_2),
    ::core::option::Option::None => Err(EGeneric),
  }
}

enum DelayedFrameOutput {
  Error(Rav1dError),
  Empty,
  Discard,
  Display,
}

fn retire_delayed_output(
  decoder: &Rav1dContext,
  state: &mut Rav1dState,
  frame: &Rav1dFrameContext,
  slot: u32,
  frame_count: NonZeroU32,
) -> Rav1dResult {
  let Ok(index) = console_core_number_conversion::index(slot);
  let delayed = state
    .frame_thread
    .out_delayed
    .get_mut(index)
    .ok_or(EINVAL)?;
  let mut result = frame.task_thread.retval.try_lock().ok_or(EAGAIN)?;
  let output = match (*result, &delayed.p.data) {
    (Some(error), _) => DelayedFrameOutput::Error(error),
    (None, None) => DelayedFrameOutput::Empty,
    (None, Some(_)) => {
      let Ok(counters) = delayed.progress();
      let [_, final_progress] = counters.ok_or(EINVAL)?;
      use crate::src::picture::FramePresentation;
      let visible = matches!(
        delayed.presentation,
        FramePresentation::Visible | FramePresentation::VisibleShowable
      ) || decoder.frame_visibility == FrameVisibilityPolicy::IncludeInvisible;

      match (visible && final_progress.load(Ordering::Relaxed) != FRAME_ERROR).then_some(()) {
        Some(()) => DelayedFrameOutput::Display,
        None => DelayedFrameOutput::Discard,
      }
    }
  };
  let had_pending_output =
    delayed.p.data.is_some() || frame.task_thread.error.load(Ordering::SeqCst) != 0;

  match ::core::primitive::bool::then_some(had_pending_output, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(()) = advance_completed_frame(&decoder.task_thread, frame_count.get());
    }
    ::core::option::Option::None => {}
  }

  match output {
    DelayedFrameOutput::Error(error) => {
      *result = None;
      state.cached_error = Some(error);
      state.cached_error_props = delayed.p.m.clone();
      drop(mem::take(delayed));
    }
    DelayedFrameOutput::Display => {
      let completed = mem::take(delayed);
      state.event_flags |= Rav1dEventFlags::from(completed.flags);
      state.out = completed;
    }
    DelayedFrameOutput::Discard => drop(mem::take(delayed)),
    DelayedFrameOutput::Empty => {}
  }

  Ok(())
}

pub(crate) fn reserve_delayed_frame<'a>(
  decoder: &'a Rav1dContext,
  state: &mut Rav1dState,
) -> Rav1dResult<FrameReservation<'a>> {
  let next = state.frame_thread.next;
  let Ok(index) = console_core_number_conversion::index(next);
  let frame = decoder.fc.get(index).ok_or(EINVAL)?;
  let count = u32::try_from(decoder.fc.len()).map_err(|_too_many_frames| EINVAL)?;
  let frame_count = NonZeroU32::new(count).ok_or(EINVAL)?;
  let following = next
    .wrapping_add(1)
    .checked_rem(frame_count.get())
    .ok_or(EINVAL)?;
  let scheduler = wait_for_frame_slot(decoder, frame)?;
  retire_delayed_output(decoder, state, frame, next, frame_count)?;
  state.frame_thread.next = following;
  Ok(FrameReservation {
    frame,
    index: next,
    scheduler,
  })
}

#[derive(PartialEq, Eq)]
enum ScanReset {
  Rewound,
  Unchanged,
}

struct ResetFramePosition {
  slot: u32,
  frame_count: u32,
}

fn clear_scan_positions(
  c: &Rav1dContext,
  start: u64,
) -> Result<ScanReset, console_core_never::Never> {
  let Ok(frames) = wrapping_iter(c.fc.iter(), start);

  for frame in frames {
    let Ok(()) = frame.task_thread.tasks.cur_prev.set(Rav1dTaskIndex::None);
  }

  Ok(ScanReset::Rewound)
}

#[inline]
fn reset_task_cur(
  c: &Rav1dContext,
  ttd: &TaskThreadData,
  requested: Option<u32>,
) -> Result<ScanReset, console_core_never::Never> {
  let first = ttd.first.load(Ordering::SeqCst);
  let pending = ttd
    .reset_task_cur
    .swap(None, Ordering::SeqCst)
    .map(|encoded| encoded.get().wrapping_sub(1))
    .filter(|&frame| frame >= first);
  let Ok(current) = ttd.cur.get();
  let Ok(first_index) = console_core_number_conversion::index(first);
  let first_frame = match c.fc.get(first_index) {
    Some(frame) => frame,
    None => return Ok(ScanReset::Unchanged),
  };
  let Ok(previous) = first_frame.task_thread.tasks.cur_prev.get();

  match (current, previous) {
    (0, Rav1dTaskIndex::None) => return Ok(ScanReset::Unchanged),
    _ => {}
  }

  let Ok(frame_count) = console_core_number_conversion::wrapped::<_, u32>(c.fc.len());
  let current_frame = first.wrapping_add(current);
  let requested_frame = match requested {
    Some(frame) => match frame.cmp(&first) {
      cmp::Ordering::Less => frame.wrapping_add(frame_count),
      cmp::Ordering::Equal | cmp::Ordering::Greater => frame,
    },
    None => match pending {
      Some(frame) => match frame.cmp(&current_frame) {
        cmp::Ordering::Less | cmp::Ordering::Equal => {
          let offset = frame.wrapping_sub(first);
          let Ok(()) = ttd.cur.set(offset);
          let start = u64::from(first).wrapping_add(u64::from(offset));
          return clear_scan_positions(c, start);
        }
        cmp::Ordering::Greater => return Ok(ScanReset::Unchanged),
      },
      None => return Ok(ScanReset::Unchanged),
    },
  };
  let target = match pending {
    Some(frame) => frame.min(requested_frame),
    None => requested_frame,
  };

  match (current < frame_count && current_frame < target).then_some(()) {
    Some(()) => return Ok(ScanReset::Unchanged),
    None => {}
  }

  let mut offset = target.wrapping_sub(first);
  let Ok(()) = ttd.cur.set(offset);
  let frame_position = match first.wrapping_add(offset).checked_rem(frame_count) {
    Some(position) => position,
    None => return Ok(ScanReset::Unchanged),
  };
  let Ok(frames) = wrapping_iter(c.fc.iter(), u64::from(frame_position));

  for (_, frame) in (offset..frame_count).zip(frames) {
    match frame.task_thread.tasks.head.load(Ordering::SeqCst) {
      Rav1dTaskIndex::None => {
        offset = offset.wrapping_add(1);
        let Ok(()) = ttd.cur.set(offset);
      }
      _ => break,
    }
  }

  let scan_start = u64::from(first).wrapping_add(u64::from(offset));
  clear_scan_positions(c, scan_start)
}

#[inline]
fn reset_task_cur_async(
  ttd: &TaskThreadData,
  ResetFramePosition { slot, frame_count }: ResetFramePosition,
) -> Result<(), console_core_never::Never> {
  let first = ttd.first.load(Ordering::SeqCst);
  let frame = match slot.cmp(&first) {
    cmp::Ordering::Less => slot.wrapping_add(frame_count),
    cmp::Ordering::Equal | cmp::Ordering::Greater => slot,
  };
  let encoded = match NonZeroU32::new(frame.wrapping_add(1)) {
    Some(index) => index,
    None => return Ok(()),
  };
  let active =
    match ttd
      .reset_task_cur
      .fetch_update(
        Ordering::SeqCst,
        Ordering::SeqCst,
        |previous| match previous {
          Some(earlier) => match earlier.cmp(&encoded) {
            cmp::Ordering::Equal | cmp::Ordering::Less => None,
            cmp::Ordering::Greater => Some(Some(encoded)),
          },
          None => Some(Some(encoded)),
        },
      ) {
      Ok(_) => encoded,
      Err(Some(earlier)) => earlier,
      Err(None) => return Ok(()),
    };

  match (active.get().wrapping_sub(1) == first && ttd.first.load(Ordering::SeqCst) != first)
    .then_some(())
  {
    Some(()) => {
      let _superseded = ttd.reset_task_cur.compare_exchange(
        Some(active),
        None,
        Ordering::SeqCst,
        Ordering::SeqCst,
      );
    }
    None => {}
  }

  Ok(())
}

#[derive(Default)]
pub struct Rav1dTasks {
  tasks: RwLock<Vec<Rav1dTask>>,
  pending_tasks: Mutex<Vec<Rav1dTask>>,
  pending_tasks_merge: AtomicBool,

  pub head: Atomic<Rav1dTaskIndex>,

  pub cur_prev: RelaxedAtomic<Rav1dTaskIndex>,
}

struct TaskRef<'a> {
  tasks: RwLockReadGuard<'a, Vec<Rav1dTask>>,
  position: Rav1dTaskIndex,
}

fn queue_entry(tasks: &[Rav1dTask], position: Rav1dTaskIndex) -> Rav1dResult<&Rav1dTask> {
  let encoded = position.0.ok_or(EINVAL)?;
  let Ok(index) = console_core_number_conversion::index(encoded.get().wrapping_sub(1));
  tasks.get(index).ok_or(EINVAL)
}

impl TaskRef<'_> {
  fn entry(&self) -> Rav1dResult<&Rav1dTask> {
    queue_entry(&self.tasks, self.position)
  }
}

struct QueueOrder {
  row: i32,
  kind: TaskType,
  tile: u32,
}

impl From<&Rav1dTask> for QueueOrder {
  fn from(task: &Rav1dTask) -> Self {
    Self {
      row: task.sby,
      kind: task.type_0,
      tile: task.tile_idx,
    }
  }
}

impl QueueOrder {
  fn compare_insertion(
    &self,
    existing: &Self,
  ) -> Result<cmp::Ordering, console_core_never::Never> {
    let priority = match (self.kind, existing.kind) {
      (incoming, TaskType::TileEntropy) => match incoming.cmp(&TaskType::TileEntropy) {
        cmp::Ordering::Greater => cmp::Ordering::Greater,
        cmp::Ordering::Equal | cmp::Ordering::Less => self.row.cmp(&existing.row),
      },
      (TaskType::TileEntropy, _) => cmp::Ordering::Less,
      _ => (self.row, self.kind).cmp(&(existing.row, existing.kind)),
    };

    match priority {
      cmp::Ordering::Equal => {
        assert!(matches!(
          self.kind,
          TaskType::TileEntropy | TaskType::TileReconstruction
        ));
        assert_eq!(self.kind, existing.kind);
        assert_ne!(self.tile, existing.tile);
        Ok(self.tile.cmp(&existing.tile))
      }
      cmp::Ordering::Less | cmp::Ordering::Greater => Ok(priority),
    }
  }
}

struct TaskInsertion {
  first: Rav1dTaskIndex,
  last: Rav1dTaskIndex,
  previous: Rav1dTaskIndex,
  following: Rav1dTaskIndex,
}

#[derive(Clone, Copy)]
enum QueueWake {
  Notify,
  KeepParked,
}

fn stage_pending_tasks(
  tasks: &mut Vec<Rav1dTask>,
  pending: &mut Vec<Rav1dTask>,
  head: Rav1dTaskIndex,
) -> Rav1dResult<core::ops::Range<u32>> {
  let Ok(current) = console_core_number_conversion::fixed_index(tasks.len());
  let retained = match head {
    Rav1dTaskIndex::None => 0,
    _ => current,
  };
  let Ok(additional) = console_core_number_conversion::fixed_index(pending.len());
  let final_length = retained.checked_add(additional).ok_or(ENOMEM)?;
  let start = u32::try_from(retained).map_err(|_queue_capacity| ENOMEM)?;
  let end = u32::try_from(final_length).map_err(|_queue_capacity| ENOMEM)?;
  let Ok(extra) = console_core_number_conversion::index(final_length.saturating_sub(current));
  tasks
    .try_reserve(extra.min(pending.len()))
    .map_err(|_allocation| ENOMEM)?;
  let Ok(keep) = console_core_number_conversion::index(retained);
  tasks.truncate(keep);
  tasks.append(pending);
  Ok(start..end)
}

impl Rav1dTasks {
  fn insert_tasks_between(
    &self,
    c: &Rav1dContext,
    insertion: TaskInsertion,
    wake: QueueWake,
  ) -> Rav1dResult {
    let ttd = &c.task_thread;

    match c.flush.load(Ordering::SeqCst).then_some(()) {
      Some(()) => return Ok(()),
      None => {}
    }

    let TaskInsertion {
      first,
      last,
      previous,
      following,
    } = insertion;
    let tasks = self.tasks.try_read().ok_or(EINVAL)?;
    let origin = queue_entry(&tasks, first)?;
    let tail = queue_entry(&tasks, last)?;

    match previous {
      Rav1dTaskIndex::None => self.head.store(first, Ordering::SeqCst),
      _ => {
        let predecessor = queue_entry(&tasks, previous)?;
        let Ok(next) = predecessor.next();

        match next.cmp(&following) {
          cmp::Ordering::Equal => {}
          cmp::Ordering::Less | cmp::Ordering::Greater => return Err(EINVAL),
        }

        let Ok(()) = predecessor.set_next(first);
      }
    }

    let Ok(()) = tail.set_next(following);
    let frame = origin.frame_idx;
    drop(tasks);
    let Ok(_reset) = reset_task_cur(c, ttd, Some(frame));

    match wake {
      QueueWake::KeepParked => {}
      QueueWake::Notify => match ttd.cond_signaled.fetch_or(1, Ordering::SeqCst) {
        0 => ttd.cond.notify_one(),
        _ => {}
      },
    }

    Ok(())
  }

  fn insert_tasks(
    &self,
    c: &Rav1dContext,
    first: Rav1dTaskIndex,
    last: Rav1dTaskIndex,
    wake: QueueWake,
  ) -> Rav1dResult {
    let tasks = self.tasks.try_read().ok_or(EINVAL)?;
    let new_task = queue_entry(&tasks, first)?;
    let incoming = QueueOrder::from(new_task);
    let Ok(task_count) = console_core_number_conversion::fixed_index(tasks.len());
    let mut previous = Rav1dTaskIndex::None;
    let mut following = self.head.load(Ordering::SeqCst);

    for _ in 0..=task_count {
      let order = match following {
        Rav1dTaskIndex::None => cmp::Ordering::Less,
        _ => {
          let task = queue_entry(&tasks, following)?;
          let encounter = QueueOrder::from(task);
          let Ok(order) = incoming.compare_insertion(&encounter);
          order
        }
      };

      match order {
        cmp::Ordering::Less | cmp::Ordering::Equal => {
          let insertion = TaskInsertion {
            first,
            last,
            previous,
            following,
          };
          drop(tasks);
          return self.insert_tasks_between(c, insertion, wake);
        }
        cmp::Ordering::Greater => {
          let task = queue_entry(&tasks, following)?;
          let Ok(next) = task.next();
          previous = following;
          following = next;
        }
      }
    }

    Err(EINVAL)
  }

  fn push(&self, task: Rav1dTask) -> Rav1dResult<Rav1dTaskIndex> {
    let mut tasks = self.tasks.try_write().ok_or(EINVAL)?;
    let count = u32::try_from(tasks.len()).map_err(|_queue_capacity| ENOMEM)?;
    let after = count.checked_add(1).ok_or(ENOMEM)?;
    let position = NonZeroU32::new(after).ok_or(ENOMEM)?;
    tasks.try_reserve(1).map_err(|_allocation| ENOMEM)?;
    tasks.push(task);
    Ok(Rav1dTaskIndex(Some(position)))
  }

  pub fn clear(&self) -> core::result::Result<(), console_core_never::Never> {
    {
      self.tasks.write().clear();
      self.pending_tasks.lock().clear();
      self.pending_tasks_merge.store(false, Ordering::SeqCst);
      self.head.store(Default::default(), Ordering::Relaxed);
      let Ok(()) = self.cur_prev.set(Default::default());
    };

    Ok(())
  }

  pub fn remove(
    &self,
    position: Rav1dTaskIndex,
    previous: Rav1dTaskIndex,
  ) -> Rav1dResult<Option<Rav1dTask>> {
    let tasks = self.tasks.try_read().ok_or(EINVAL)?;
    let task = queue_entry(&tasks, position)?;
    let Ok(next) = task.next();
    let result = match previous {
      Rav1dTaskIndex::None => {
        self
          .head
          .compare_exchange(position, next, Ordering::SeqCst, Ordering::SeqCst)
      }
      _ => {
        let predecessor = queue_entry(&tasks, previous)?;
        predecessor
          .next
          .compare_exchange(position, next, Ordering::SeqCst, Ordering::SeqCst)
      }
    };

    match result {
      Err(_queue_changed) => Ok(None),
      Ok(_detached) => {
        let Ok(()) = task.set_next(Rav1dTaskIndex::None);
        let Ok(detached) = task.without_next();
        Ok(Some(detached))
      }
    }
  }

  #[inline]
  fn read(&self, position: Rav1dTaskIndex) -> Rav1dResult<TaskRef<'_>> {
    let tasks = self.tasks.try_read().ok_or(EINVAL)?;
    Ok(TaskRef { tasks, position })
  }

  #[inline]
  fn add_pending(&self, task: Rav1dTask) -> core::result::Result<(), console_core_never::Never> {
    {
      self.pending_tasks.lock().push(task);
      self.pending_tasks_merge.store(true, Ordering::SeqCst);
    };

    Ok(())
  }

  #[inline]
  fn merge_pending_frame(&self, c: &Rav1dContext) -> Rav1dResult<PendingMerge> {
    let merge = PendingMerge::from(self.pending_tasks_merge.swap(false, Ordering::SeqCst));

    match merge {
      PendingMerge::Unchanged => {}
      PendingMerge::Merged => {
        let mut pending = self.pending_tasks.lock();
        let mut tasks = match self.tasks.try_write() {
          Some(guard) => guard,
          None => {
            self.pending_tasks_merge.store(true, Ordering::SeqCst);
            return Err(EINVAL);
          }
        };
        let head = self.head.load(Ordering::Relaxed);
        let ready = stage_pending_tasks(&mut tasks, &mut pending, head).inspect_err(
          |_failed_to_stage| self.pending_tasks_merge.store(true, Ordering::SeqCst),
        )?;
        drop(tasks);

        for position in ready {
          let encoded = NonZeroU32::new(position.wrapping_add(1)).ok_or(ENOMEM)?;
          let index = Rav1dTaskIndex(Some(encoded));
          self.insert_tasks(c, index, index, QueueWake::KeepParked)?;
        }
      }
    }

    Ok(merge)
  }
}

impl Rav1dFrameContextTaskThread {
  fn insert_task(
    &self,
    c: &Rav1dContext,
    task: Rav1dTask,
    wake: QueueWake,
  ) -> Rav1dResult<Rav1dTaskIndex> {
    let index = self.tasks.push(task)?;
    self.tasks.insert_tasks(c, index, index, wake)?;
    Ok(index)
  }
}

#[derive(Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Debug, Atom)]
#[repr(transparent)]
pub struct Rav1dTaskIndex(Option<NonZeroU32>);

impl Rav1dTaskIndex {
  pub const None: Self = Self(None);
}

impl Add<u32> for Rav1dTaskIndex {
  type Output = Self;

  fn add(self, rhs: u32) -> Self::Output {
    Self(
      self
        .0
        .and_then(|i| NonZeroU32::new(u32::wrapping_add(i.get(), rhs))),
    )
  }
}

impl AddAssign<u32> for Rav1dTaskIndex {
  fn add_assign(&mut self, rhs: u32) {
    *self = *self + rhs;
  }
}

#[derive(Clone, Copy)]
enum PendingMerge {
  Merged,
  Unchanged,
}

impl From<bool> for PendingMerge {
  fn from(merged: bool) -> Self {
    match merged {
      true => Self::Merged,
      false => Self::Unchanged,
    }
  }
}

#[inline]
fn merge_pending(c: &Rav1dContext) -> Rav1dResult<PendingMerge> {
  let mut outcome = PendingMerge::Unchanged;

  for fc in c.fc.iter() {
    let checked_value = fc.task_thread.tasks.merge_pending_frame(c)?;

    match checked_value {
      PendingMerge::Merged => outcome = PendingMerge::Merged,
      PendingMerge::Unchanged => {}
    }
  }

  Ok(outcome)
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum TileQueueStage {
  Entropy,
  Reconstruction,
}

impl TileQueueStage {
  fn state_slot<T>(
    self,
    [reconstruction, entropy]: &[T; 2],
  ) -> Result<&T, console_core_never::Never> {
    Ok(match self {
      Self::Entropy => entropy,
      Self::Reconstruction => reconstruction,
    })
  }

  fn picture_progress<T>(
    self,
    [entropy, reconstruction]: &[T; 2],
  ) -> Result<&T, console_core_never::Never> {
    Ok(match self {
      Self::Entropy => entropy,
      Self::Reconstruction => reconstruction,
    })
  }
}

fn first_row_task(
  frame: &Rav1dFrameContext,
  data: &Rav1dFrameData,
  stage: TileQueueStage,
) -> Rav1dResult<Rav1dTask> {
  let header = data.frame_hdr()?;
  let sequence = data.seq_hdr()?;
  let task_type = match stage {
    TileQueueStage::Entropy => {
      frame
        .frame_thread_progress
        .entropy
        .store(0, Ordering::Relaxed);
      TaskType::EntropyProgress
    }
    TileQueueStage::Reconstruction => {
      let row_count = u32::try_from(data.sbh).map_err(|_invalid_height| EINVAL)?;
      let words = row_count.div_ceil(u32::BITS);
      let Ok(length) = console_core_number_conversion::index(words);
      let mut picture_rows = frame
        .frame_thread_progress
        .frame
        .try_write()
        .ok_or(EINVAL)?;
      picture_rows.clear();
      picture_rows
        .try_resize_with(length, Default::default)
        .map_err(|_allocation| ENOMEM)?;

      let mut copied_rows = frame.frame_thread_progress.copy_lpf.write();
      copied_rows.clear();
      copied_rows
        .try_resize_with(length, Default::default)
        .map_err(|_allocation| ENOMEM)?;
      frame
        .frame_thread_progress
        .deblock
        .store(0, Ordering::SeqCst);

      let needs_deblock = header.loopfilter.level_y != [0; 2];
      let needs_copy = sequence.cdef != 0 || !data.lf.restore_planes.is_empty();
      let [coded_width, display_width] = header.size.width;

      match (needs_deblock, needs_copy, coded_width != display_width) {
        (true, _, _) => TaskType::DeblockCols,
        (false, true, _) => TaskType::DeblockRows,
        (false, false, true) => TaskType::SuperResolution,
        (false, false, false) => TaskType::ReconstructionProgress,
      }
    }
  };
  let Ok(next_tile_row) = stage.state_slot(&data.frame_thread.next_tile_row);
  let Ok(()) = next_tile_row.set(0);

  Ok(Rav1dTask {
    type_0: task_type,
    frame_idx: frame.index,
    recon_progress: 1,
    ..Default::default()
  })
}

pub(crate) fn rav1d_task_create_tile_sbrow(
  frame: &Rav1dFrameContext,
  data: &Rav1dFrameData,
  stage: TileQueueStage,
) -> Rav1dResult {
  let header = data.frame_hdr()?;
  let count = u32::from(header.tiling.cols).wrapping_mul(u32::from(header.tiling.rows));
  let Ok(tile_count) = console_core_number_conversion::index(count);
  let tiles = data.ts.get(..tile_count).ok_or(EINVAL)?;
  let Ok(sb_shift) = console_core_number_conversion::wrapped::<_, u32>(data.sb_shift);
  let task_type = match stage {
    TileQueueStage::Entropy => TaskType::TileEntropy,
    TileQueueStage::Reconstruction => TaskType::TileReconstruction,
  };
  let tasks = &frame.task_thread.tasks;

  {
    let mut pending = tasks.pending_tasks.lock();
    pending
      .try_reserve(tiles.len().saturating_add(1))
      .map_err(|_allocation| ENOMEM)?;
    let Ok(stage_done) = stage.state_slot(&frame.task_thread.done);
    stage_done.store(0, Ordering::SeqCst);
    let row_task = first_row_task(frame, data, stage)?;
    pending.push(row_task);

    for (tile_idx, tile) in (0..count).zip(tiles) {
      pending.push(Rav1dTask {
        type_0: task_type,
        sby: tile.tiling.row_start.wrapping_shr(sb_shift),
        frame_idx: frame.index,
        tile_idx,
        ..Default::default()
      });
    }
  }

  tasks.pending_tasks_merge.store(true, Ordering::SeqCst);
  frame.task_thread.init_done.store(1, Ordering::SeqCst);
  Ok(())
}

pub(crate) fn rav1d_task_frame_init(c: &Rav1dContext, fc: &Rav1dFrameContext) -> Rav1dResult {
  fc.task_thread.init_done.store(0, Ordering::SeqCst);
  let Ok(task) = Rav1dTask::init(fc.index);
  let _queued = fc.task_thread.insert_task(c, task, QueueWake::Notify)?;
  Ok(())
}

pub(crate) fn rav1d_task_delayed_fg(
  c: &Rav1dContext,
  out: &mut Rav1dPicture,
  in_0: &Rav1dPicture,
) -> Rav1dResult {
  {
    let ttd = &c.task_thread;

    {
      let mut delayed_fg = ttd.delayed_fg.try_write().ok_or(EGeneric)?;

      #[cfg(all(feature = "bitdepth_8", feature = "bitdepth_16"))]
      match (out.p.bpc, &delayed_fg.grain) {
        (8, Grain::Bpc8(_)) | (10 | 12, Grain::Bpc16(_)) => {}
        (8, Grain::Bpc16(_)) => delayed_fg.grain = Grain::Bpc8(Default::default()),
        (10 | 12, Grain::Bpc8(_)) => delayed_fg.grain = Grain::Bpc16(Default::default()),
        (_, _) => return Err(EINVAL),
      }

      delayed_fg.in_0 = in_0.clone();
      delayed_fg.out = out.clone();
      delayed_fg.stage = DelayedGrainStage::Prepare;
      delayed_fg.row_failed.store(false, Ordering::SeqCst);
    }

    let task_thread_lock = ttd.lock.lock();
    let Ok(()) = ttd.delayed_fg_exec.set(1);
    ttd.cond.notify_one();

    let [submitted, finished] = &ttd.delayed_fg_progress;
    let scheduler = wait_while(&ttd.delayed_fg_cond, task_thread_lock, |_lock| {
      let Ok(active) = ttd.delayed_fg_exec.get();
      !ttd.panicked.load(Ordering::SeqCst)
        && (active != 0 || submitted.load(Ordering::SeqCst) != finished.load(Ordering::SeqCst))
    });

    match ::core::primitive::bool::then_some(ttd.panicked.load(Ordering::SeqCst), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = ttd.delayed_fg_exec.set(0);
        return Ok(());
      }
      ::core::option::Option::None => {}
    }

    drop(scheduler);
    submitted.store(0, Ordering::SeqCst);
    finished.store(0, Ordering::SeqCst);

    let mut delayed = ttd.delayed_fg.try_write().ok_or(EGeneric)?;
    let completed = mem::take(&mut *delayed);

    let rows_finished =
      completed.stage == DelayedGrainStage::Apply && !completed.row_failed.load(Ordering::SeqCst);

    match rows_finished.then_some(()) {
      Some(()) => {}
      None => return Err(EINVAL),
    }
  };

  Ok(())
}

#[derive(Clone, Copy)]
enum FramePipeline {
  Serial,
  Parallel,
}

#[derive(Clone, Copy)]
enum TileReadiness {
  Ready,
  Waiting,
}

fn tile_pipeline(c: &Rav1dContext) -> Result<FramePipeline, console_core_never::Never> {
  Ok(match c.fc.len() {
    0 | 1 => FramePipeline::Serial,
    _ => FramePipeline::Parallel,
  })
}

fn reference_tile_progress(
  f: &Rav1dFrameData,
  task_thread: &Rav1dFrameContextTaskThread,
  task: &Rav1dTask,
  tile: &Rav1dTileState,
  stage: TileQueueStage,
) -> Rav1dResult<TileReadiness> {
  let (block_bottom, lowest_rows) = match stage {
    TileQueueStage::Entropy => {
      let shift = u32::try_from(f.sb_shift).map_err(|_invalid_shift| EINVAL)?;
      let bottom = task.sby.wrapping_add(1).wrapping_shl(shift.wrapping_add(2));
      let checked_value = u32::try_from(bottom).map_err(|_outside_frame| EINVAL)?;
      (checked_value, None)
    }
    TileQueueStage::Reconstruction => {
      let shift = u32::try_from(f.sb_shift).map_err(|_invalid_shift| EINVAL)?;
      let row = task
        .sby
        .wrapping_sub(tile.tiling.row_start.wrapping_shr(shift));
      let address = f.lowest_pixel_position(tile, row)?;
      let Ok(index) = console_core_number_conversion::index(address);
      (0, Some(f.lowest_pixel_mem.index(index)))
    }
  };
  let chroma_shift = u32::from(f.sr_cur.p.p.layout == Rav1dPixelLayout::I420);
  let Ok(deps_skip) = task.deps_skip.get();
  let start = u32::try_from(deps_skip).map_err(|_invalid_dependency| EINVAL)?;
  let Ok(skip) = console_core_number_conversion::index(start);

  for (reference, slot) in f.refp.iter().skip(skip).zip(start..7) {
    let lowest = match &lowest_rows {
      None => Some(block_bottom),
      Some(rows) => {
        let Ok(index) = console_core_number_conversion::index(slot);
        let &[luma, chroma] = rows.get(index).ok_or(EINVAL)?;
        let y = match luma {
          i32::MIN => i32::MIN,
          _ => luma.wrapping_add(8),
        };
        let uv = match chroma {
          i32::MIN => i32::MIN,
          _ => chroma.wrapping_shl(chroma_shift).wrapping_add(8),
        };
        let bottom = y.max(uv);

        match bottom {
          i32::MIN => None,
          _ => {
            let bound = iclip(bottom, 1, reference.p.p.h);
            let Ok(position) = console_core_number_conversion::wrapped::<_, u32>(bound);
            Some(position)
          }
        }
      }
    };

    match lowest {
      Some(required) => {
        let Ok(counters) = reference.progress();
        let [motion, picture] = counters.ok_or(EINVAL)?;
        let producer = match stage {
          TileQueueStage::Entropy => motion,
          TileQueueStage::Reconstruction => picture,
        };
        let progress = producer.load(Ordering::SeqCst);

        match { ::core::primitive::bool::then_some(progress < required, ()) } {
          ::core::option::Option::Some(()) => {
            return Ok(TileReadiness::Waiting);
          }
          ::core::option::Option::None => {}
        }

        let _previous_error = task_thread
          .error
          .fetch_or(i32::from(progress == FRAME_ERROR), Ordering::SeqCst);
      }
      None => {}
    }

    let Ok(()) = task.deps_skip.update(|previous| previous.wrapping_add(1));
  }

  Ok(TileReadiness::Ready)
}

fn check_tile(
  f: &Rav1dFrameData,
  task_thread: &Rav1dFrameContextTaskThread,
  task: &Rav1dTask,
  pipeline: FramePipeline,
) -> Rav1dResult<TileReadiness> {
  let stage = match task.type_0 {
    TaskType::TileEntropy => TileQueueStage::Entropy,
    TaskType::TileReconstruction => TileQueueStage::Reconstruction,
    TaskType::Initial
    | TaskType::InitCdf
    | TaskType::EntropyProgress
    | TaskType::DeblockCols
    | TaskType::DeblockRows
    | TaskType::Cdef
    | TaskType::SuperResolution
    | TaskType::LoopRestoration
    | TaskType::ReconstructionProgress => return Err(EINVAL),
  };
  let tile = f.tile(task.tile_idx)?;
  let Ok(counter) = stage.state_slot(&tile.progress);
  let first = counter.load(Ordering::SeqCst);

  match { ::core::primitive::bool::then_some(first < task.sby, ()) } {
    ::core::option::Option::Some(()) => {
      return Ok(TileReadiness::Waiting);
    }
    ::core::option::Option::None => {}
  }

  let tile_error = i32::from(first == TILE_ERROR);
  let mut error = task_thread.error.fetch_or(tile_error, Ordering::SeqCst) | tile_error;

  match (error, pipeline, stage) {
    (0, FramePipeline::Parallel, TileQueueStage::Reconstruction) => {
      let Ok(entropy) = TileQueueStage::Entropy.state_slot(&tile.progress);
      let second = entropy.load(Ordering::SeqCst);

      match { ::core::primitive::bool::then_some(second <= task.sby, ()) } {
        ::core::option::Option::Some(()) => {
          return Ok(TileReadiness::Waiting);
        }
        ::core::option::Option::None => {}
      }

      let producer_error = i32::from(second == TILE_ERROR);
      error = task_thread.error.fetch_or(producer_error, Ordering::SeqCst) | producer_error;
    }
    _ => {}
  }

  match (error, pipeline) {
    (0, FramePipeline::Parallel) => {
      let frame = f.frame_hdr.as_ref().ok_or(EINVAL)?;

      match frame.frame_type.is_key_or_intra().then_some(()) {
        Some(()) => Ok(TileReadiness::Ready),
        None => reference_tile_progress(f, task_thread, task, tile, stage),
      }
    }
    _ => Ok(TileReadiness::Ready),
  }
}

fn check_tile_or_record_error(
  f: &Rav1dFrameData,
  task_thread: &Rav1dFrameContextTaskThread,
  task: &Rav1dTask,
  pipeline: FramePipeline,
) -> Result<TileReadiness, console_core_never::Never> {
  Ok(match check_tile(f, task_thread, task, pipeline) {
    Ok(readiness) => readiness,
    Err(error) => {
      let _previous_error = task_thread.error.fetch_or(1, Ordering::SeqCst);
      console_core_logging::debug!("AV1 tile scheduling failed with {error:?}");
      TileReadiness::Ready
    }
  })
}

struct ProgressBit<'a> {
  word: &'a AtomicU32,
  mask: u32,
}

enum RowCompletion {
  Pending,
  Complete,
}

impl<'a> ProgressBit<'a> {
  fn at(words: &'a [AtomicU32], row: i32) -> Rav1dResult<Self> {
    let position = u32::try_from(row).map_err(|_negative_row| EINVAL)?;
    let Ok(index) = console_core_number_conversion::index(position.wrapping_shr(5));
    let word = words.get(index).ok_or(EINVAL)?;
    Ok(Self {
      word,
      mask: 1_u32.wrapping_shl(position & 31),
    })
  }

  fn load(self) -> Result<RowCompletion, console_core_never::Never> {
    let value = self.word.load(Ordering::SeqCst);
    Ok(match (value & self.mask).cmp(&0) {
      cmp::Ordering::Equal => RowCompletion::Pending,
      cmp::Ordering::Less | cmp::Ordering::Greater => RowCompletion::Complete,
    })
  }

  fn complete(self) -> Result<(), console_core_never::Never> {
    let _previous = self.word.fetch_or(self.mask, Ordering::SeqCst);
    Ok(())
  }
}

#[inline]
fn get_frame_progress(fc: &Rav1dFrameContext, f: &Rav1dFrameData) -> Rav1dResult<i32> {
  let Ok(counters) = f.sr_cur.progress();
  let row_progress = match counters {
    Some([_, completed]) => completed.load(Ordering::SeqCst),
    None => 0,
  };

  match row_progress.cmp(&FRAME_ERROR) {
    cmp::Ordering::Equal | cmp::Ordering::Greater => return Ok(f.sbh.wrapping_sub(1)),
    cmp::Ordering::Less => {}
  }

  let progress_map = fc.frame_thread_progress.frame.try_read().ok_or(EINVAL)?;
  let Ok(shift) = console_core_number_conversion::wrapped::<_, u32>(f.sb_shift.wrapping_add(7));
  let first_word = row_progress.wrapping_shr(shift);
  let Ok(skip_words) = console_core_number_conversion::index(first_word);
  let first_incomplete =
    progress_map
      .iter()
      .zip(0_u32..)
      .skip(skip_words)
      .find_map(|(entry, word)| {
        let pending_bits = !entry.load(Ordering::SeqCst);

        match pending_bits.trailing_zeros() {
          32 => None,
          pending => Some((word, pending)),
        }
      });
  let (word, bit) = match first_incomplete {
    Some(incomplete) => incomplete,
    None => {
      let Ok(word_count) = console_core_number_conversion::wrapped::<_, u32>(progress_map.len());
      (word_count, 0)
    }
  };
  let Ok(first_pending) =
    console_core_number_conversion::wrapped::<_, i32>(word.wrapping_shl(5) | bit);
  Ok(first_pending.wrapping_sub(1))
}

#[inline]
fn abort_frame(
  decoder: &Rav1dContext,
  frame: &Rav1dFrameContext,
  error: Rav1dError,
) -> Result<(), console_core_never::Never> {
  let tasks = &frame.task_thread;
  let code = match error {
    EINVAL => 1,
    EGeneric
    | EAGAIN
    | ENOMEM
    | Rav1dError::ENOENT
    | Rav1dError::EIO
    | Rav1dError::ERANGE
    | Rav1dError::ENOPROTOOPT
    | Rav1dError::ECANCELED => -1,
  };
  tasks.error.store(code, Ordering::SeqCst);
  tasks.task_counter.store(0, Ordering::SeqCst);
  let [reconstruction, entropy] = &tasks.done;
  reconstruction.store(1, Ordering::SeqCst);
  entropy.store(1, Ordering::SeqCst);

  {
    let data = frame.data.try_read();
    let progress = data.as_deref().and_then(|current| {
      let Ok(counters) = current.sr_cur.progress();
      counters
    });

    match progress {
      Some([first, last]) => {
        first.store(FRAME_ERROR, Ordering::SeqCst);
        last.store(FRAME_ERROR, Ordering::SeqCst);
      }
      None => {}
    }
  }

  publish_frame_completion(decoder, frame, Err(error))
}

enum GrainStart {
  ApplyRows,
  PreparationFailed,
}

fn prepare_worker_grain(control: &TaskThreadData) -> Rav1dResult<GrainStart> {
  let stage = {
    let grain = control.delayed_fg.try_read().ok_or(EINVAL)?;
    grain.stage
  };

  match stage {
    DelayedGrainStage::Apply => return Ok(GrainStart::ApplyRows),
    DelayedGrainStage::Idle => return Err(EINVAL),
    DelayedGrainStage::Prepare => {}
  }

  let Ok(()) = control.delayed_fg_exec.set(0);

  match control.cond_signaled.load(Ordering::SeqCst) {
    0 => {}
    _ => control.cond.notify_one(),
  }

  let mut storage = control.delayed_fg.try_write().ok_or(EINVAL)?;
  let state = &mut *storage;
  let Ok(backend) = Rav1dBitDepthDSPContext::get(state.out.p.bpc);
  let prepared = match backend {
    Some(depth) => match &mut state.grain {
      #[cfg(feature = "bitdepth_8")]
      Grain::Bpc8(grain) => {
        rav1d_prep_grain::<BitDepth8>(&depth.fg, &mut state.out, &state.in_0, grain)
      }
      #[cfg(feature = "bitdepth_16")]
      Grain::Bpc16(grain) => {
        rav1d_prep_grain::<BitDepth16>(&depth.fg, &mut state.out, &state.in_0, grain)
      }
    },
    None => Err(EINVAL),
  };

  match prepared {
    Ok(()) => {
      state.stage = DelayedGrainStage::Apply;
      let Ok(()) = control.delayed_fg_exec.set(1);
      Ok(GrainStart::ApplyRows)
    }
    Err(_invalid_film_grain) => {
      control.delayed_fg_cond.notify_one();
      Ok(GrainStart::PreparationFailed)
    }
  }
}

fn apply_worker_grain_row(state: &TaskThreadDataDelayedFg, row: i32) -> Rav1dResult {
  let Ok(backend) = Rav1dBitDepthDSPContext::get(state.out.p.bpc);
  let depth = backend.ok_or(EINVAL)?;
  let position = u32::try_from(row).map_err(|_invalid_grain_row| EINVAL)?;

  match &state.grain {
    #[cfg(feature = "bitdepth_8")]
    Grain::Bpc8(grain) => {
      rav1d_apply_grain_row::<BitDepth8>(&depth.fg, &state.out, &state.in_0, grain, position)
    }
    #[cfg(feature = "bitdepth_16")]
    Grain::Bpc16(grain) => {
      rav1d_apply_grain_row::<BitDepth16>(&depth.fg, &state.out, &state.in_0, grain, position)
    }
  }
}

#[inline]
fn delayed_fg_task<'ttd>(
  control: &'ttd TaskThreadData,
  scheduler: &mut Option<MutexGuard<'ttd, ()>>,
) -> Rav1dResult {
  let start = prepare_worker_grain(control)?;

  match start {
    GrainStart::ApplyRows => {}
    GrainStart::PreparationFailed => return Ok(()),
  }

  let [claimed, applied] = &control.delayed_fg_progress;
  let mut row = claimed.fetch_add(1, Ordering::SeqCst);
  drop(scheduler.take());
  let grain = control.delayed_fg.try_read().ok_or(EINVAL)?;
  let block_rows = i32::from(FG_BLOCK_SIZE);
  let rounded = grain.out.p.h.wrapping_add(block_rows).wrapping_sub(1);
  let row_count = rounded.checked_div(block_rows).ok_or(EINVAL)?;

  for _ in 0..=row_count.max(0) {
    match row.wrapping_add(1).cmp(&row_count) {
      cmp::Ordering::Less => control.cond.notify_one(),
      cmp::Ordering::Equal => {
        let _held = control.lock.lock();
        let Ok(()) = control.delayed_fg_exec.set(0);
      }
      cmp::Ordering::Greater => {
        *scheduler = Some(control.lock.lock());
        let Ok(()) = control.delayed_fg_exec.set(0);
        break;
      }
    }

    match apply_worker_grain_row(&grain, row) {
      Ok(()) => {}
      Err(_invalid_row) => grain.row_failed.store(true, Ordering::SeqCst),
    }

    row = claimed.fetch_add(1, Ordering::SeqCst);
    let _finished = applied.fetch_add(1, Ordering::SeqCst);

    match row.cmp(&row_count) {
      cmp::Ordering::Less => {}
      cmp::Ordering::Equal | cmp::Ordering::Greater => {
        *scheduler = Some(control.lock.lock());
        let Ok(()) = control.delayed_fg_exec.set(0);
        break;
      }
    }
  }

  let done = applied.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
  let last_claim = claimed.load(Ordering::SeqCst);

  match done.cmp(&last_claim) {
    cmp::Ordering::Equal | cmp::Ordering::Greater => control.delayed_fg_cond.notify_one(),
    cmp::Ordering::Less => {}
  }

  Ok(())
}

struct WorkerPanicGuard<'a> {
  c: &'a Rav1dContext,
  thread_data: &'a Rav1dTaskContextTaskThread,
}

impl WorkerPanicGuard<'_> {
  fn notify_failure(&self) -> Result<(), console_core_never::Never> {
    let ttd = &self.c.task_thread;

    let _task_thread_lock = ttd.lock.lock();
    ttd.panicked.store(true, Ordering::SeqCst);

    let Ok(()) = self.thread_data.flushed.set(true);

    for fc in self.c.fc.iter() {
      fc.task_thread.cond.notify_one();
    }

    for tc in self.c.tc.iter() {
      tc.thread_data.cond.notify_one();
    }

    ttd.cond.notify_all();
    ttd.delayed_fg_cond.notify_all();
    Ok(())
  }
}

impl Drop for WorkerPanicGuard<'_> {
  fn drop(&mut self) {
    match std::thread::panicking().then_some(()) {
      Some(()) => {
        let Ok(()) = self.notify_failure();
      }
      None => {}
    }
  }
}

pub(crate) fn rav1d_worker_task(
  c: &Rav1dContext,
  position: u32,
) -> core::result::Result<(), console_core_never::Never> {
  {
    let Ok(index) = console_core_number_conversion::index(position);
    let worker = match c.tc.get(index) {
      Some(slot) => &slot.thread_data,
      None => {
        console_core_logging::error!("AV1 worker slot {position} is unavailable");
        return Ok(());
      }
    };
    let failure_guard = WorkerPanicGuard {
      c,
      thread_data: worker,
    };

    match run_worker(c, worker) {
      Ok(()) => {}
      Err(error) => {
        console_core_logging::error!("AV1 worker {position} failed: {error:?}");
        let Ok(()) = failure_guard.notify_failure();
      }
    }
  }

  Ok(())
}

enum TaskEligibility {
  Waiting,
  Ready { following: Option<Rav1dTask> },
}

fn check_progress_task(
  frame: &Rav1dFrameData,
  context: &Rav1dFrameContext,
  task: &Rav1dTask,
  stage: TileQueueStage,
) -> Rav1dResult<TaskEligibility> {
  let Ok(final_progress) = stage.state_slot(&context.task_thread.done);
  let done = final_progress.load(Ordering::SeqCst);
  let error = context.task_thread.error.load(Ordering::SeqCst);

  match (done, error) {
    (0, _) => {}
    (_, 0) => return Err(EINVAL),
    (_, _) => {}
  }

  let header = frame.frame_hdr()?;
  let columns = i32::from(header.tiling.cols);
  let Ok(next_tile_row) = stage.state_slot(&frame.frame_thread.next_tile_row);
  let Ok(row) = next_tile_row.get();
  let tile_row_base = columns.wrapping_mul(row);

  match stage {
    TileQueueStage::Entropy => {
      let progress = context.frame_thread_progress.entropy.load(Ordering::SeqCst);

      match progress.cmp(&task.sby) {
        cmp::Ordering::Less => return Ok(TaskEligibility::Waiting),
        cmp::Ordering::Equal | cmp::Ordering::Greater => {
          let _previous_error = context
            .task_thread
            .error
            .fetch_or(i32::from(progress == TILE_ERROR), Ordering::SeqCst);
        }
      }
    }
    TileQueueStage::Reconstruction => {}
  }

  for column in 0..columns {
    let position =
      u32::try_from(tile_row_base.wrapping_add(column)).map_err(|_outside_tiles| EINVAL)?;
    let tile = frame.tile(position)?;
    let Ok(counter) = stage.state_slot(&tile.progress);
    let progress = counter.load(Ordering::SeqCst);

    match progress.cmp(&task.recon_progress) {
      cmp::Ordering::Less => return Ok(TaskEligibility::Waiting),
      cmp::Ordering::Equal | cmp::Ordering::Greater => {
        let _previous_error = context
          .task_thread
          .error
          .fetch_or(i32::from(progress == TILE_ERROR), Ordering::SeqCst);
      }
    }
  }

  let next = task.sby.wrapping_add(1);

  match next.cmp(&frame.sbh) {
    cmp::Ordering::Equal | cmp::Ordering::Greater => {
      Ok(TaskEligibility::Ready { following: None })
    }
    cmp::Ordering::Less => {
      let Ok(current) = next_tile_row.get();
      let following_row = current.wrapping_add(1);
      let index = console_core_number_conversion::checked_index(following_row)
        .map_err(|_outside_rows| EINVAL)?;
      let start = header
        .tiling
        .row_start_sb
        .get(index)
        .copied()
        .ok_or(EINVAL)?;

      match next.cmp(&i32::from(start)) {
        cmp::Ordering::Equal => {
          let Ok(()) = next_tile_row.set(following_row);
        }
        cmp::Ordering::Less | cmp::Ordering::Greater => {}
      }

      let Ok(base) = task.without_next();
      let following = Rav1dTask {
        sby: next,
        recon_progress: next.wrapping_add(1),
        ..base
      };
      Ok(TaskEligibility::Ready {
        following: Some(following),
      })
    }
  }
}

struct ScheduledTask<'a> {
  frame: &'a Rav1dFrameContext,
  index: Rav1dTaskIndex,
  previous: Rav1dTaskIndex,
}

#[derive(Clone, Copy)]
enum InputCdfProgress {
  Waiting,
  Available,
  Failed,
}

fn input_cdf_progress(frame: &Rav1dFrameContext) -> Rav1dResult<InputCdfProgress> {
  let input = frame.in_cdf()?;
  let Ok(progress) = input.progress();
  let code = match progress {
    Some(counter) => counter.load(Ordering::SeqCst),
    None => 1,
  };
  let Ok(failure) = console_core_number_conversion::wrapped::<_, u32>(TILE_ERROR);
  Ok(match (code, code.cmp(&failure)) {
    (0, _) => InputCdfProgress::Waiting,
    (_, cmp::Ordering::Equal) => InputCdfProgress::Failed,
    (_, cmp::Ordering::Less | cmp::Ordering::Greater) => InputCdfProgress::Available,
  })
}

fn select_initial_task<'a>(
  decoder: &'a Rav1dContext,
  control: &TaskThreadData,
) -> Rav1dResult<Option<ScheduledTask<'a>>> {
  match decoder.fc.len() {
    0 | 1 => return Ok(None),
    2.. => {}
  }

  let first = u64::from(control.first.load(Ordering::SeqCst));
  let Ok(frames) = wrapping_iter(decoder.fc.iter(), first);

  for frame in frames {
    let tasks = &frame.task_thread;

    match tasks.init_done.load(Ordering::SeqCst) {
      0 => {}
      _ => continue,
    }

    let index = tasks.tasks.head.load(Ordering::SeqCst);

    match index {
      Rav1dTaskIndex::None => continue,
      _ => {}
    }

    let task_view = tasks.tasks.read(index)?;
    let task = task_view.entry()?;
    let previous = Rav1dTaskIndex::None;

    match task.type_0 {
      TaskType::Initial => {
        return Ok(Some(ScheduledTask {
          frame,
          index,
          previous,
        }));
      }
      TaskType::InitCdf => {
        let progress = input_cdf_progress(frame)?;

        match progress {
          InputCdfProgress::Waiting => {}
          InputCdfProgress::Available | InputCdfProgress::Failed => {
            let failed = i32::from(matches!(progress, InputCdfProgress::Failed));
            let _previous_error = tasks.error.fetch_or(failed, Ordering::SeqCst);
            return Ok(Some(ScheduledTask {
              frame,
              index,
              previous,
            }));
          }
        }
      }
      TaskType::TileEntropy
      | TaskType::EntropyProgress
      | TaskType::TileReconstruction
      | TaskType::DeblockCols
      | TaskType::DeblockRows
      | TaskType::Cdef
      | TaskType::SuperResolution
      | TaskType::LoopRestoration
      | TaskType::ReconstructionProgress => {}
    }
  }

  Ok(None)
}

fn check_filter_task(
  frame: &Rav1dFrameContext,
  task: &Rav1dTask,
) -> Rav1dResult<TaskEligibility> {
  let progress = &frame.frame_thread_progress;

  match task.type_0 {
    TaskType::Cdef => {
      let copied = progress.copy_lpf.try_read().ok_or(EINVAL)?;
      let previous = ProgressBit::at(&copied, task.sby.wrapping_sub(1))?;
      let Ok(completion) = previous.load();
      Ok(match completion {
        RowCompletion::Complete => TaskEligibility::Ready { following: None },
        RowCompletion::Pending => TaskEligibility::Waiting,
      })
    }
    TaskType::Initial
    | TaskType::InitCdf
    | TaskType::TileEntropy
    | TaskType::EntropyProgress
    | TaskType::TileReconstruction
    | TaskType::DeblockCols
    | TaskType::DeblockRows
    | TaskType::SuperResolution
    | TaskType::LoopRestoration
    | TaskType::ReconstructionProgress => {
      match task.deblock_progress {
        0 => return Err(EINVAL),
        _ => {}
      }

      let deblocked = progress.deblock.load(Ordering::SeqCst);

      match deblocked.cmp(&task.deblock_progress) {
        cmp::Ordering::Less => Ok(TaskEligibility::Waiting),
        cmp::Ordering::Equal | cmp::Ordering::Greater => {
          let _previous = frame
            .task_thread
            .error
            .fetch_or(i32::from(deblocked == TILE_ERROR), Ordering::SeqCst);
          Ok(TaskEligibility::Ready { following: None })
        }
      }
    }
  }
}

fn check_task_candidate(
  decoder: &Rav1dContext,
  frame: &Rav1dFrameContext,
  task: &Rav1dTask,
) -> Rav1dResult<TaskEligibility> {
  match task.type_0 {
    TaskType::InitCdf => Ok(TaskEligibility::Waiting),
    TaskType::TileEntropy | TaskType::TileReconstruction => {
      let data = frame.data.read();
      let Ok(pipeline) = tile_pipeline(decoder);
      let Ok(readiness) = check_tile_or_record_error(&data, &frame.task_thread, task, pipeline);
      Ok(match readiness {
        TileReadiness::Ready => TaskEligibility::Ready { following: None },
        TileReadiness::Waiting => TaskEligibility::Waiting,
      })
    }
    TaskType::Initial
    | TaskType::EntropyProgress
    | TaskType::DeblockCols
    | TaskType::DeblockRows
    | TaskType::Cdef
    | TaskType::SuperResolution
    | TaskType::LoopRestoration
    | TaskType::ReconstructionProgress => match task.recon_progress {
      0 => check_filter_task(frame, task),
      _ => {
        let data = frame.data.read();
        let stage = match task.type_0 {
          TaskType::EntropyProgress => TileQueueStage::Entropy,
          TaskType::Initial
          | TaskType::InitCdf
          | TaskType::TileEntropy
          | TaskType::TileReconstruction
          | TaskType::DeblockCols
          | TaskType::DeblockRows
          | TaskType::Cdef
          | TaskType::SuperResolution
          | TaskType::LoopRestoration
          | TaskType::ReconstructionProgress => TileQueueStage::Reconstruction,
        };
        check_progress_task(&data, frame, task, stage)
      }
    },
  }
}

fn scan_pending_frame<'a>(
  decoder: &Rav1dContext,
  frame: &'a Rav1dFrameContext,
) -> Rav1dResult<Option<ScheduledTask<'a>>> {
  let tasks = &frame.task_thread.tasks;
  let _merged = tasks.merge_pending_frame(decoder)?;
  let Ok(mut previous) = tasks.cur_prev.get();
  let mut index = match previous {
    Rav1dTaskIndex::None => tasks.head.load(Ordering::SeqCst),
    position => {
      let view = tasks.read(position)?;
      let task = view.entry()?;
      let Ok(next) = task.next();
      next
    }
  };
  let entries = tasks.tasks.try_read().ok_or(EINVAL)?;
  let Ok(limit) = console_core_number_conversion::fixed_index(entries.len());
  drop(entries);

  for _ in 0..limit {
    match index {
      Rav1dTaskIndex::None => return Ok(None),
      _ => {}
    }

    let view = tasks.read(index)?;
    let task = view.entry()?;
    let eligibility = check_task_candidate(decoder, frame, task)?;

    match eligibility {
      TaskEligibility::Waiting => {}
      TaskEligibility::Ready { following } => {
        drop(view);

        match following {
          Some(following_task) => {
            let _queued =
              frame
                .task_thread
                .insert_task(decoder, following_task, QueueWake::KeepParked)?;
          }
          None => {}
        }

        return Ok(Some(ScheduledTask {
          frame,
          index,
          previous,
        }));
      }
    }

    previous = index;
    let Ok(next) = task.next();
    index = next;
    let Ok(()) = tasks.cur_prev.set(previous);
  }

  match index {
    Rav1dTaskIndex::None => Ok(None),
    _ => Err(EINVAL),
  }
}

fn select_next_worker_task<'a>(
  decoder: &'a Rav1dContext,
  control: &TaskThreadData,
) -> Rav1dResult<Option<ScheduledTask<'a>>> {
  let initialization = select_initial_task(decoder, control)?;

  match initialization {
    Some(task) => return Ok(Some(task)),
    None => {}
  }

  let Ok(frame_count) = console_core_number_conversion::wrapped::<_, u32>(decoder.fc.len());
  let Ok(first_candidate) = control.cur.get();

  for _ in first_candidate..frame_count {
    let first = control.first.load(Ordering::SeqCst);
    let Ok(offset) = control.cur.get();
    let slot = first
      .wrapping_add(offset)
      .checked_rem(frame_count)
      .ok_or(EINVAL)?;
    let Ok(index) = console_core_number_conversion::index(slot);
    let frame = decoder.fc.get(index).ok_or(EINVAL)?;
    let candidate = scan_pending_frame(decoder, frame)?;

    match candidate {
      Some(task) => return Ok(Some(task)),
      None => {}
    }

    let Ok(()) = control.cur.update(|current| current.wrapping_add(1));
  }

  Ok(None)
}

struct RowProgress {
  next: i32,
  total: i32,
  block_size: i32,
}

impl RowProgress {
  fn pixels(&self) -> Result<u32, console_core_never::Never> {
    Ok(match self.next.cmp(&self.total) {
      cmp::Ordering::Equal => u32::MAX,
      cmp::Ordering::Less | cmp::Ordering::Greater => {
        let Ok(row) = console_core_number_conversion::wrapped::<_, u32>(self.next);
        let Ok(step) = console_core_number_conversion::wrapped::<_, u32>(self.block_size);
        row.wrapping_mul(step)
      }
    })
  }
}

fn record_picture_row(
  frame: &Rav1dFrameData,
  stage: TileQueueStage,
  row: &RowProgress,
  error: i32,
) -> Rav1dResult {
  let Ok(progress) = frame.sr_cur.progress();
  let counter = match stage {
    TileQueueStage::Entropy => {
      let required = progress.ok_or(EINVAL)?;
      Some(required)
    }
    TileQueueStage::Reconstruction => progress,
  };

  match (counter, &frame.sr_cur.p.data) {
    (Some(picture), Some(_)) => {
      let Ok(output) = stage.picture_progress(picture);
      let Ok(pixels) = row.pixels();
      let value = match error {
        0 => pixels,
        _ => FRAME_ERROR,
      };
      output.store(value, Ordering::SeqCst);
    }
    (None, _) | (_, None) => {}
  }

  Ok(())
}

struct ObservedWorkerRow {
  next: i32,
  error: i32,
}

fn observe_completed_row(
  context: &Rav1dFrameContext,
  frame: &Rav1dFrameData,
  stage: TileQueueStage,
  row: i32,
) -> Rav1dResult<ObservedWorkerRow> {
  let block_size = frame.sb_step.wrapping_mul(4);

  match stage {
    TileQueueStage::Entropy => {
      let error = context.task_thread.error.load(Ordering::SeqCst);
      let progress = RowProgress {
        next: row.wrapping_add(1),
        total: frame.sbh,
        block_size,
      };
      record_picture_row(frame, stage, &progress, error)?;
      Ok(ObservedWorkerRow {
        next: progress.next,
        error,
      })
    }
    TileQueueStage::Reconstruction => {
      {
        let map = context
          .frame_thread_progress
          .frame
          .try_read()
          .ok_or(EINVAL)?;
        let complete = ProgressBit::at(&map, row)?;
        let Ok(()) = complete.complete();
      }

      let _progress_lock = context.task_thread.lock.lock();
      let completed = get_frame_progress(context, frame)?;
      let error = context.task_thread.error.load(Ordering::SeqCst);
      let progress = RowProgress {
        next: completed.wrapping_add(1),
        total: frame.sbh,
        block_size,
      };
      record_picture_row(frame, stage, &progress, error)?;
      Ok(ObservedWorkerRow {
        next: progress.next,
        error,
      })
    }
  }
}

fn complete_worker_row<'a>(
  decoder: &'a Rav1dContext,
  frame: &Rav1dFrameContext,
  task: &Rav1dTask,
) -> Rav1dResult<MutexGuard<'a, ()>> {
  let stage = match task.type_0 {
    TaskType::EntropyProgress => TileQueueStage::Entropy,
    TaskType::Initial
    | TaskType::InitCdf
    | TaskType::TileEntropy
    | TaskType::TileReconstruction
    | TaskType::DeblockCols
    | TaskType::DeblockRows
    | TaskType::Cdef
    | TaskType::SuperResolution
    | TaskType::LoopRestoration
    | TaskType::ReconstructionProgress => TileQueueStage::Reconstruction,
  };
  let data = frame.data.try_read().ok_or(EINVAL)?;
  let ObservedWorkerRow { next, error } = observe_completed_row(frame, &data, stage, task.sby)?;
  let total = data.sbh;
  drop(data);

  match stage {
    TileQueueStage::Entropy => {
      let value = match error {
        0 => next,
        _ => TILE_ERROR,
      };
      frame
        .frame_thread_progress
        .entropy
        .store(value, Ordering::SeqCst);
    }
    TileQueueStage::Reconstruction => {}
  }

  match next.cmp(&total) {
    cmp::Ordering::Equal => {
      let Ok(done) = stage.state_slot(&frame.task_thread.done);
      done.store(1, Ordering::SeqCst);
    }
    cmp::Ordering::Less | cmp::Ordering::Greater => {}
  }

  let control = &decoder.task_thread;
  let scheduler = control.lock.lock();
  let remaining = frame
    .task_thread
    .task_counter
    .fetch_sub(1, Ordering::SeqCst)
    .wrapping_sub(1);
  let [reconstruction, entropy] = &frame.task_thread.done;
  let Ok(pipeline) = tile_pipeline(decoder);
  let complete = match (stage, pipeline) {
    (TileQueueStage::Reconstruction, FramePipeline::Serial) => {
      reconstruction.load(Ordering::SeqCst) != 0
    }
    (TileQueueStage::Entropy, _) | (TileQueueStage::Reconstruction, FramePipeline::Parallel) => {
      reconstruction.load(Ordering::SeqCst) != 0 && entropy.load(Ordering::SeqCst) != 0
    }
  };

  match (remaining, complete) {
    (0, true) => {
      let final_error = frame.task_thread.error.load(Ordering::SeqCst);
      let outcome = worker_frame_outcome(final_error);
      let Ok(()) = publish_frame_completion(decoder, frame, outcome);
    }
    (_, _) => {}
  }

  let Ok(_reset) = reset_task_cur(decoder, control, Some(task.frame_idx));
  Ok(scheduler)
}

fn attempt_frame_cdf(
  decoder: &Rav1dContext,
  frame: &Rav1dFrameContext,
) -> Rav1dResult<Rav1dResult> {
  let mut data = frame.data.try_write().ok_or(EINVAL)?;
  let outcome = match frame.task_thread.error.load(Ordering::SeqCst) {
    0 => {
      let input = frame.in_cdf()?;
      rav1d_decode_frame_init_cdf(decoder, frame, &mut data, &input)
    }
    _ => Err(EINVAL),
  };
  let header = data.frame_hdr()?;

  match header.refresh_context {
    0 => {}
    _ => {
      let Ok(update_required) = frame.task_thread.update_set.get();

      match update_required.then_some(()) {
        Some(()) => {}
        None => {
          let Ok(tracker) = data.out_cdf.progress();
          let output = tracker.ok_or(EINVAL)?;
          let Ok(failed) = console_core_number_conversion::wrapped::<_, u32>(TILE_ERROR);
          let status = match outcome {
            Ok(()) => 1,
            Err(_invalid_frame_cdf) => failed,
          };
          output.store(status, Ordering::SeqCst);
        }
      }
    }
  }

  Ok(outcome)
}

enum QueueFailure {
  OtherStagePending,
  RejectFrame,
}

fn record_failed_frame_queue(
  frame: &Rav1dFrameContext,
  data: &Rav1dFrameData,
  stage: TileQueueStage,
) -> Rav1dResult<QueueFailure> {
  let tasks = &frame.task_thread;
  let Ok(done) = stage.state_slot(&tasks.done);
  done.store(1, Ordering::SeqCst);
  tasks.error.store(-1, Ordering::SeqCst);
  let header = data.frame_hdr()?;
  let columns = i32::from(header.tiling.cols);
  let rows = i32::from(header.tiling.rows);
  let absent_tasks = columns.wrapping_mul(rows).wrapping_add(data.sbh);
  let _previous = tasks.task_counter.fetch_sub(absent_tasks, Ordering::SeqCst);
  let Ok(counters) = data.sr_cur.progress();
  let picture = counters.ok_or(EINVAL)?;
  let Ok(progress) = stage.picture_progress(picture);
  progress.store(FRAME_ERROR, Ordering::SeqCst);
  let [_, entropy] = &tasks.done;

  match (stage, entropy.load(Ordering::SeqCst)) {
    (TileQueueStage::Entropy, _) | (TileQueueStage::Reconstruction, 0) => {
      Ok(QueueFailure::OtherStagePending)
    }
    (TileQueueStage::Reconstruction, _) => match tasks.task_counter.load(Ordering::SeqCst) {
      0 => Ok(QueueFailure::RejectFrame),
      _ => Err(EINVAL),
    },
  }
}

fn queue_frame_rows<'a>(
  decoder: &'a Rav1dContext,
  frame: &Rav1dFrameContext,
) -> Rav1dResult<MutexGuard<'a, ()>> {
  let control = &decoder.task_thread;

  for stage in [TileQueueStage::Entropy, TileQueueStage::Reconstruction] {
    let data = frame.data.try_read().ok_or(EINVAL)?;
    let queued = rav1d_task_create_tile_sbrow(frame, &data, stage);

    match queued {
      Ok(()) => {}
      Err(error) => {
        console_core_logging::debug!("AV1 tile queue could not start: {error:?}");
        let scheduler = control.lock.lock();
        let recovery = record_failed_frame_queue(frame, &data, stage)?;

        match recovery {
          QueueFailure::OtherStagePending => drop(scheduler),
          QueueFailure::RejectFrame => {
            drop(data);
            let Ok(()) = publish_frame_completion(decoder, frame, Err(ENOMEM));
            return Ok(scheduler);
          }
        }
      }
    }
  }

  Ok(control.lock.lock())
}

enum WorkerTaskTransition<'a> {
  Continue(Rav1dTask),
  Scheduled(MutexGuard<'a, ()>),
}

fn wake_idle_worker(control: &TaskThreadData) -> Result<(), console_core_never::Never> {
  match control.cond_signaled.fetch_or(1, Ordering::SeqCst) {
    0 => control.cond.notify_one(),
    _ => {}
  }

  Ok(())
}

fn publish_completed_tile_cdf(
  frame: &Rav1dFrameContext,
  data: &Rav1dFrameData,
  tile: u32,
  pass: i32,
) -> Rav1dResult {
  let mut error = frame.task_thread.error.load(Ordering::SeqCst);
  let header = data.frame_hdr()?;
  let Ok(update_set) = frame.task_thread.update_set.get();

  match (
    header.refresh_context,
    pass,
    update_set,
    u32::from(header.tiling.update).cmp(&tile),
  ) {
    (0, _, _, _)
    | (_, 2.., _, _)
    | (_, _, false, _)
    | (_, _, _, cmp::Ordering::Less | cmp::Ordering::Greater) => return Ok(()),
    (_, ..=1, true, cmp::Ordering::Equal) => {}
  }

  match error {
    0 => match rav1d_publish_tile_cdf(data, header) {
      Ok(()) => {}
      Err(_tile_cdf_unavailable) => {
        error = frame.task_thread.error.fetch_or(1, Ordering::SeqCst) | 1;
      }
    },
    _ => {}
  }

  let Ok(progress) = data.out_cdf.progress();

  match progress {
    Some(position) => {
      let Ok(failed) = console_core_number_conversion::wrapped::<_, u32>(TILE_ERROR);
      let code = match error {
        0 => 1,
        _ => failed,
      };
      position.store(code, Ordering::SeqCst);
    }
    None => {}
  }

  Ok(())
}

fn decode_worker_tile_row<'a>(
  decoder: &'a Rav1dContext,
  frame: &Rav1dFrameContext,
  worker: &mut Rav1dTaskContext,
  mut task: Rav1dTask,
  error: i32,
) -> Rav1dResult<WorkerTaskTransition<'a>> {
  let control = &decoder.task_thread;
  let data = frame.data.try_read().ok_or(EINVAL)?;
  let tile = match data.tile(task.tile_idx) {
    Ok(tile) => tile,
    Err(_invalid_slot) => {
      drop(data);
      let scheduler = control.lock.lock();
      let Ok(()) = abort_frame(decoder, frame, EINVAL);
      let Ok(_reset) = reset_task_cur(decoder, control, Some(task.frame_idx));
      return Ok(WorkerTaskTransition::Scheduled(scheduler));
    }
  };
  let Ok(pipeline) = tile_pipeline(decoder);
  let stage = match task.type_0 {
    TaskType::TileEntropy => TileQueueStage::Entropy,
    TaskType::TileReconstruction => TileQueueStage::Reconstruction,
    TaskType::Initial
    | TaskType::InitCdf
    | TaskType::EntropyProgress
    | TaskType::DeblockCols
    | TaskType::DeblockRows
    | TaskType::Cdef
    | TaskType::SuperResolution
    | TaskType::LoopRestoration
    | TaskType::ReconstructionProgress => return Err(EINVAL),
  };
  let Ok(tile_progress) = stage.state_slot(&tile.progress);
  worker.tile_index = task.tile_idx;
  let Ok(shift) = console_core_number_conversion::wrapped::<_, u32>(data.sb_shift);
  worker.b.y = task.sby.wrapping_shl(shift);
  worker.frame_thread.pass = match (pipeline, stage) {
    (FramePipeline::Serial, _) => 0,
    (FramePipeline::Parallel, TileQueueStage::Entropy) => 1,
    (FramePipeline::Parallel, TileQueueStage::Reconstruction) => 2,
  };
  let outcome = match error {
    0 => match rav1d_decode_tile_sbrow(decoder, worker, &data) {
      Ok(()) => 0,
      Err(()) => 1,
    },
    previous => previous,
  };
  let next = task.sby.wrapping_add(1);
  let progress = match outcome {
    0 => next,
    _ => TILE_ERROR,
  };
  let _previous = frame.task_thread.error.fetch_or(outcome, Ordering::SeqCst);

  match next.wrapping_shl(shift).cmp(&tile.tiling.row_end) {
    cmp::Ordering::Less => {
      task.sby = next;
      task.deps_skip = RelaxedAtomic::from(0);
      let Ok(readiness) = check_tile_or_record_error(&data, &frame.task_thread, &task, pipeline);
      tile_progress.store(progress, Ordering::SeqCst);

      match readiness {
        TileReadiness::Ready => {
          let Ok(frame_count) =
            console_core_number_conversion::wrapped::<_, u32>(decoder.fc.len());
          let Ok(()) = reset_task_cur_async(
            control,
            ResetFramePosition {
              slot: task.frame_idx,
              frame_count,
            },
          );
          let Ok(()) = wake_idle_worker(control);
          Ok(WorkerTaskTransition::Continue(task))
        }
        TileReadiness::Waiting => {
          let Ok(()) = frame.task_thread.tasks.add_pending(task);
          drop(data);
          Ok(WorkerTaskTransition::Scheduled(control.lock.lock()))
        }
      }
    }
    cmp::Ordering::Equal | cmp::Ordering::Greater => {
      let scheduler = control.lock.lock();
      tile_progress.store(progress, Ordering::SeqCst);
      let Ok(_reset) = reset_task_cur(decoder, control, Some(task.frame_idx));
      publish_completed_tile_cdf(frame, &data, task.tile_idx, worker.frame_thread.pass)?;
      let left = frame
        .task_thread
        .task_counter
        .fetch_sub(1, Ordering::SeqCst)
        .wrapping_sub(1);
      let [reconstruction, entropy] = &frame.task_thread.done;
      let completed = left == 0
        && reconstruction.load(Ordering::SeqCst) != 0
        && (matches!(pipeline, FramePipeline::Serial) || entropy.load(Ordering::SeqCst) != 0);

      match completed.then_some(()) {
        Some(()) => {
          let error_code = frame.task_thread.error.load(Ordering::SeqCst);
          drop(data);
          let completed_frame = worker_frame_outcome(error_code);
          let Ok(()) = publish_frame_completion(decoder, frame, completed_frame);
        }
        None => {}
      }

      match frame.task_thread.task_counter.load(Ordering::SeqCst) {
        ..=-1 => return Err(EINVAL),
        0.. => {}
      }

      let Ok(()) = wake_idle_worker(control);
      Ok(WorkerTaskTransition::Scheduled(scheduler))
    }
  }
}

enum DeblockRowProgress {
  Deblocked,
  CopyReady,
  NoCopy,
}

fn complete_deblock_columns<'a>(
  decoder: &'a Rav1dContext,
  frame: &Rav1dFrameContext,
  worker: &mut Rav1dTaskContext,
  task: Rav1dTask,
) -> Rav1dResult<WorkerTaskTransition<'a>> {
  let row = task.sby;
  let data = frame.data.try_read().ok_or(EINVAL)?;

  match frame.task_thread.error.load(Ordering::SeqCst) {
    0 => {
      let Ok(backend) = data.bd_fn();

      match (backend.filter_sbrow_deblock_cols)(decoder, &data, worker, row) {
        Ok(()) => {}
        Err(_invalid_deblock_region) => {
          let _previous_error = frame.task_thread.error.fetch_or(1, Ordering::SeqCst);
        }
      }
    }
    _ => {}
  }

  drop(data);
  let deblocked = frame.frame_thread_progress.deblock.load(Ordering::SeqCst);

  match deblocked.cmp(&row) {
    cmp::Ordering::Less => {
      let Ok(detached) = task.without_next();
      let postponed = Rav1dTask {
        type_0: TaskType::DeblockRows,
        recon_progress: 0,
        deblock_progress: row,
        ..detached
      };
      let Ok(()) = frame.task_thread.tasks.add_pending(postponed);
      Ok(WorkerTaskTransition::Scheduled(
        decoder.task_thread.lock.lock(),
      ))
    }
    cmp::Ordering::Equal | cmp::Ordering::Greater => Ok(WorkerTaskTransition::Continue(task)),
  }
}

fn filter_worker_deblock_rows(
  decoder: &Rav1dContext,
  frame: &Rav1dFrameContext,
  worker: &mut Rav1dTaskContext,
  row: i32,
) -> Rav1dResult<DeblockRowProgress> {
  let data = frame.data.try_read().ok_or(EINVAL)?;

  match frame.task_thread.error.load(Ordering::SeqCst) {
    0 => {
      let Ok(backend) = data.bd_fn();

      match (backend.filter_sbrow_deblock_rows)(decoder, &data, worker, row) {
        Ok(()) => {}
        Err(_invalid_deblock_region) => {
          let _previous_error = frame.task_thread.error.fetch_or(1, Ordering::SeqCst);
        }
      }
    }
    _ => {}
  }

  let sequence = data.seq_hdr()?;
  let header = data.frame_hdr()?;
  Ok(
    match (
      header.loopfilter.level_y,
      sequence.cdef,
      data.lf.restore_planes.is_empty(),
    ) {
      ([0, 0], 0, true) => DeblockRowProgress::NoCopy,
      ([0, 0], _, _) => DeblockRowProgress::CopyReady,
      (_, _, _) => DeblockRowProgress::Deblocked,
    },
  )
}

fn publish_deblock_copy(words: &[AtomicU32], row: i32) -> Rav1dResult<RowCompletion> {
  let current = ProgressBit::at(words, row)?;
  let Ok(()) = current.complete();

  match row {
    0 => Ok(RowCompletion::Complete),
    _ => {
      let previous = ProgressBit::at(words, row.wrapping_sub(1))?;
      let Ok(status) = previous.load();
      Ok(status)
    }
  }
}

fn complete_deblock_rows<'a>(
  decoder: &'a Rav1dContext,
  frame: &Rav1dFrameContext,
  worker: &mut Rav1dTaskContext,
  mut task: Rav1dTask,
) -> Rav1dResult<WorkerTaskTransition<'a>> {
  let control = &decoder.task_thread;
  let row = task.sby;
  let progress = filter_worker_deblock_rows(decoder, frame, worker, row)?;

  match progress {
    DeblockRowProgress::Deblocked => {
      let code = match frame.task_thread.error.load(Ordering::SeqCst) {
        0 => row.wrapping_add(1),
        _ => TILE_ERROR,
      };
      frame
        .frame_thread_progress
        .deblock
        .store(code, Ordering::SeqCst);
      let Ok(frame_count) = console_core_number_conversion::wrapped::<_, u32>(decoder.fc.len());
      let Ok(()) = reset_task_cur_async(
        control,
        ResetFramePosition {
          slot: task.frame_idx,
          frame_count,
        },
      );
      let Ok(()) = wake_idle_worker(control);
    }
    DeblockRowProgress::CopyReady => {
      let completed = frame
        .frame_thread_progress
        .copy_lpf
        .try_read()
        .ok_or(EINVAL)?;

      let published = publish_deblock_copy(&completed, row)?;

      match published {
        RowCompletion::Pending => {
          task.type_0 = TaskType::Cdef;
          task.deblock_progress = 0;
          task.recon_progress = 0;
          let Ok(()) = frame.task_thread.tasks.add_pending(task);
          return Ok(WorkerTaskTransition::Scheduled(control.lock.lock()));
        }
        RowCompletion::Complete => {}
      }
    }
    DeblockRowProgress::NoCopy => {}
  }

  Ok(WorkerTaskTransition::Continue(task))
}

fn complete_worker_cdef(
  decoder: &Rav1dContext,
  frame: &Rav1dFrameContext,
  worker: &mut Rav1dTaskContext,
  task: &Rav1dTask,
) -> Rav1dResult<TaskType> {
  let data = frame.data.try_read().ok_or(EINVAL)?;
  let sequence = match data.seq_hdr() {
    Ok(sequence) => sequence,
    Err(_missing_sequence_header) => {
      let _previous_error = frame.task_thread.error.fetch_or(1, Ordering::SeqCst);
      return Ok(TaskType::ReconstructionProgress);
    }
  };

  match sequence.cdef {
    0 => return Ok(TaskType::SuperResolution),
    _ => {}
  }

  let next = match frame.task_thread.error.load(Ordering::SeqCst) {
    0 => {
      let Ok(backend) = data.bd_fn();

      match (backend.filter_sbrow_cdef)(decoder, &data, worker, task.sby) {
        Ok(()) => TaskType::SuperResolution,
        Err(_invalid_cdef_window) => {
          let _previous_error = frame.task_thread.error.fetch_or(1, Ordering::SeqCst);
          TaskType::ReconstructionProgress
        }
      }
    }
    _ => TaskType::SuperResolution,
  };

  drop(data);
  let Ok(frame_count) = console_core_number_conversion::wrapped::<_, u32>(decoder.fc.len());
  let control = &decoder.task_thread;
  let Ok(()) = reset_task_cur_async(
    control,
    ResetFramePosition {
      slot: task.frame_idx,
      frame_count,
    },
  );
  let Ok(()) = wake_idle_worker(control);
  Ok(next)
}

struct WorkerLifetime<'a> {
  signals: &'a Rav1dTaskContextTaskThread,
}

impl<'a> Iterator for WorkerLifetime<'a> {
  type Item = &'a Rav1dTaskContextTaskThread;

  fn next(&mut self) -> Option<Self::Item> {
    let Ok(stopping) = self.signals.die.get();

    match stopping.then_some(()) {
      Some(()) => None,
      None => Some(self.signals),
    }
  }
}

const MAX_SUPERBLOCK_ROWS_PER_FRAME: u32 = 65_536 / 64;
const WORKER_ROW_FILTER_STAGES: u8 = 6;
const WORKER_PARK_RECHECK: std::time::Duration = std::time::Duration::from_secs(1);

fn park_worker<'scheduler>(
  decoder: &Rav1dContext,
  worker: &Rav1dTaskContextTaskThread,
  control: &TaskThreadData,
  held: MutexGuard<'scheduler, ()>,
) -> Result<MutexGuard<'scheduler, ()>, console_core_never::Never> {
  let Ok(()) = worker.flushed.set(true);
  worker.cond.notify_one();
  control.cond_signaled.store(0, Ordering::SeqCst);
  let (resumed, _poll_interval) = unpoison(control.cond.wait_timeout(held, WORKER_PARK_RECHECK));
  let Ok(()) = worker.flushed.set(false);
  let Ok(_scan) = reset_task_cur(decoder, control, None);
  Ok(resumed)
}

enum WorkerCycle<'a> {
  Again,
  Work {
    frame: &'a Rav1dFrameContext,
    task: Rav1dTask,
  },
}

fn prepare_worker_cycle<'a>(
  decoder: &'a Rav1dContext,
  worker: &Rav1dTaskContextTaskThread,
  scheduler: &mut Option<MutexGuard<'a, ()>>,
) -> Rav1dResult<WorkerCycle<'a>> {
  let control = &decoder.task_thread;

  match decoder.flush.load(Ordering::SeqCst).then_some(()) {
    Some(()) => {
      let held = scheduler.take().ok_or(EINVAL)?;
      let Ok(guard) = park_worker(decoder, worker, control, held);
      *scheduler = Some(guard);
      return Ok(WorkerCycle::Again);
    }
    None => {}
  }

  let _pending_frames = merge_pending(decoder)?;
  let Ok(grain) = control.delayed_fg_exec.get();

  match grain {
    0 => {}
    _ => {
      delayed_fg_task(control, scheduler)?;
      return Ok(WorkerCycle::Again);
    }
  }

  let selection = select_next_worker_task(decoder, control)?;
  let ScheduledTask {
    frame,
    index,
    previous,
  } = match selection {
    Some(task) => task,
    None => {
      match reset_task_cur(decoder, control, None) {
        Ok(ScanReset::Rewound) => return Ok(WorkerCycle::Again),
        Ok(ScanReset::Unchanged) => {}
      }

      let pending = merge_pending(decoder)?;

      match pending {
        PendingMerge::Merged => return Ok(WorkerCycle::Again),
        PendingMerge::Unchanged => {}
      }

      let held = scheduler.take().ok_or(EINVAL)?;
      let Ok(guard) = park_worker(decoder, worker, control, held);
      *scheduler = Some(guard);
      return Ok(WorkerCycle::Again);
    }
  };
  let removed = frame.task_thread.tasks.remove(index, previous)?;
  let task = match removed {
    Some(task) => task,
    None => {
      console_core_logging::debug!("Task {index:?} already consumed");
      return Ok(WorkerCycle::Again);
    }
  };

  match (task.type_0 > TaskType::InitCdf
    && frame.task_thread.tasks.head.load(Ordering::SeqCst) == Rav1dTaskIndex::None)
    .then_some(())
  {
    Some(()) => {
      let Ok(()) = control.cur.update(|cursor| cursor.wrapping_add(1));
    }
    None => {}
  }

  control.cond_signaled.store(1, Ordering::SeqCst);
  control.cond.notify_one();
  let held = scheduler.take().ok_or(EINVAL)?;
  drop(held);
  Ok(WorkerCycle::Work { frame, task })
}

enum WorkerRow<'scheduler> {
  Next(Rav1dTask),
  Complete(Rav1dTask),
  Scheduled(MutexGuard<'scheduler, ()>),
}

enum WorkerStage<'scheduler> {
  Next { task: Rav1dTask, stage: TaskType },
  Row(WorkerRow<'scheduler>),
}

fn prepare_worker_frame<'scheduler>(
  decoder: &'scheduler Rav1dContext,
  frame: &Rav1dFrameContext,
  mut task: Rav1dTask,
) -> Rav1dResult<WorkerRow<'scheduler>> {
  let Ok(pipeline) = tile_pipeline(decoder);

  match pipeline {
    FramePipeline::Parallel => {}
    FramePipeline::Serial => return Err(EINVAL),
  }

  let control = &decoder.task_thread;
  let initialized = rav1d_decode_frame_init(decoder, frame);
  let progress = input_cdf_progress(frame)?;

  Ok(match (initialized, progress) {
    (Err(error), _) => {
      let scheduler = control.lock.lock();
      let Ok(()) = abort_frame(decoder, frame, error);
      let Ok(_scan) = reset_task_cur(decoder, control, Some(task.frame_idx));
      WorkerRow::Scheduled(scheduler)
    }
    (Ok(()), InputCdfProgress::Failed) => {
      let scheduler = control.lock.lock();
      let Ok(()) = abort_frame(decoder, frame, EINVAL);
      let Ok(_scan) = reset_task_cur(decoder, control, Some(task.frame_idx));
      WorkerRow::Scheduled(scheduler)
    }
    (Ok(()), InputCdfProgress::Available) => {
      task.type_0 = TaskType::InitCdf;
      WorkerRow::Next(task)
    }
    (Ok(()), InputCdfProgress::Waiting) => {
      task.type_0 = TaskType::InitCdf;
      let Ok(()) = frame.task_thread.tasks.add_pending(task);
      WorkerRow::Scheduled(control.lock.lock())
    }
  })
}

fn initialize_worker_cdf<'scheduler>(
  decoder: &'scheduler Rav1dContext,
  frame: &Rav1dFrameContext,
  task: Rav1dTask,
) -> Rav1dResult<WorkerRow<'scheduler>> {
  let Ok(pipeline) = tile_pipeline(decoder);

  match pipeline {
    FramePipeline::Parallel => {}
    FramePipeline::Serial => return Err(EINVAL),
  }

  let outcome = attempt_frame_cdf(decoder, frame)?;
  let scheduler = match outcome {
    Ok(()) => {
      let queued = queue_frame_rows(decoder, frame)?;
      queued
    }
    Err(error) => {
      let control = &decoder.task_thread;
      let held = control.lock.lock();
      let Ok(()) = abort_frame(decoder, frame, error);
      let Ok(_scan) = reset_task_cur(decoder, control, Some(task.frame_idx));
      frame.task_thread.init_done.store(1, Ordering::SeqCst);
      held
    }
  };

  Ok(WorkerRow::Scheduled(scheduler))
}

fn resize_worker_row(
  decoder: &Rav1dContext,
  frame: &Rav1dFrameContext,
  worker: &mut Rav1dTaskContext,
  row: i32,
) -> Rav1dResult {
  let data = frame.data.try_read().ok_or(EINVAL)?;
  let header = data.frame_hdr.as_ref().ok_or(EINVAL)?;
  let [source, output] = header.size.width;

  match (source != output && frame.task_thread.error.load(Ordering::SeqCst) == 0).then_some(()) {
    Some(()) => {
      let Ok(depth) = data.bd_fn();

      match (depth.filter_sbrow_resize)(decoder, &data, worker, row) {
        Ok(()) => {}
        Err(_invalid_resize_window) => {
          let _previous_error = frame.task_thread.error.fetch_or(1, Ordering::SeqCst);
        }
      }
    }
    None => {}
  }

  Ok(())
}

fn restore_worker_row(
  decoder: &Rav1dContext,
  frame: &Rav1dFrameContext,
  worker: &mut Rav1dTaskContext,
  row: i32,
) -> Rav1dResult {
  let data = frame.data.try_read().ok_or(EINVAL)?;
  let restore = !data.lf.restore_planes.is_empty();

  match (frame.task_thread.error.load(Ordering::SeqCst) == 0 && restore).then_some(()) {
    Some(()) => {
      let Ok(depth) = data.bd_fn();

      match (depth.filter_sbrow_lr)(decoder, &data, worker, row) {
        Ok(()) => {}
        Err(_invalid_restoration_window) => {
          let _previous_error = frame.task_thread.error.fetch_or(1, Ordering::SeqCst);
        }
      }
    }
    None => {}
  }

  Ok(())
}

fn after_worker_filter(
  transition: WorkerTaskTransition<'_>,
  next: TaskType,
) -> Result<WorkerStage<'_>, console_core_never::Never> {
  Ok(match transition {
    WorkerTaskTransition::Continue(task) => WorkerStage::Next { task, stage: next },
    WorkerTaskTransition::Scheduled(scheduler) => WorkerStage::Row(WorkerRow::Scheduled(scheduler)),
  })
}

fn advance_worker_stage<'scheduler>(
  decoder: &'scheduler Rav1dContext,
  frame: &Rav1dFrameContext,
  worker: &mut Rav1dTaskContext,
  task: Rav1dTask,
  stage: TaskType,
  error: i32,
) -> Rav1dResult<WorkerStage<'scheduler>> {
  match stage {
    TaskType::Initial => prepare_worker_frame(decoder, frame, task).map(WorkerStage::Row),
    TaskType::InitCdf => initialize_worker_cdf(decoder, frame, task).map(WorkerStage::Row),
    TaskType::TileEntropy | TaskType::TileReconstruction => {
      let executed = decode_worker_tile_row(decoder, frame, worker, task, error)?;

      Ok(WorkerStage::Row(match executed {
        WorkerTaskTransition::Continue(next) => WorkerRow::Next(next),
        WorkerTaskTransition::Scheduled(scheduler) => WorkerRow::Scheduled(scheduler),
      }))
    }
    TaskType::DeblockCols => {
      let finished = complete_deblock_columns(decoder, frame, worker, task)?;
      let Ok(next) = after_worker_filter(finished, TaskType::DeblockRows);
      Ok(next)
    }
    TaskType::DeblockRows => {
      let finished = complete_deblock_rows(decoder, frame, worker, task)?;
      let Ok(next) = after_worker_filter(finished, TaskType::Cdef);
      Ok(next)
    }
    TaskType::Cdef => {
      let next = complete_worker_cdef(decoder, frame, worker, &task)?;
      Ok(WorkerStage::Next { task, stage: next })
    }
    TaskType::SuperResolution => {
      resize_worker_row(decoder, frame, worker, task.sby)?;
      Ok(WorkerStage::Next { task, stage: TaskType::LoopRestoration })
    }
    TaskType::LoopRestoration => {
      restore_worker_row(decoder, frame, worker, task.sby)?;
      Ok(WorkerStage::Next { task, stage: TaskType::ReconstructionProgress })
    }
    TaskType::ReconstructionProgress | TaskType::EntropyProgress => {
      Ok(WorkerStage::Row(WorkerRow::Complete(task)))
    }
  }
}

fn process_worker_stages<'scheduler>(
  decoder: &'scheduler Rav1dContext,
  frame: &Rav1dFrameContext,
  worker: &mut Rav1dTaskContext,
  mut task: Rav1dTask,
) -> Rav1dResult<WorkerRow<'scheduler>> {
  let Ok(stop_token) = decoder.stop_token();
  let stop = i32::from(
    stop_token.is_some_and(|token| matches!(token.state(), Ok(DecoderStopState::Requested))),
  );
  let flush = i32::from(decoder.flush.load(Ordering::SeqCst)) | stop;
  let error = frame.task_thread.error.fetch_or(flush, Ordering::SeqCst) | flush;
  let mut stage = task.type_0;

  for _stage in 0..WORKER_ROW_FILTER_STAGES {
    let advanced = advance_worker_stage(decoder, frame, worker, task, stage, error)?;

    match advanced {
      WorkerStage::Next { task: next, stage: next_stage } => {
        task = next;
        stage = next_stage;
      }
      WorkerStage::Row(finished) => return Ok(finished),
    }
  }

  Err(EINVAL)
}

fn process_worker_rows<'scheduler>(
  decoder: &'scheduler Rav1dContext,
  frame: &Rav1dFrameContext,
  worker: &mut Rav1dTaskContext,
  mut task: Rav1dTask,
) -> Rav1dResult<MutexGuard<'scheduler, ()>> {
  for _row in 0..=MAX_SUPERBLOCK_ROWS_PER_FRAME {
    let advanced = process_worker_stages(decoder, frame, worker, task)?;

    match advanced {
      WorkerRow::Next(next) => task = next,
      WorkerRow::Complete(finished) => return complete_worker_row(decoder, frame, &finished),
      WorkerRow::Scheduled(scheduler) => return Ok(scheduler),
    }
  }

  Err(EINVAL)
}

fn run_worker(decoder: &Rav1dContext, worker: &Rav1dTaskContextTaskThread) -> Rav1dResult {
  let scratch = crate::src::internal::TaskContextScratch::allocate()?;
  let Ok(mut task_context) = Rav1dTaskContext::new(scratch);
  let mut held = Some(decoder.task_thread.lock.lock());
  let lifetime = WorkerLifetime { signals: worker };

  for live_worker in lifetime {
    let cycle = prepare_worker_cycle(decoder, live_worker, &mut held)?;

    match cycle {
      WorkerCycle::Again => {}
      WorkerCycle::Work { frame, task } => {
        let scheduler = process_worker_rows(decoder, frame, &mut task_context, task)?;
        held = Some(scheduler);
      }
    }
  }

  let scheduler = held.take().ok_or(EINVAL)?;
  drop(scheduler);

  Ok(())
}

#[cfg(test)]
mod grain_panic_tests {
  use super::*;
  use std::sync::mpsc;
  use std::time::Duration;

  #[test]
  fn reservation_caches_finished_fault_and_preserves_unfinished_slot_on_worker_failure()
  -> Rav1dResult {
    let (Ok(completed), Ok(incomplete)) =
      (Rav1dFrameContext::default(0), Rav1dFrameContext::default(1));
    completed.task_thread.finished.store(true, Ordering::SeqCst);
    completed.task_thread.error.store(1, Ordering::SeqCst);

    {
      let mut output_fault = completed.task_thread.retval.try_lock().ok_or(EAGAIN)?;
      *output_fault = Some(EINVAL);
    }

    let decoder = Rav1dContext {
      fc: Box::new([completed, incomplete]),
      ..Rav1dContext::default()
    };
    let mut state = Rav1dState::default();
    state.frame_thread.out_delayed = Box::new([Default::default(), Default::default()]);

    {
      let frame = decoder.fc.first().ok_or(EINVAL)?;
      let _busy_result = frame.task_thread.retval.try_lock().ok_or(EINVAL)?;
      let busy = reserve_delayed_frame(&decoder, &mut state);
      assert!(matches!(busy, Err(EAGAIN)));
      assert_eq!(state.frame_thread.next, 0);
      assert_eq!(decoder.task_thread.first.load(Ordering::SeqCst), 0);
    }

    let retired = reserve_delayed_frame(&decoder, &mut state)?;
    assert_eq!(retired.index, 0);
    drop(retired);
    assert_eq!(state.cached_error, Some(EINVAL));
    assert_eq!(state.frame_thread.next, 1);
    assert_eq!(decoder.task_thread.first.load(Ordering::SeqCst), 1);

    decoder.task_thread.panicked.store(true, Ordering::SeqCst);
    let failed = reserve_delayed_frame(&decoder, &mut state);
    assert!(matches!(failed, Err(EGeneric)));
    assert_eq!(state.frame_thread.next, 1);
    assert_eq!(state.cached_error, Some(EINVAL));
    Ok(())
  }

  const TEST_WAIT: Duration = Duration::from_secs(5);

  fn wait_for_grain_test_work(control: &TaskThreadData) -> Rav1dResult<MutexGuard<'_, ()>> {
    let held = control.lock.lock();
    let (ready, timeout) = unpoison(control.cond.wait_timeout_while(held, TEST_WAIT, |_lock| {
      let Ok(active) = control.delayed_fg_exec.get();
      active == 0
    }));

    match timeout.timed_out().then_some(()) {
      Some(()) => {
        control.panicked.store(true, Ordering::SeqCst);
        control.delayed_fg_cond.notify_one();
        Err(EGeneric)
      }
      None => Ok(ready),
    }
  }

  fn run_grain_test_worker(control: &TaskThreadData) -> Rav1dResult {
    let ready = wait_for_grain_test_work(control)?;
    let mut scheduler = Some(ready);
    delayed_fg_task(control, &mut scheduler)?;
    drop(scheduler);
    Ok(())
  }

  #[test]
  fn failed_grain_preparation_returns_without_scheduling_rows() -> Rav1dResult {
    let decoder = Rav1dContext::default();
    let control = &decoder.task_thread;

    thread::scope(|scope| {
      let (report, outcome) = mpsc::sync_channel(1);
      let _scoped_worker = scope.spawn(move || {
        let result = run_grain_test_worker(control);
        let _test_caller_still_waiting = report.send(result);
      });
      let mut out = Rav1dPicture::default();
      #[cfg(feature = "bitdepth_8")]
      let bit_depth = 8;
      #[cfg(not(feature = "bitdepth_8"))]
      let bit_depth = 10;
      out.p.bpc = bit_depth;
      let rejected = rav1d_task_delayed_fg(&decoder, &mut out, &Rav1dPicture::default());
      let worker = outcome
        .recv_timeout(TEST_WAIT)
        .map_err(|_worker_still_running| EGeneric)?;
      worker?;
      assert_eq!(rejected, Err(EINVAL));
      let [submitted, completed] = &control.delayed_fg_progress;
      assert_eq!(submitted.load(Ordering::SeqCst), 0);
      assert_eq!(completed.load(Ordering::SeqCst), 0);
      assert!(!control.panicked.load(Ordering::SeqCst));
      Ok(())
    })
  }

  fn simulate_grain_worker_panic(
    control: &TaskThreadData,
    released: mpsc::Receiver<()>,
  ) -> Rav1dResult {
    let ready = wait_for_grain_test_work(control)?;
    let grain = control.delayed_fg.read();
    let [submitted, completed] = &control.delayed_fg_progress;
    submitted.store(3, Ordering::SeqCst);
    completed.store(1, Ordering::SeqCst);
    control.panicked.store(true, Ordering::SeqCst);
    control.delayed_fg_cond.notify_one();
    drop(ready);
    released
      .recv_timeout(TEST_WAIT)
      .map_err(|_caller_still_waiting| EGeneric)?;
    assert_eq!(grain.out.p.bpc, 8);
    Ok(())
  }

  #[test]
  fn delayed_grain_panic_preserves_live_worker_state() -> Rav1dResult {
    let decoder = Rav1dContext::default();
    let control = &decoder.task_thread;

    thread::scope(|scope| {
      let (release, released) = mpsc::sync_channel(1);
      let (report, outcome) = mpsc::sync_channel(1);
      let _scoped_worker = scope.spawn(move || {
        let result = simulate_grain_worker_panic(control, released);
        let _test_caller_still_waiting = report.send(result);
      });
      let mut out = Rav1dPicture::default();
      out.p.bpc = 8;
      rav1d_task_delayed_fg(&decoder, &mut out, &Rav1dPicture::default())?;
      assert!(control.panicked.load(Ordering::SeqCst));
      let Ok(active) = control.delayed_fg_exec.get();
      assert_eq!(active, 0);
      let [submitted, completed] = &control.delayed_fg_progress;
      assert_eq!(submitted.load(Ordering::SeqCst), 3);
      assert_eq!(completed.load(Ordering::SeqCst), 1);
      release.send(()).map_err(|_reader_exited| EGeneric)?;
      let worker = outcome
        .recv_timeout(TEST_WAIT)
        .map_err(|_worker_still_running| EGeneric)?;
      worker
    })
  }
}
