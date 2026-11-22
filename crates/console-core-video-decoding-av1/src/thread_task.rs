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
use crate::src::error::Rav1dError::{EAGAIN, EGeneric, EINVAL};
use crate::src::error::Rav1dError::ENOMEM;
use crate::src::error::Rav1dResult;
use crate::src::fg_apply::rav1d_apply_grain_row;
use crate::src::fg_apply::rav1d_prep_grain;
use crate::src::filmgrain::FG_BLOCK_SIZE;
use crate::src::internal::Grain;
use crate::src::internal::Rav1dBitDepthDSPContext;
use crate::src::internal::Rav1dContext;
use crate::src::internal::Rav1dFrameContext;
use crate::src::internal::Rav1dFrameContextTaskThread;
use crate::src::internal::Rav1dFrameData;
use crate::src::internal::Rav1dState;
use crate::src::internal::Rav1dTask;
use crate::src::internal::Rav1dTaskContext;
use crate::src::internal::{Rav1dTaskContextTaskThread, Rav1dTileState};
use crate::src::internal::TaskThreadData;
use crate::src::internal::DelayedGrainStage;
use crate::src::internal::TaskType;
use crate::src::iter::wrapping_iter;
use crate::src::relaxed_atomic::RelaxedAtomic;
use atomig::Atom;
use atomig::Atomic;
use console_core_synchronization::nonpoison::{
  Mutex, MutexGuard, RwLock, RwLockReadGuard, unpoison, wait_while,
};
use std::cmp;
use std::ffi::c_int;
use std::ffi::c_uint;
use std::mem;
use std::num::NonZeroU32;
use std::ops::Add;
use std::ops::AddAssign;
#[cfg(test)]
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicI32;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;
#[cfg(test)]
use std::thread;

pub const FRAME_ERROR: u32 = u32::MAX - 1;
pub const TILE_ERROR: i32 = i32::MAX - 1;

pub(crate) fn advance_completed_frame(control: &TaskThreadData, count: u32) -> Result<(), console_core_never::Never> {
  let first = control.first.load(Ordering::SeqCst);

  match first.wrapping_add(1).cmp(&count) {
    std::cmp::Ordering::Less => {
      let _previous_first = control.first.fetch_add(1, Ordering::SeqCst);
    }
    std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => control.first.store(0, Ordering::SeqCst),
  }

  let expected = NonZeroU32::new(first.wrapping_add(1));
  let _reset_superseded_by_another_task = control.reset_task_cur.compare_exchange(
    expected,
    None,
    Ordering::SeqCst,
    Ordering::SeqCst,
  );
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

  match ::core::primitive::bool::then_some(frame.task_thread.finished.load(Ordering::SeqCst), ()) {
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
  let delayed = state.frame_thread.out_delayed.get_mut(index).ok_or(EINVAL)?;
  let mut result = frame.task_thread.retval.try_lock().ok_or(EAGAIN)?;
  let output = match (*result, &delayed.p.data) {
    (Some(error), _) => DelayedFrameOutput::Error(error),
    (None, None) => DelayedFrameOutput::Empty,
    (None, Some(_)) => {
      let [_, final_progress] = delayed.progress.as_deref().ok_or(EINVAL)?;
      let visible = delayed.visible || decoder.output_invisible_frames;

      match (visible && final_progress.load(Ordering::Relaxed) != FRAME_ERROR).then_some(()) {
        Some(()) => DelayedFrameOutput::Display,
        None => DelayedFrameOutput::Discard,
      }
    }
  };
  let had_pending_output = delayed.p.data.is_some() || frame.task_thread.error.load(Ordering::SeqCst) != 0;

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
  let following = next.wrapping_add(1).checked_rem(frame_count.get()).ok_or(EINVAL)?;
  let scheduler = wait_for_frame_slot(decoder, frame)?;
  retire_delayed_output(decoder, state, frame, next, frame_count)?;
  state.frame_thread.next = following;
  Ok(FrameReservation { frame, index: next, scheduler })
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

fn clear_scan_positions(c: &Rav1dContext, start: u64) -> Result<ScanReset, console_core_never::Never> {
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
  let pending = ttd.reset_task_cur.swap(None, Ordering::SeqCst)
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
      Some(frame) if frame <= current_frame => {
        let offset = frame.wrapping_sub(first);
        let Ok(()) = ttd.cur.set(offset);
        let start = u64::from(first).wrapping_add(u64::from(offset));
        return clear_scan_positions(c, start);
      }
      Some(_) | None => return Ok(ScanReset::Unchanged),
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
  let active = match ttd.reset_task_cur.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |previous| {
    match previous {
      Some(earlier) if earlier <= encoded => None,
      Some(_) | None => Some(Some(encoded)),
    }
  }) {
    Ok(_) => encoded,
    Err(Some(earlier)) => earlier,
    Err(None) => return Ok(()),
  };

  match (active.get().wrapping_sub(1) == first && ttd.first.load(Ordering::SeqCst) != first).then_some(()) {
    Some(()) => {
      let _superseded = ttd.reset_task_cur.compare_exchange(Some(active), None, Ordering::SeqCst, Ordering::SeqCst);
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
    Self { row: task.sby, kind: task.type_0, tile: task.tile_idx }
  }
}

impl QueueOrder {
  fn compare_insertion(&self, existing: &Self) -> Result<cmp::Ordering, console_core_never::Never> {
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
        assert!(matches!(self.kind, TaskType::TileEntropy | TaskType::TileReconstruction));
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
  tasks.try_reserve(extra).map_err(|_allocation| ENOMEM)?;
  let Ok(keep) = console_core_number_conversion::index(retained);
  tasks.truncate(keep);
  tasks.append(pending);
  Ok(start..end)
}

impl Rav1dTasks {
  fn insert_tasks_between(&self, c: &Rav1dContext, insertion: TaskInsertion, wake: QueueWake) -> Rav1dResult {
    let ttd = &*c.task_thread;

    match c.flush.load(Ordering::SeqCst).then_some(()) {
      Some(()) => return Ok(()),
      None => {}
    }

    let TaskInsertion { first, last, previous, following } = insertion;
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
          let insertion = TaskInsertion { first, last, previous, following };
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

  pub fn remove(&self, position: Rav1dTaskIndex, previous: Rav1dTaskIndex) -> Rav1dResult<Option<Rav1dTask>> {
    let tasks = self.tasks.try_read().ok_or(EINVAL)?;
    let task = queue_entry(&tasks, position)?;
    let Ok(next) = task.next();
    let result = match previous {
      Rav1dTaskIndex::None => self.head.compare_exchange(position, next, Ordering::SeqCst, Ordering::SeqCst),
      _ => {
        let predecessor = queue_entry(&tasks, previous)?;
        predecessor.next.compare_exchange(position, next, Ordering::SeqCst, Ordering::SeqCst)
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
        let ready = stage_pending_tasks(&mut tasks, &mut pending, head)
          .inspect_err(|_failed_to_stage| self.pending_tasks_merge.store(true, Ordering::SeqCst))?;
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
  fn insert_task(&self, c: &Rav1dContext, task: Rav1dTask, wake: QueueWake) -> Rav1dResult<Rav1dTaskIndex> {
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
  fn state_slot<T>(self, [reconstruction, entropy]: &[T; 2]) -> Result<&T, console_core_never::Never> {
    Ok(match self {
      Self::Entropy => entropy,
      Self::Reconstruction => reconstruction,
    })
  }

  fn picture_progress<T>(self, [entropy, reconstruction]: &[T; 2]) -> Result<&T, console_core_never::Never> {
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
      frame.frame_thread_progress.entropy.store(0, Ordering::Relaxed);
      TaskType::EntropyProgress
    }
    TileQueueStage::Reconstruction => {
      let row_count = u32::try_from(data.sbh).map_err(|_invalid_height| EINVAL)?;
      let words = row_count.div_ceil(u32::BITS);
      let Ok(length) = console_core_number_conversion::index(words);
      let mut picture_rows = frame.frame_thread_progress.frame.try_write().ok_or(EINVAL)?;
      picture_rows.clear();
      picture_rows.try_resize_with(length, Default::default).map_err(|_allocation| ENOMEM)?;

      let mut copied_rows = frame.frame_thread_progress.copy_lpf.write();
      copied_rows.clear();
      copied_rows.try_resize_with(length, Default::default).map_err(|_allocation| ENOMEM)?;
      frame.frame_thread_progress.deblock.store(0, Ordering::SeqCst);

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
    let new_tasks = tile_count.checked_add(1).ok_or(ENOMEM)?;
    pending.try_reserve(new_tasks).map_err(|_allocation| ENOMEM)?;
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
    let ttd = &*c.task_thread;

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

    let rows_finished = completed.stage == DelayedGrainStage::Apply
      && !completed.row_failed.load(Ordering::SeqCst);

    match rows_finished.then_some(()) {
      Some(()) => {}
      None => return Err(EINVAL),
    }
  };

  Ok(())
}

#[inline]
fn ensure_progress<'l, 'ttd: 'l>(
  ttd: &'ttd TaskThreadData,
  f: &Rav1dFrameContext,
  t: &Rav1dTask,
  type_0: TaskType,
  state: &AtomicI32,
  task_thread_lock: &'l mut Option<MutexGuard<'ttd, ()>>,
) -> core::result::Result<c_int, console_core_never::Never> {
  let p1 = state.load(Ordering::SeqCst);

  match ::core::primitive::bool::then_some(p1 < t.sby, ()) {
    ::core::option::Option::Some(()) => {
      let t_2 = Rav1dTask {
        type_0,
        recon_progress: 0,
        deblock_progress: t.sby,
        ..({
          let Ok(explicit_value) = t.without_next();
          explicit_value
        })
      };
      let Ok(()) = f.task_thread.tasks.add_pending(t_2);
      *task_thread_lock = Some(ttd.lock.lock());
      return Ok({
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(1);
        numeric_value
      });
    }
    ::core::option::Option::None => {}
  }

  return Ok({
    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(0);
    numeric_value
  });
}

#[derive(Clone, Copy)]
enum TileStage {
  Entropy,
  Reconstruction,
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
  stage: TileStage,
) -> Rav1dResult<TileReadiness> {
  let (block_bottom, lowest_rows) = match stage {
    TileStage::Entropy => {
      let shift = u32::try_from(f.sb_shift).map_err(|_invalid_shift| EINVAL)?;
      let bottom = task.sby.wrapping_add(1).wrapping_shl(shift.wrapping_add(2));
      let checked_value = u32::try_from(bottom).map_err(|_outside_frame| EINVAL)?;
      (checked_value, None)
    }
    TileStage::Reconstruction => {
      let shift = u32::try_from(f.sb_shift).map_err(|_invalid_shift| EINVAL)?;
      let row = task.sby.wrapping_sub(tile.tiling.row_start.wrapping_shr(shift));
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
        let y = match luma { i32::MIN => i32::MIN, _ => luma.wrapping_add(8) };
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
        let [motion, picture] = reference.progress.as_deref().ok_or(EINVAL)?;
        let producer = match stage { TileStage::Entropy => motion, TileStage::Reconstruction => picture };
        let progress = producer.load(Ordering::SeqCst);

        match {
            ::core::primitive::bool::then_some(progress < required, ())
        } {
            ::core::option::Option::Some(()) => {
              return Ok(TileReadiness::Waiting);
            },
            ::core::option::Option::None => {},
        }

        let _previous_error = task_thread.error.fetch_or(i32::from(progress == FRAME_ERROR), Ordering::SeqCst);
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
    TaskType::TileEntropy => TileStage::Entropy,
    TaskType::TileReconstruction => TileStage::Reconstruction,
    _ => return Err(EINVAL),
  };
  let tile = f.tile(task.tile_idx).ok_or(EINVAL)?;
  let [reconstruction, entropy] = &tile.progress;
  let first = match stage {
    TileStage::Entropy => entropy,
    TileStage::Reconstruction => reconstruction,
  }.load(Ordering::SeqCst);

  match {
      ::core::primitive::bool::then_some(first < task.sby, ())
  } {
      ::core::option::Option::Some(()) => {
        return Ok(TileReadiness::Waiting);
      },
      ::core::option::Option::None => {},
  }

  let tile_error = i32::from(first == TILE_ERROR);
  let mut error = task_thread.error.fetch_or(tile_error, Ordering::SeqCst) | tile_error;

  match (error, pipeline, stage) {
    (0, FramePipeline::Parallel, TileStage::Reconstruction) => {
      let second = entropy.load(Ordering::SeqCst);

      match {
          ::core::primitive::bool::then_some(second <= task.sby, ())
      } {
          ::core::option::Option::Some(()) => {
            return Ok(TileReadiness::Waiting);
          },
          ::core::option::Option::None => {},
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
    Ok(Self { word, mask: 1_u32.wrapping_shl(position & 31) })
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
  let row_progress = match f.sr_cur.progress.as_deref() {
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
  let first_incomplete = progress_map.iter().zip(0_u32..).skip(skip_words).find_map(|(entry, word)| {
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
  let Ok(first_pending) = console_core_number_conversion::wrapped::<_, i32>(word.wrapping_shl(5) | bit);
  Ok(first_pending.wrapping_sub(1))
}

#[inline]
fn abort_frame(
  c: &Rav1dContext,
  fc: &Rav1dFrameContext,
  error: Rav1dResult,
) -> core::result::Result<(), console_core_never::Never> {
  {
    fc.task_thread.error.store(
      match ::core::primitive::bool::then_some(error == Err(EINVAL), ()) {
        ::core::option::Option::Some(()) => 1,
        ::core::option::Option::None => -1,
      },
      Ordering::SeqCst,
    );
    fc.task_thread.task_counter.store(0, Ordering::SeqCst);
    (*match &fc.task_thread.done {
      [element, ..] => element,
    })
    .store(1, Ordering::SeqCst);
    (*match &fc.task_thread.done {
      [_, element, ..] => element,
    })
    .store(1, Ordering::SeqCst);

    {
      {
          match fc.data.try_read() {
              Some(frame) => {
                match frame.sr_cur.progress.as_deref() {
                  Some([first, last]) => {
                    first.store(FRAME_ERROR, Ordering::SeqCst);
                    last.store(FRAME_ERROR, Ordering::SeqCst);
                  }
                  None => {}
                }
              },
              ::core::option::Option::None => {},
          };
      }
    }

    let _ = rav1d_decode_frame_exit(c, fc, error);
    fc.task_thread.cond.notify_one();
  };

  Ok(())
}

#[inline]
fn delayed_fg_task<'l, 'ttd: 'l>(
  ttd: &'ttd TaskThreadData,
  task_thread_lock: &'l mut Option<MutexGuard<'ttd, ()>>,
) -> Rav1dResult {
  {
    let delayed_fg_type = ttd.delayed_fg.try_read().ok_or(EINVAL)?.stage;
    let mut row;
    let mut progmax;
    let mut done;

    match delayed_fg_type {
      DelayedGrainStage::Prepare => {
        let Ok(()) = ttd.delayed_fg_exec.set(0);

        match ::core::primitive::bool::then_some(ttd.cond_signaled.load(Ordering::SeqCst) != 0, ()) {
          ::core::option::Option::Some(()) => {
            ttd.cond.notify_one();
          }
          ::core::option::Option::None => {}
        }

        let delayed_fg = &mut *ttd.delayed_fg.try_write().ok_or(EINVAL)?;
        let Ok(dsp) = Rav1dBitDepthDSPContext::get(delayed_fg.out.p.bpc);
        let prepared = match dsp {
          Some(depth) => match &mut delayed_fg.grain {
            #[cfg(feature = "bitdepth_8")]
            Grain::Bpc8(grain) => rav1d_prep_grain::<BitDepth8>(&depth.fg, &mut delayed_fg.out, &delayed_fg.in_0, grain),
            #[cfg(feature = "bitdepth_16")]
            Grain::Bpc16(grain) => rav1d_prep_grain::<BitDepth16>(&depth.fg, &mut delayed_fg.out, &delayed_fg.in_0, grain),
          },
          None => Err(EINVAL),
        };

        match prepared {
          Ok(()) => {}
          Err(_invalid_film_grain) => {
            ttd.delayed_fg_cond.notify_one();
            return Ok(());
          }
        }

        delayed_fg.stage = DelayedGrainStage::Apply;
        let Ok(()) = ttd.delayed_fg_exec.set(1);
      }
      DelayedGrainStage::Apply => {}
      DelayedGrainStage::Idle => return Err(EINVAL),
    }

    row = (*match &ttd.delayed_fg_progress {
      [element, ..] => element,
    })
    .fetch_add(1, Ordering::SeqCst);
    let _ = task_thread_lock.take();
    let delayed_fg = ttd.delayed_fg.try_read().ok_or(EINVAL)?;
    progmax = i32::wrapping_sub(
      i32::wrapping_add(delayed_fg.out.p.h, {
        let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(FG_BLOCK_SIZE);
        numeric_value
      }),
      1,
    ) / ({
      let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(FG_BLOCK_SIZE);
      numeric_value
    });

    loop {
      match ::core::primitive::bool::then_some(i32::wrapping_add(row, 1) < progmax, ()) {
        ::core::option::Option::Some(()) => {
          ttd.cond.notify_one();
        }
        ::core::option::Option::None => match ::core::primitive::bool::then_some(i32::wrapping_add(row, 1) >= progmax, ()) {
          ::core::option::Option::Some(()) => {
            *task_thread_lock = Some(ttd.lock.lock());
            let Ok(()) = ttd.delayed_fg_exec.set(0);

            match ::core::primitive::bool::then_some(row >= progmax, ()) {
              ::core::option::Option::Some(()) => {
                break;
              }
              ::core::option::Option::None => {}
            }

            let _ = task_thread_lock.take();
          }
          ::core::option::Option::None => {}
        },
      }

      let Ok(dsp) = Rav1dBitDepthDSPContext::get(delayed_fg.out.p.bpc);
      let applied = match (dsp, u32::try_from(row)) {
        (Some(depth), Ok(position)) => match &delayed_fg.grain {
          #[cfg(feature = "bitdepth_8")]
          Grain::Bpc8(grain) => rav1d_apply_grain_row::<BitDepth8>(
            &depth.fg, &delayed_fg.out, &delayed_fg.in_0, grain, position,
          ),
          #[cfg(feature = "bitdepth_16")]
          Grain::Bpc16(grain) => rav1d_apply_grain_row::<BitDepth16>(
            &depth.fg, &delayed_fg.out, &delayed_fg.in_0, grain, position,
          ),
        },
        (None, _) | (Some(_), Err(_)) => Err(EINVAL),
      };

      match applied {
        Ok(()) => {}
        Err(_invalid_row) => delayed_fg.row_failed.store(true, Ordering::SeqCst),
      }

      row = (*match &ttd.delayed_fg_progress {
        [element, ..] => element,
      })
      .fetch_add(1, Ordering::SeqCst);

      {
        done = i32::wrapping_add(
          (*match &ttd.delayed_fg_progress {
            [_, element, ..] => element,
          })
          .fetch_add(1, Ordering::SeqCst),
          1,
        );
      }

      match ::core::primitive::bool::then_some(row < progmax, ()) {
        ::core::option::Option::Some(()) => {
          continue;
        }
        ::core::option::Option::None => {}
      }

      *task_thread_lock = Some(ttd.lock.lock());
      let Ok(()) = ttd.delayed_fg_exec.set(0);
      break;
    }

    done = i32::wrapping_add(
      (*match &ttd.delayed_fg_progress {
        [_, element, ..] => element,
      })
      .fetch_add(1, Ordering::SeqCst),
      1,
    );
    progmax = (*match &ttd.delayed_fg_progress {
      [element, ..] => element,
    })
    .load(Ordering::SeqCst);

    match ::core::primitive::bool::then_some(!(done < progmax), ()) {
      ::core::option::Option::Some(()) => {
        ttd.delayed_fg_cond.notify_one();
      }
      ::core::option::Option::None => {}
    }
  };

  Ok(())
}

struct WorkerPanicGuard<'a> {
  c: &'a Rav1dContext,
  thread_data: &'a Rav1dTaskContextTaskThread,
}

impl WorkerPanicGuard<'_> {
  fn notify_failure(&self) -> Result<(), console_core_never::Never> {
    let ttd = &*self.c.task_thread;

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
    let failure_guard = WorkerPanicGuard { c, thread_data: worker };

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

fn run_worker(c: &Rav1dContext, worker: &Rav1dTaskContextTaskThread) -> Rav1dResult {
  {
    let scratch = crate::src::internal::TaskContextScratch::allocate()?;
    let Ok(mut tc) = Rav1dTaskContext::new(scratch);
    let ttd = &*c.task_thread;

    fn park<'ttd>(
      c: &Rav1dContext,
      worker: &Rav1dTaskContextTaskThread,
      ttd: &TaskThreadData,
      task_thread_lock: MutexGuard<'ttd, ()>,
    ) -> core::result::Result<MutexGuard<'ttd, ()>, console_core_never::Never> {
      let Ok(()) = worker.flushed.set(true);
      worker.cond.notify_one();

      ttd.cond_signaled.store(0, Ordering::SeqCst);


      let task_thread_lock = unpoison(ttd.cond.wait(task_thread_lock));

      let Ok(()) = worker.flushed.set(false);
      let Ok(_) = reset_task_cur(c, ttd, None);

      Ok(task_thread_lock)
    }

    let mut task_thread_lock = Some(ttd.lock.lock());

    'outer: while !({
      let Ok(explicit_value) = worker.die.get();
      explicit_value
    }) {
      match ::core::primitive::bool::then_some(c.flush.load(Ordering::SeqCst), ()) {
        ::core::option::Option::Some(()) => {
          let held = task_thread_lock.take().ok_or(EINVAL)?;
          let Ok(guard) = park(c, worker, ttd, held);
          task_thread_lock = Some(guard);
          continue 'outer;
        }
        ::core::option::Option::None => {}
      }

      let _outcome = merge_pending(c)?;

      let Ok(delayed_fg_exec) = ttd.delayed_fg_exec.get();

      match ::core::primitive::bool::then_some(delayed_fg_exec != 0, ()) {
        ::core::option::Option::Some(()) => {
          delayed_fg_task(ttd, &mut task_thread_lock)?;
          continue 'outer;
        }
        ::core::option::Option::None => {}
      }

      let (fc, t_idx, prev_t) = 'found: {
        match ::core::primitive::bool::then_some(c.fc.len() > 1, ()) {
          ::core::option::Option::Some(()) => {
            let first = u64::from(ttd.first.load(Ordering::SeqCst));
            let Ok(frames) = wrapping_iter(c.fc.iter(), first);

            'init_tasks: for fc in frames {
              let tasks = &fc.task_thread.tasks;

              match ::core::primitive::bool::then_some(fc.task_thread.init_done.load(Ordering::SeqCst) != 0, ()) {
                ::core::option::Option::Some(()) => {
                  continue 'init_tasks;
                }
                ::core::option::Option::None => {}
              }

              let t_idx = tasks.head.load(Ordering::SeqCst);

              match ::core::primitive::bool::then_some(t_idx == Rav1dTaskIndex::None, ()) {
                ::core::option::Option::Some(()) => {
                  continue 'init_tasks;
                }
                ::core::option::Option::None => {}
              }

              let task_view = tasks.read(t_idx)?;
              let t = task_view.entry()?;

              match ::core::primitive::bool::then_some(t.type_0 == TaskType::Init, ()) {
                ::core::option::Option::Some(()) => {
                  break 'found (fc, t_idx, Rav1dTaskIndex::None);
                }
                ::core::option::Option::None => {}
              }

              match ::core::primitive::bool::then_some(t.type_0 == TaskType::InitCdf, ()) {
                ::core::option::Option::Some(()) => {
                  let in_cdf = fc.in_cdf()?;
                  let Ok(progress) = in_cdf.progress();
                  let Ok(p1) =
                    console_core_number_conversion::wrapped::<_, i32>(match progress {
                      Some(progress_2) => progress_2.load(Ordering::SeqCst),
                      None => {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, u32>({
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, i32>(1);
                            numeric_value
                          });
                        numeric_value
                      }
                    });

                  match ::core::primitive::bool::then_some(p1 != 0, ()) {
                    ::core::option::Option::Some(()) => {
                      fc.task_thread
                        .error
                        .fetch_or(i32::from(p1 == TILE_ERROR), Ordering::SeqCst);
                      break 'found (fc, t_idx, Rav1dTaskIndex::None);
                    }
                    ::core::option::Option::None => {}
                  }
                }
                ::core::option::Option::None => {}
              }
            }
          }
          ::core::option::Option::None => {}
        }

        while ({
          let Ok(numeric_value) = console_core_number_conversion::wrapped_index({
            let Ok(explicit_value) = ttd.cur.get();
            explicit_value
          });
          numeric_value
        }) < c.fc.len()
        {
          let first = ttd.first.load(Ordering::SeqCst);
          let Ok(ttd_cur) = ttd.cur.get();
          let fc = &c.fc[({
            let Ok(numeric_value) =
              console_core_number_conversion::wrapped_index(u32::wrapping_add(first, ttd_cur));
            numeric_value
          }) % c.fc.len()];
          let tasks = &fc.task_thread.tasks;
          let _merge_outcome = tasks.merge_pending_frame(c)?;
          let Ok(mut prev_t) = tasks.cur_prev.get();
          let mut t_idx = match ::core::primitive::bool::then_some(prev_t != Rav1dTaskIndex::None, ()) {
            ::core::option::Option::Some(()) => {
              let position = tasks.read(prev_t)?;
              let entry = position.entry()?;
              let Ok(next) = entry.next();

              next
            }
            ::core::option::Option::None => tasks.head.load(Ordering::SeqCst),
          };

          while t_idx != Rav1dTaskIndex::None {
            let task_view = tasks.read(t_idx)?;
              let t = task_view.entry()?;

            'next: {
              match ::core::primitive::bool::then_some(t.type_0 == TaskType::InitCdf, ()) {
                ::core::option::Option::Some(()) => {
                  break 'next;
                }
                ::core::option::Option::None => {}
              }

              match t.type_0 {
                TaskType::TileEntropy | TaskType::TileReconstruction => {
                  let f = fc.data.read();

                  let Ok(pipeline) = tile_pipeline(c);
                  let Ok(readiness) = check_tile_or_record_error(&f, &fc.task_thread, &t, pipeline);

                  match readiness {
                    TileReadiness::Ready => break 'found (fc, t_idx, prev_t),
                    TileReadiness::Waiting => {}
                  }
                }
                _ => match ::core::primitive::bool::then_some(t.recon_progress != 0, ()) {
                  ::core::option::Option::Some(()) => {
                    let f = fc.data.read();
                    let p = t.type_0 == TaskType::EntropyProgress;
                    let error = fc.task_thread.error.load(Ordering::SeqCst);
                    let done =
                      fc.task_thread.done[match ::console_core_number_conversion::index::<
                        ::core::primitive::u8,
                      >(
                        ::core::primitive::u8::from(p)
                      ) {
                        ::core::result::Result::Ok(converted_position) => converted_position,
                      }]
                      .load(Ordering::SeqCst);
                    assert!(done == 0 || error != 0, "done: {done}, error: {error}");
                    let frame_hdr = f.frame_hdr()?;
                    let Ok(cols) =
                      console_core_number_conversion::wrapped::<_, i32>(frame_hdr.tiling.cols);
                    let Ok(next_tile_row) = f.frame_thread.next_tile_row
                      [match ::console_core_number_conversion::index::<::core::primitive::u8>(
                        ::core::primitive::u8::from(p),
                      ) {
                        ::core::result::Result::Ok(converted_position) => converted_position,
                      }]
                    .get();
                    let tile_row_base = i32::wrapping_mul(cols, next_tile_row);

                    match ::core::primitive::bool::then_some(p, ()) {
                      ::core::option::Option::Some(()) => {
                        let p1_0 = fc.frame_thread_progress.entropy.load(Ordering::SeqCst);

                        match ::core::primitive::bool::then_some(p1_0 < t.sby, ()) {
                          ::core::option::Option::Some(()) => {
                            break 'next;
                          }
                          ::core::option::Option::None => {}
                        }

                        fc.task_thread
                          .error
                          .fetch_or(i32::from(p1_0 == TILE_ERROR), Ordering::SeqCst);
                      }
                      ::core::option::Option::None => {}
                    }

                    for tc_0 in 0..frame_hdr.tiling.cols {
                      let ts = &f.ts[{
                        let Ok(numeric_value) = console_core_number_conversion::wrapped_index(
                          i32::wrapping_add(tile_row_base, {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, i32>(tc_0);
                            numeric_value
                          }),
                        );
                        numeric_value
                      }];
                      let p2 = ts.progress[match ::console_core_number_conversion::index::<
                        ::core::primitive::u8,
                      >(
                        ::core::primitive::u8::from(p)
                      ) {
                        ::core::result::Result::Ok(converted_position) => converted_position,
                      }]
                      .load(Ordering::SeqCst);

                      match ::core::primitive::bool::then_some(p2 < t.recon_progress, ()) {
                        ::core::option::Option::Some(()) => {
                          break 'next;
                        }
                        ::core::option::Option::None => {}
                      }

                      fc.task_thread
                        .error
                        .fetch_or(i32::from(p2 == TILE_ERROR), Ordering::SeqCst);
                    }

                    match ::core::primitive::bool::then_some(i32::wrapping_add(t.sby, 1) < f.sbh, ()) {
                      ::core::option::Option::Some(()) => {
                        let next_t = Rav1dTask {
                          sby: i32::wrapping_add(t.sby, 1),
                          recon_progress: i32::wrapping_add(t.sby, 2),
                          ..({
                            let Ok(explicit_value) = t.without_next();
                            explicit_value
                          })
                        };
                        let Ok(f_next_tile_row) = f.frame_thread.next_tile_row
                          [match ::console_core_number_conversion::index::<::core::primitive::u8>(
                            ::core::primitive::u8::from(p),
                          ) {
                            ::core::result::Result::Ok(converted_position) => converted_position,
                          }]
                        .get();
                        let ntr = i32::wrapping_add(f_next_tile_row, 1);
                        let Ok(start) = console_core_number_conversion::wrapped::<_, i32>(
                          frame_hdr.tiling.row_start_sb[{
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped_index(ntr);
                            numeric_value
                          }],
                        );

                        match ::core::primitive::bool::then_some(next_t.sby == start, ()) {
                          ::core::option::Option::Some(()) => {
                            let Ok(()) = f.frame_thread.next_tile_row
                              [match ::console_core_number_conversion::index::<
                                ::core::primitive::u8,
                              >(::core::primitive::u8::from(
                                p,
                              )) {
                                ::core::result::Result::Ok(converted_position) => {
                                  converted_position
                                }
                              }]
                            .set(ntr);
                          }
                          ::core::option::Option::None => {}
                        }

                        drop(task_view);
                        let _queued = fc.task_thread.insert_task(c, next_t, QueueWake::KeepParked)?;
                      }
                      ::core::option::Option::None => {}
                    }

                    break 'found (fc, t_idx, prev_t);
                  }
                  ::core::option::Option::None => match ::core::primitive::bool::then_some(t.type_0 == TaskType::Cdef, ()) {
                    ::core::option::Option::Some(()) => {
                      let copied = fc.frame_thread_progress.copy_lpf.try_read().ok_or(EINVAL)?;
                      let previous = ProgressBit::at(&copied, t.sby.wrapping_sub(1))?;
                      let Ok(completion) = previous.load();

                      match completion {
                        RowCompletion::Complete => break 'found (fc, t_idx, prev_t),
                        RowCompletion::Pending => {}
                      }
                    }
                    ::core::option::Option::None => {
                      match ::core::primitive::bool::then_some(t.deblock_progress == 0, ()) {
                        ::core::option::Option::Some(()) => {
                          return Err(EINVAL);
                        }
                        ::core::option::Option::None => {}
                      }

                      let p1_2 = fc.frame_thread_progress.deblock.load(Ordering::SeqCst);

                      match ::core::primitive::bool::then_some(p1_2 >= t.deblock_progress, ()) {
                        ::core::option::Option::Some(()) => {
                          fc.task_thread
                            .error
                            .fetch_or(i32::from(p1_2 == TILE_ERROR), Ordering::SeqCst);
                          break 'found (fc, t_idx, prev_t);
                        }
                        ::core::option::Option::None => {}
                      }
                    }
                  },
                },
              }
            }

            prev_t = t_idx;
            let Ok(next) = t.next();
            t_idx = next;
            let Ok(()) = tasks.cur_prev.set(prev_t);
          }

          let Ok(()) = ttd.cur.update(|cur| u32::wrapping_add(cur, 1));
        }

        match reset_task_cur(c, ttd, None) {
          Ok(ScanReset::Rewound) => continue 'outer,
          Ok(ScanReset::Unchanged) => {}
        }

        let pending = merge_pending(c)?;

        match pending {
          PendingMerge::Merged => continue 'outer,
          PendingMerge::Unchanged => {}
        }

        let held = task_thread_lock.take().ok_or(EINVAL)?;
          let Ok(guard) = park(c, worker, ttd, held);
        task_thread_lock = Some(guard);
        continue 'outer;
      };

      let remove = fc.task_thread.tasks.remove(t_idx, prev_t)?;
      let mut t = match remove {
        Some(t) => t,
        None => {
          console_core_logging::debug!("Task {t_idx:?} already consumed");
          continue 'outer;
        }
      };

      match ::core::primitive::bool::then_some(t.type_0 > TaskType::InitCdf
        && fc.task_thread.tasks.head.load(Ordering::SeqCst) == Rav1dTaskIndex::None, ())
      {
        ::core::option::Option::Some(()) => {
          let Ok(()) = ttd.cur.update(|cur| u32::wrapping_add(cur, 1));
        }
        ::core::option::Option::None => {}
      }

      ttd.cond_signaled.store(1, Ordering::SeqCst);
      ttd.cond.notify_one();
      let held = task_thread_lock.take().ok_or(EINVAL)?;
      drop(held);

      'found_unlocked: loop {
        let Ok(stop_token) = c.stop_token();
        let stop = i32::from(stop_token.is_some_and(|s| s.should_stop()));
        let flush = i32::from(c.flush.load(Ordering::SeqCst)) | stop;
        let mut error_0 = fc.task_thread.error.fetch_or(flush, Ordering::SeqCst) | flush;

        let mut sby = t.sby;
        let mut task_type = t.type_0;

        'fallthrough: loop {
          match task_type {
            TaskType::Init => {
              match ::core::primitive::bool::then_some(!(c.fc.len() > 1), ()) {
                ::core::option::Option::Some(()) => {
                  return Err(EINVAL);
                }
                ::core::option::Option::None => {}
              }

              let res = rav1d_decode_frame_init(c, fc);
              let p1_3 = {
                let in_cdf = fc.in_cdf()?;
                let Ok(progress) = in_cdf.progress();

                {
                  let Ok(numeric_value) =
                    console_core_number_conversion::wrapped::<_, i32>(match progress {
                      Some(progress_3) => progress_3.load(Ordering::SeqCst),
                      None => {
                        let Ok(numeric_value) =
                          console_core_number_conversion::wrapped::<_, u32>({
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, i32>(1);
                            numeric_value
                          });
                        numeric_value
                      }
                    });
                  numeric_value
                }
              };

              match ::core::primitive::bool::then_some(res.is_err() || p1_3 == TILE_ERROR, ()) {
                ::core::option::Option::Some(()) => {
                  assert!(task_thread_lock.is_none(), "thread lock should not be held");
                  task_thread_lock = Some(ttd.lock.lock());
                  let Ok(()) = abort_frame(
                    c,
                    fc,
                    match res {
                      ::core::result::Result::Err(_) => res,
                      ::core::result::Result::Ok(_) => Err(EINVAL),
                    },
                  );
                  let Ok(_) = reset_task_cur(c, ttd, Some(t.frame_idx));
                }
                ::core::option::Option::None => {
                  t.type_0 = TaskType::InitCdf;

                  match ::core::primitive::bool::then_some(p1_3 != 0, ()) {
                    ::core::option::Option::Some(()) => {
                      continue 'found_unlocked;
                    }
                    ::core::option::Option::None => {}
                  }

                  let Ok(()) = fc.task_thread.tasks.add_pending(t);
                  assert!(task_thread_lock.is_none(), "thread lock should not be held");
                  task_thread_lock = Some(ttd.lock.lock());
                }
              }

              continue 'outer;
            }
            TaskType::InitCdf => {
              match ::core::primitive::bool::then_some(!(c.fc.len() > 1), ()) {
                ::core::option::Option::Some(()) => {
                  return Err(EINVAL);
                }
                ::core::option::Option::None => {}
              }

              let mut res_0 = Err(EINVAL);
              let mut f = fc.data.try_write().ok_or(EINVAL)?;

              match ::core::primitive::bool::then_some(fc.task_thread.error.load(Ordering::SeqCst) == 0, ()) {
                ::core::option::Option::Some(()) => {
                  let in_cdf = fc.in_cdf()?;
                  res_0 = rav1d_decode_frame_init_cdf(c, fc, &mut f, &in_cdf);
                }
                ::core::option::Option::None => {}
              }

              let frame_hdr = &***f.frame_hdr.as_ref().ok_or(EINVAL)?;

              match ::core::primitive::bool::then_some(frame_hdr.refresh_context != 0
                && !({
                  let Ok(explicit_value) = fc.task_thread.update_set.get();
                  explicit_value
                }), ()) {
                ::core::option::Option::Some(()) => {
                  let Ok(f_progress) = f.out_cdf.progress();
                  let checked_value = f_progress.ok_or(EINVAL)?;
                  checked_value.store(
                    {
                      let Ok(numeric_value) =
                        console_core_number_conversion::wrapped::<_, u32>(match res_0 {
                          ::core::result::Result::Err(_) => TILE_ERROR,
                          ::core::result::Result::Ok(_) => {
                            let Ok(numeric_value) =
                              console_core_number_conversion::wrapped::<_, i32>(1);
                            numeric_value
                          }
                        });
                      numeric_value
                    },
                    Ordering::SeqCst,
                  );
                }
                ::core::option::Option::None => {}
              }

              drop(f);

              match res_0 {
                ::core::result::Result::Ok(_) => {
                  match ::core::primitive::bool::then_some(!(c.fc.len() > 1), ()) {
                    ::core::option::Option::Some(()) => {
                      return Err(EINVAL);
                    }
                    ::core::option::Option::None => {}
                  }

                  for stage in [TileQueueStage::Entropy, TileQueueStage::Reconstruction] {
                    let f_2 = fc.data.try_read().ok_or(EINVAL)?;
                    let res_1 = rav1d_task_create_tile_sbrow(fc, &f_2, stage);

                    match res_1 {
                      ::core::result::Result::Err(_) => {
                        assert!(task_thread_lock.is_none(), "thread lock should not be held");
                        task_thread_lock = Some(ttd.lock.lock());

                        let Ok(done) = stage.state_slot(&fc.task_thread.done);
                        done.store(1, Ordering::SeqCst);
                        fc.task_thread.error.store(-1, Ordering::SeqCst);
                        let frame_hdr_2 = &***f_2.frame_hdr.as_ref().ok_or(EINVAL)?;
                        let Ok(cols) = console_core_number_conversion::wrapped::<_, i32>(
                          frame_hdr_2.tiling.cols,
                        );
                        let Ok(rows) = console_core_number_conversion::wrapped::<_, i32>(
                          frame_hdr_2.tiling.rows,
                        );
                        fc.task_thread.task_counter.fetch_sub(
                          i32::wrapping_add(i32::wrapping_mul(cols, rows), f_2.sbh),
                          Ordering::SeqCst,
                        );

                        let progress = f_2.sr_cur.progress.as_deref().ok_or(EINVAL)?;
                        let Ok(stage_progress) = stage.picture_progress(progress);
                        stage_progress.store(FRAME_ERROR, Ordering::SeqCst);

                        match ::core::primitive::bool::then_some(stage == TileQueueStage::Reconstruction
                          && (*match &fc.task_thread.done {
                            [_, element_2, ..] => element_2,
                          })
                          .load(Ordering::SeqCst)
                            != 0, ())
                        {
                          ::core::option::Option::Some(()) => {
                            match ::core::primitive::bool::then_some(fc.task_thread.task_counter.load(Ordering::SeqCst) != 0, ()) {
                              ::core::option::Option::Some(()) => {
                                return Err(EINVAL);
                              }
                              ::core::option::Option::None => {}
                            }

                            drop(f_2);
                            let _ = rav1d_decode_frame_exit(c, fc, Err(ENOMEM));
                            fc.task_thread.cond.notify_one();
                          }
                          ::core::option::Option::None => {
                            drop(
                              task_thread_lock
                                .take()
                                .ok_or(EINVAL)?,
                            );
                          }
                        }
                      }
                      ::core::result::Result::Ok(_) => {}
                    }
                  }

                  assert!(task_thread_lock.is_none(), "thread lock should not be held");
                  task_thread_lock = Some(ttd.lock.lock());
                }
                ::core::result::Result::Err(_) => {
                  assert!(task_thread_lock.is_none(), "thread lock should not be held");
                  task_thread_lock = Some(ttd.lock.lock());
                  let Ok(()) = abort_frame(c, fc, res_0);
                  let Ok(_) = reset_task_cur(c, ttd, Some(t.frame_idx));
                  fc.task_thread.init_done.store(1, Ordering::SeqCst);
                }
              }

              continue 'outer;
            }
            TaskType::TileEntropy | TaskType::TileReconstruction => {
              let f = fc.data.try_read().ok_or(EINVAL)?;
              let p_1 = t.type_0 == TaskType::TileEntropy;
              let ts = match f.tile(t.tile_idx) {
                Some(tile) => tile,
                None => {
                  drop(f);
                  task_thread_lock = Some(ttd.lock.lock());
                  let Ok(()) = abort_frame(c, fc, Err(EINVAL));
                  let Ok(_) = reset_task_cur(c, ttd, Some(t.frame_idx));
                  continue 'outer;
                }
              };
              tc.tile_index = t.tile_idx;
              let Ok(sb_shift) = console_core_number_conversion::wrapped::<_, u32>(f.sb_shift);
              tc.b.y = i32::wrapping_shl(sby, sb_shift);
              let uses_2pass = i32::from(c.fc.len() > 1);
              tc.frame_thread.pass = match ::core::primitive::bool::then_some(uses_2pass == 0, ()) {
                ::core::option::Option::Some(()) => {
                  let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(0);
                  numeric_value
                }
                ::core::option::Option::None => i32::wrapping_add(
                  {
                    let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, i32>(1);
                    numeric_value
                  },
                  i32::from(t.type_0 == TaskType::TileReconstruction),
                ),
              };

              match ::core::primitive::bool::then_some(error_0 == 0, ()) {
                ::core::option::Option::Some(()) => {
                  error_0 = match rav1d_decode_tile_sbrow(c, &mut tc, &f) {
                    Ok(()) => 0,
                    Err(()) => 1,
                  };
                }
                ::core::option::Option::None => {}
              }

              let progress = match ::core::primitive::bool::then_some(error_0 != 0, ()) {
                ::core::option::Option::Some(()) => TILE_ERROR,
                ::core::option::Option::None => i32::wrapping_add(1, sby),
              };

              fc.task_thread.error.fetch_or(error_0, Ordering::SeqCst);

              match ::core::primitive::bool::then_some(i32::wrapping_shl(i32::wrapping_add(sby, 1), {
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, u32>(f.sb_shift);
                numeric_value
              }) < ts.tiling.row_end, ())
              {
                ::core::option::Option::Some(()) => {
                  {
                    let numeric_amount = 1;
                    let numeric_target = &mut (t.sby);
                    *numeric_target = i32::wrapping_add(*numeric_target, numeric_amount);
                  };

                  t.deps_skip = <crate::src::relaxed_atomic::RelaxedAtomic<::core::primitive::i32> as ::core::convert::From<::core::primitive::i32>>::from(0);

                  let Ok(pipeline) = tile_pipeline(c);
                  let Ok(readiness) = check_tile_or_record_error(&f, &fc.task_thread, &t, pipeline);

                  match readiness {
                    TileReadiness::Ready => {
                      ts.progress[match ::console_core_number_conversion::index::<
                        ::core::primitive::u8,
                      >(::core::primitive::u8::from(p_1))
                      {
                        ::core::result::Result::Ok(converted_position) => converted_position,
                      }]
                      .store(progress, Ordering::SeqCst);
                      let Ok(length) =
                        console_core_number_conversion::wrapped::<_, u32>(c.fc.len());
                      let Ok(()) = reset_task_cur_async(ttd, ResetFramePosition { slot: t.frame_idx, frame_count: length });

                      match ::core::primitive::bool::then_some(ttd.cond_signaled.fetch_or(1, Ordering::SeqCst) == 0, ()) {
                        ::core::option::Option::Some(()) => {
                          ttd.cond.notify_one();
                        }
                        ::core::option::Option::None => {}
                      }

                      continue 'found_unlocked;
                    }
                    TileReadiness::Waiting => {}
                  }

                  ts.progress[match ::console_core_number_conversion::index::<::core::primitive::u8>(::core::primitive::u8::from(p_1)) { ::core::result::Result::Ok(converted_position) => converted_position }].store(progress, Ordering::SeqCst);
                  let Ok(()) = fc.task_thread.tasks.add_pending(t);
                  assert!(task_thread_lock.is_none(), "thread lock should not be held");
                  drop(f);
                  task_thread_lock = Some(ttd.lock.lock());
                }
                ::core::option::Option::None => {
                  assert!(task_thread_lock.is_none(), "thread lock should not be held");
                  task_thread_lock = Some(ttd.lock.lock());
                  ts.progress[match ::console_core_number_conversion::index::<::core::primitive::u8>(::core::primitive::u8::from(p_1)) { ::core::result::Result::Ok(converted_position) => converted_position }].store(progress, Ordering::SeqCst);
                  let Ok(_) = reset_task_cur(c, ttd, Some(t.frame_idx));
                  error_0 = fc.task_thread.error.load(Ordering::SeqCst);
                  let frame_hdr = &***f.frame_hdr.as_ref().ok_or(EINVAL)?;

                  match ::core::primitive::bool::then_some(frame_hdr.refresh_context != 0
                    && tc.frame_thread.pass <= 1
                    && ({
                      let Ok(explicit_value) = fc.task_thread.update_set.get();
                      explicit_value
                    })
                    && u32::from(frame_hdr.tiling.update) == t.tile_idx, ())
                  {
                    ::core::option::Option::Some(()) => {
                      match ::core::primitive::bool::then_some(error_0 == 0, ()) {
                        ::core::option::Option::Some(()) => {
                          match rav1d_publish_tile_cdf(&f, frame_hdr) {
                            Ok(()) => {}
                            Err(_tile_cdf_unavailable) => {
                              error_0 = fc.task_thread.error.fetch_or(1, Ordering::SeqCst) | 1;
                            }
                          }
                        }
                        ::core::option::Option::None => {}
                      }

                      let Ok(progress_4) = f.out_cdf.progress();

                      match progress_4 {
                        Some(progress_5) => {
                          progress_5.store(
                            {
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped::<_, u32>(
                                  match ::core::primitive::bool::then_some(error_0 != 0, ()) {
                                    ::core::option::Option::Some(()) => TILE_ERROR,
                                    ::core::option::Option::None => {
                                      let Ok(numeric_value) =
                                        console_core_number_conversion::wrapped::<_, i32>(1);
                                      numeric_value
                                    }
                                  },
                                );
                              numeric_value
                            },
                            Ordering::SeqCst,
                          );
                        }
                        None => {}
                      }
                    }
                    ::core::option::Option::None => {}
                  }

                  match ::core::primitive::bool::then_some(i32::wrapping_sub(
                    fc.task_thread.task_counter.fetch_sub(1, Ordering::SeqCst),
                    1,
                  ) == 0
                    && (*match &fc.task_thread.done {
                      [element_2, ..] => element_2,
                    })
                    .load(Ordering::SeqCst)
                      != 0
                    && (uses_2pass == 0
                      || (*match &fc.task_thread.done {
                        [_, element_2, ..] => element_2,
                      })
                      .load(Ordering::SeqCst)
                        != 0), ())
                  {
                    ::core::option::Option::Some(()) => {
                      error_0 = fc.task_thread.error.load(Ordering::SeqCst);
                      drop(f);
                      let _ = rav1d_decode_frame_exit(
                        c,
                        fc,
                        match ::core::primitive::bool::then_some(error_0 == 1, ()) {
                          ::core::option::Option::Some(()) => Err(EINVAL),
                          ::core::option::Option::None => match ::core::primitive::bool::then_some(error_0 != 0, ()) {
                            ::core::option::Option::Some(()) => Err(ENOMEM),
                            ::core::option::Option::None => Ok(()),
                          },
                        },
                      );
                      fc.task_thread.cond.notify_one();
                    }
                    ::core::option::Option::None => {}
                  }

                  match ::core::primitive::bool::then_some(!(fc.task_thread.task_counter.load(Ordering::SeqCst) >= 0), ()) {
                    ::core::option::Option::Some(()) => {
                      return Err(EINVAL);
                    }
                    ::core::option::Option::None => {}
                  }

                  match ::core::primitive::bool::then_some(ttd.cond_signaled.fetch_or(1, Ordering::SeqCst) == 0, ()) {
                    ::core::option::Option::Some(()) => {
                      ttd.cond.notify_one();
                    }
                    ::core::option::Option::None => {}
                  }
                }
              }

              continue 'outer;
            }
            TaskType::DeblockCols => {
              {
                let f = fc.data.try_read().ok_or(EINVAL)?;

                match ::core::primitive::bool::then_some(fc.task_thread.error.load(Ordering::SeqCst) == 0, ()) {
                  ::core::option::Option::Some(()) => {
                    let Ok(bd_fn) = f.bd_fn();

                    match (bd_fn.filter_sbrow_deblock_cols)(c, &f, &mut tc, sby) {
                      Ok(()) => {}
                      Err(_invalid_filter_row) => {
                        let _previous_error = fc.task_thread.error.fetch_or(1, Ordering::SeqCst);
                      }
                    }
                  }
                  ::core::option::Option::None => {}
                }
              }

              match ::core::primitive::bool::then_some(({
                let Ok(explicit_value) = ensure_progress(
                  ttd,
                  fc,
                  &t,
                  TaskType::DeblockRows,
                  &fc.frame_thread_progress.deblock,
                  &mut task_thread_lock,
                );
                explicit_value
              }) != 0, ())
              {
                ::core::option::Option::Some(()) => {
                  continue 'outer;
                }
                ::core::option::Option::None => {}
              }

              task_type = TaskType::DeblockRows;
              continue 'fallthrough;
            }
            TaskType::DeblockRows => {
              let f = fc.data.try_read().ok_or(EINVAL)?;

              match ::core::primitive::bool::then_some(fc.task_thread.error.load(Ordering::SeqCst) == 0, ()) {
                ::core::option::Option::Some(()) => {
                  let Ok(bd_fn) = f.bd_fn();

                  match (bd_fn.filter_sbrow_deblock_rows)(c, &f, &mut tc, sby) {
                    Ok(()) => {}
                    Err(_invalid_resize_window) => {
                      let _previous_error = fc.task_thread.error.fetch_or(1, Ordering::SeqCst);
                    }
                  }
                }
                ::core::option::Option::None => {}
              }

              let seq_hdr = &***f.seq_hdr.as_ref().ok_or(EINVAL)?;
              let frame_hdr = &***f.frame_hdr.as_ref().ok_or(EINVAL)?;

              match ::core::primitive::bool::then_some(frame_hdr.loopfilter.level_y != [0; 2], ()) {
                ::core::option::Option::Some(()) => {
                  drop(f);
                  error_0 = fc.task_thread.error.load(Ordering::SeqCst);
                  fc.frame_thread_progress.deblock.store(
                    match ::core::primitive::bool::then_some(error_0 != 0, ()) {
                      ::core::option::Option::Some(()) => TILE_ERROR,
                      ::core::option::Option::None => i32::wrapping_add(sby, 1),
                    },
                    Ordering::SeqCst,
                  );
                  let Ok(length) = console_core_number_conversion::wrapped::<_, u32>(c.fc.len());
                  let Ok(()) = reset_task_cur_async(ttd, ResetFramePosition { slot: t.frame_idx, frame_count: length });

                  match ::core::primitive::bool::then_some(ttd.cond_signaled.fetch_or(1, Ordering::SeqCst) == 0, ()) {
                    ::core::option::Option::Some(()) => {
                      ttd.cond.notify_one();
                    }
                    ::core::option::Option::None => {}
                  }
                }
                ::core::option::Option::None => match ::core::primitive::bool::then_some(seq_hdr.cdef != 0 || !f.lf.restore_planes.is_empty(), ()) {
                  ::core::option::Option::Some(()) => {
                    drop(f);
                    let copy_lpf = fc.frame_thread_progress.copy_lpf.try_read().ok_or(EINVAL)?;
                    let row = ProgressBit::at(&copy_lpf, sby)?;
                    let Ok(()) = row.complete();

                    match ::core::primitive::bool::then_some(sby != 0, ()) {
                      ::core::option::Option::Some(()) => {
                        let previous = ProgressBit::at(&copy_lpf, sby.wrapping_sub(1))?;
                        let Ok(completion) = previous.load();

                        match completion {
                          RowCompletion::Pending => {
                            t.type_0 = TaskType::Cdef;
                            t.deblock_progress = {
                              let Ok(numeric_value) =
                                console_core_number_conversion::wrapped::<_, i32>(0);
                              numeric_value
                            };
                            t.recon_progress = t.deblock_progress;
                            let Ok(()) = fc.task_thread.tasks.add_pending(t);
                            assert!(task_thread_lock.is_none(), "thread lock should not be held");
                            task_thread_lock = Some(ttd.lock.lock());
                            continue 'outer;
                          }
                          RowCompletion::Complete => {}
                        }
                      }
                      ::core::option::Option::None => {}
                    }
                  }
                  ::core::option::Option::None => {}
                },
              }

              task_type = TaskType::Cdef;
              continue 'fallthrough;
            }
            TaskType::Cdef => {
              let f = fc.data.try_read().ok_or(EINVAL)?;
              let seq_hdr = match f.seq_hdr.as_ref() {
                Some(header) => &***header,
                None => {
                  let _previous_error = fc.task_thread.error.fetch_or(1, Ordering::SeqCst);
                  task_type = TaskType::ReconstructionProgress;
                  continue 'fallthrough;
                }
              };
              let mut next_stage = TaskType::SuperResolution;

              match ::core::primitive::bool::then_some(seq_hdr.cdef != 0, ()) {
                ::core::option::Option::Some(()) => {
                  match ::core::primitive::bool::then_some(fc.task_thread.error.load(Ordering::SeqCst) == 0, ()) {
                    ::core::option::Option::Some(()) => {
                      let Ok(bd_fn) = f.bd_fn();

                      match (bd_fn.filter_sbrow_cdef)(c, &f, &mut tc, sby) {
                        Ok(()) => {}
                        Err(_invalid_cdef_window) => {
                          let _previous_error = fc.task_thread.error.fetch_or(1, Ordering::SeqCst);
                          next_stage = TaskType::ReconstructionProgress;
                        }
                      }
                    }
                    ::core::option::Option::None => {}
                  }

                  drop(f);
                  let Ok(length) = console_core_number_conversion::wrapped::<_, u32>(c.fc.len());
                  let Ok(()) = reset_task_cur_async(ttd, ResetFramePosition { slot: t.frame_idx, frame_count: length });

                  match ::core::primitive::bool::then_some(ttd.cond_signaled.fetch_or(1, Ordering::SeqCst) == 0, ()) {
                    ::core::option::Option::Some(()) => {
                      ttd.cond.notify_one();
                    }
                    ::core::option::Option::None => {}
                  }
                }
                ::core::option::Option::None => {}
              }

              task_type = next_stage;
              continue 'fallthrough;
            }
            TaskType::SuperResolution => {
              let f = fc.data.try_read().ok_or(EINVAL)?;
              let frame_hdr = &***f.frame_hdr.as_ref().ok_or(EINVAL)?;

              match ::core::primitive::bool::then_some((match frame_hdr.size.width {
                [element, ..] => element,
              }) != (match frame_hdr.size.width {
                [_, element, ..] => element,
              }), ()) {
                ::core::option::Option::Some(()) => match ::core::primitive::bool::then_some(fc.task_thread.error.load(Ordering::SeqCst) == 0, ()) {
                  ::core::option::Option::Some(()) => {
                    let Ok(bd_fn) = f.bd_fn();

                    match (bd_fn.filter_sbrow_resize)(c, &f, &mut tc, sby) {
                      Ok(()) => {}
                      Err(_invalid_resize_window) => {
                        let _previous_error = fc.task_thread.error.fetch_or(1, Ordering::SeqCst);
                      }
                    }
                  }
                  ::core::option::Option::None => {}
                },
                ::core::option::Option::None => {}
              }

              task_type = TaskType::LoopRestoration;
              continue 'fallthrough;
            }
            TaskType::LoopRestoration => {
              let f = fc.data.try_read().ok_or(EINVAL)?;

              match ::core::primitive::bool::then_some(fc.task_thread.error.load(Ordering::SeqCst) == 0
                && !f.lf.restore_planes.is_empty(), ())
              {
                ::core::option::Option::Some(()) => {
                  let Ok(bd_fn) = f.bd_fn();

                  match (bd_fn.filter_sbrow_lr)(c, &f, &mut tc, sby) {
                    Ok(()) => {}
                    Err(_invalid_restoration_window) => {
                      let _previous_error = fc.task_thread.error.fetch_or(1, Ordering::SeqCst);
                    }
                  }
                }
                ::core::option::Option::None => {}
              }

              task_type = TaskType::ReconstructionProgress;
              continue 'fallthrough;
            }
            TaskType::ReconstructionProgress => {}
            TaskType::EntropyProgress => {}
          }

          break 'fallthrough;
        }

        let f = fc.data.try_read().ok_or(EINVAL)?;
        let uses_2pass_0 = i32::from(c.fc.len() > 1);
        let sbh = f.sbh;
        let sbsz = i32::wrapping_mul(f.sb_step, 4);

        match ::core::primitive::bool::then_some(t.type_0 == TaskType::EntropyProgress, ()) {
          ::core::option::Option::Some(()) => {
            error_0 = fc.task_thread.error.load(Ordering::SeqCst);
            let y: c_uint = match ::core::primitive::bool::then_some(i32::wrapping_add(sby, 1) == sbh, ()) {
              ::core::option::Option::Some(()) => u32::MAX,
              ::core::option::Option::None => ({
                let Ok(numeric_value) =
                  console_core_number_conversion::wrapped::<_, u32>(i32::wrapping_add(sby, 1));
                numeric_value
              })
              .wrapping_mul({
                let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(sbsz);
                numeric_value
              }),
            };

            let progress = &**f.sr_cur.progress.as_ref().ok_or(EINVAL)?;

            match ::core::primitive::bool::then_some(f.sr_cur.p.data.is_some(), ()) {
              ::core::option::Option::Some(()) => {
                (*match &*progress {
                  [element_2, ..] => element_2,
                })
                .store(
                  match ::core::primitive::bool::then_some(error_0 != 0, ()) {
                    ::core::option::Option::Some(()) => FRAME_ERROR,
                    ::core::option::Option::None => y,
                  },
                  Ordering::SeqCst,
                );
              }
              ::core::option::Option::None => {}
            }

            drop(f);
            fc.frame_thread_progress.entropy.store(
              match ::core::primitive::bool::then_some(error_0 != 0, ()) {
                ::core::option::Option::Some(()) => TILE_ERROR,
                ::core::option::Option::None => i32::wrapping_add(sby, 1),
              },
              Ordering::SeqCst,
            );

            match ::core::primitive::bool::then_some(i32::wrapping_add(sby, 1) == sbh, ()) {
              ::core::option::Option::Some(()) => {
                (*match &fc.task_thread.done {
                  [_, element_2, ..] => element_2,
                })
                .store(1, Ordering::SeqCst);
              }
              ::core::option::Option::None => {}
            }

            assert!(task_thread_lock.is_none(), "thread lock should not be held");
            task_thread_lock = Some(ttd.lock.lock());
            let num_tasks = i32::wrapping_sub(
              fc.task_thread.task_counter.fetch_sub(1, Ordering::SeqCst),
              1,
            );

            match ::core::primitive::bool::then_some(i32::wrapping_add(sby, 1) < sbh && num_tasks != 0, ()) {
              ::core::option::Option::Some(()) => {
                let Ok(_) = reset_task_cur(c, ttd, Some(t.frame_idx));
                continue 'outer;
              }
              ::core::option::Option::None => {}
            }

            match ::core::primitive::bool::then_some(num_tasks == 0
              && (*match &fc.task_thread.done {
                [element_2, ..] => element_2,
              })
              .load(Ordering::SeqCst)
                != 0
              && (*match &fc.task_thread.done {
                [_, element_2, ..] => element_2,
              })
              .load(Ordering::SeqCst)
                != 0, ())
            {
              ::core::option::Option::Some(()) => {
                error_0 = fc.task_thread.error.load(Ordering::SeqCst);
                let _ = rav1d_decode_frame_exit(
                  c,
                  fc,
                  match ::core::primitive::bool::then_some(error_0 == 1, ()) {
                    ::core::option::Option::Some(()) => Err(EINVAL),
                    ::core::option::Option::None => match ::core::primitive::bool::then_some(error_0 != 0, ()) {
                      ::core::option::Option::Some(()) => Err(ENOMEM),
                      ::core::option::Option::None => Ok(()),
                    },
                  },
                );
                fc.task_thread.cond.notify_one();
              }
              ::core::option::Option::None => {}
            }

            let Ok(_) = reset_task_cur(c, ttd, Some(t.frame_idx));
            continue 'outer;
          }
          ::core::option::Option::None => {}
        }

        {
          let map = fc.frame_thread_progress.frame.try_read().ok_or(EINVAL)?;
          let completed_row = ProgressBit::at(&map, sby)?;
          let Ok(()) = completed_row.complete();
        }

        {
          let _task_thread_lock = fc.task_thread.lock.lock();
          let frame_progress = get_frame_progress(fc, &f)?;
          sby = frame_progress;
          error_0 = fc.task_thread.error.load(Ordering::SeqCst);
          let y_0: c_uint = match ::core::primitive::bool::then_some(i32::wrapping_add(sby, 1) == sbh, ()) {
            ::core::option::Option::Some(()) => u32::MAX,
            ::core::option::Option::None => ({
              let Ok(numeric_value) =
                console_core_number_conversion::wrapped::<_, u32>(i32::wrapping_add(sby, 1));
              numeric_value
            })
            .wrapping_mul({
              let Ok(numeric_value) = console_core_number_conversion::wrapped::<_, u32>(sbsz);
              numeric_value
            }),
          };

          match &f.sr_cur.progress {
            Some(progress) => match ::core::primitive::bool::then_some(f.sr_cur.p.data.is_some(), ()) {
              ::core::option::Option::Some(()) => {
                (*match &**progress {
                  [_, element_3, ..] => element_3,
                })
                .store(
                  match ::core::primitive::bool::then_some(error_0 != 0, ()) {
                    ::core::option::Option::Some(()) => FRAME_ERROR,
                    ::core::option::Option::None => y_0,
                  },
                  Ordering::SeqCst,
                );
              }
              ::core::option::Option::None => {}
            },
            None => {}
          }
        }

        drop(f);

        match ::core::primitive::bool::then_some(i32::wrapping_add(sby, 1) == sbh, ()) {
          ::core::option::Option::Some(()) => {
            (*match &fc.task_thread.done {
              [element_2, ..] => element_2,
            })
            .store(1, Ordering::SeqCst);
          }
          ::core::option::Option::None => {}
        }

        assert!(task_thread_lock.is_none(), "thread lock should not be held");
        task_thread_lock = Some(ttd.lock.lock());
        let num_tasks_0 = i32::wrapping_sub(
          fc.task_thread.task_counter.fetch_sub(1, Ordering::SeqCst),
          1,
        );

        match ::core::primitive::bool::then_some(i32::wrapping_add(sby, 1) < sbh && num_tasks_0 != 0, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(_) = reset_task_cur(c, ttd, Some(t.frame_idx));
            continue 'outer;
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some(num_tasks_0 == 0
          && (*match &fc.task_thread.done {
            [element_2, ..] => element_2,
          })
          .load(Ordering::SeqCst)
            != 0
          && (uses_2pass_0 == 0
            || (*match &fc.task_thread.done {
              [_, element_2, ..] => element_2,
            })
            .load(Ordering::SeqCst)
              != 0), ())
        {
          ::core::option::Option::Some(()) => {
            error_0 = fc.task_thread.error.load(Ordering::SeqCst);
            let _ = rav1d_decode_frame_exit(
              c,
              fc,
              match ::core::primitive::bool::then_some(error_0 == 1, ()) {
                ::core::option::Option::Some(()) => Err(EINVAL),
                ::core::option::Option::None => match ::core::primitive::bool::then_some(error_0 != 0, ()) {
                  ::core::option::Option::Some(()) => Err(ENOMEM),
                  ::core::option::Option::None => Ok(()),
                },
              },
            );
            fc.task_thread.cond.notify_one();
          }
          ::core::option::Option::None => {}
        }

        let Ok(_) = reset_task_cur(c, ttd, Some(t.frame_idx));

        break 'found_unlocked;
      }
    }

    let held = task_thread_lock.take().ok_or(EINVAL)?;
      drop(held);
  };

  Ok(())
}

#[cfg(test)]
mod grain_panic_tests {
  use super::*;
  use std::sync::mpsc;
  use std::time::Duration;

  #[test]
  fn reservation_caches_finished_fault_and_preserves_unfinished_slot_on_worker_failure() -> Rav1dResult {
    let (Ok(completed), Ok(incomplete)) = (Rav1dFrameContext::default(0), Rav1dFrameContext::default(1));
    completed.task_thread.finished.store(true, Ordering::SeqCst);
    completed.task_thread.error.store(1, Ordering::SeqCst);

    {
      let mut output_fault = completed.task_thread.retval.try_lock().ok_or(EAGAIN)?;
      *output_fault = Some(EINVAL);
    }

    let decoder = Rav1dContext { fc: Box::new([completed, incomplete]), ..Rav1dContext::default() };
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

  #[test]
  fn failed_grain_preparation_returns_without_scheduling_rows() -> Rav1dResult {
    let decoder = Rav1dContext::default();
    let shared = Arc::clone(&decoder.task_thread);
    let worker = thread::spawn(move || -> Rav1dResult {
      let lock = shared.lock.lock();
      let (lock_2, timeout) = unpoison(shared.cond.wait_timeout_while(lock, Duration::from_secs(5), |_data| {
        let Ok(active) = shared.delayed_fg_exec.get();
        active == 0
      }));

      match timeout.timed_out().then_some(()) {
        Some(()) => {
          shared.panicked.store(true, Ordering::SeqCst);
          shared.delayed_fg_cond.notify_one();
          return Err(EGeneric);
        }
        None => {}
      }

      let mut scheduler = Some(lock_2);
      delayed_fg_task(&shared, &mut scheduler)?;
      drop(scheduler);
      Ok(())
    });
    let mut out = Rav1dPicture::default();
    out.p.bpc = match cfg!(feature = "bitdepth_8") {
      true => 8,
      false => 10,
    };
    let rejected = rav1d_task_delayed_fg(&decoder, &mut out, &Rav1dPicture::default());
    let joined = worker.join().map_err(|_worker_panicked| EGeneric)?;
    joined?;
    assert_eq!(rejected, Err(EINVAL));
    let [submitted, completed] = &decoder.task_thread.delayed_fg_progress;
    assert_eq!(submitted.load(Ordering::SeqCst), 0);
    assert_eq!(completed.load(Ordering::SeqCst), 0);
    assert!(!decoder.task_thread.panicked.load(Ordering::SeqCst));
    Ok(())
  }

  #[test]
  fn delayed_grain_panic_preserves_live_worker_state() -> Rav1dResult {
    let c = Rav1dContext::default();
    let ttd = Arc::clone(&c.task_thread);
    let (release, released) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
      let mut lock = ttd.lock.lock();

      while ({
        let Ok(explicit_value) = ttd.delayed_fg_exec.get();
        explicit_value
      }) == 0
      {
        let (next, timeout) = unpoison(ttd.cond.wait_timeout(lock, Duration::from_secs(5)));
        lock = next;
        assert!(!timeout.timed_out());
      }

      let grain = ttd.delayed_fg.read();
      (*match &ttd.delayed_fg_progress {
        [element, ..] => element,
      })
      .store(3, Ordering::SeqCst);
      (*match &ttd.delayed_fg_progress {
        [_, element, ..] => element,
      })
      .store(1, Ordering::SeqCst);
      ttd.panicked.store(true, Ordering::SeqCst);
      ttd.delayed_fg_cond.notify_one();
      drop(lock);
      released
        .recv_timeout(Duration::from_secs(5))
        .expect("caller must return while the grain reader is alive");
      assert_eq!(grain.out.p.bpc, 8);
    });
    let mut out = Rav1dPicture::default();
    out.p.bpc = 8;
    rav1d_task_delayed_fg(&c, &mut out, &Rav1dPicture::default())?;
    assert!(c.task_thread.panicked.load(Ordering::SeqCst));
    let Ok(delayed_fg_exec) = c.task_thread.delayed_fg_exec.get();
    assert_eq!(delayed_fg_exec, 0);
    assert_eq!(
      (*match &c.task_thread.delayed_fg_progress {
        [element, ..] => element,
      })
      .load(Ordering::SeqCst),
      3
    );
    assert_eq!(
      (*match &c.task_thread.delayed_fg_progress {
        [_, element, ..] => element,
      })
      .load(Ordering::SeqCst),
      1
    );
    release.send(()).unwrap();
    worker.join().unwrap();
    Ok(())
  }
}
