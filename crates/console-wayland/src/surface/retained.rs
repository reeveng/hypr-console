//! Device pixels for the latest scene and the damage a released buffer missed.

use std::collections::VecDeque;
use std::io;
use std::num::NonZeroU32;

use console_core_geometry::{Point, Size};
use console_core_number_conversion::{fitted, fixed_index, index};
use console_core_state_machine::{
  EffectDependencies, Never, Service, SingleEffectMachine, SingleTransition,
};
use console_response_times::frames::Buffer;

use crate::memory::Shared;

use super::{Part, on_the_device};

const HISTORY: u8 = 8;
const PIXEL: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
  pub left: u32,
  pub top: u32,
  pub right: u32,
  pub bottom: u32,
}

impl Area {
  pub fn between(size: Size<u32>, before: &[u8], after: &[u8]) -> Result<Option<Self>, Never> {
    let Ok(width) = index(size.width);
    let mut damage: Option<Self> = None;

    for (row, (old, new)) in before
      .as_chunks::<4>()
      .0
      .chunks(width.max(1))
      .zip(after.as_chunks::<4>().0.chunks(width.max(1)))
      .enumerate()
    {
      match (old == new).then_some(()) {
        Some(()) => continue,
        None => {}
      }

      match old.iter().zip(new).position(|(a, b)| a != b) {
        Some(first) => {
          let Ok(left) = fitted::<_, u32>(first);
          let last_changed = match old.iter().zip(new).rposition(|(a, b)| a != b) {
            Some(last) => {
              let Ok(last_pixel) = fitted::<_, u32>(last);
              last_pixel
            }
            None => left,
          };
          let right = last_changed.saturating_add(1);
          let Ok(top) = fitted::<_, u32>(row);
          let changed = Self {
            left,
            right,
            top,
            bottom: top.saturating_add(1),
          };
          damage = Some(match damage {
            Some(area) => {
              let Ok(combined) = area.union(changed);
              combined
            }
            None => changed,
          });
        }
        None => {}
      }
    }

    Ok(damage)
  }

  #[must_use]
  pub fn union(self, other: Self) -> Result<Self, Never> {
    Ok(Self {
      left: self.left.min(other.left),
      top: self.top.min(other.top),
      right: self.right.max(other.right),
      bottom: self.bottom.max(other.bottom),
    })
  }

  #[must_use]
  pub fn width(self) -> Result<u32, Never> {
    Ok(self.right.saturating_sub(self.left))
  }

  #[must_use]
  pub fn height(self) -> Result<u32, Never> {
    Ok(self.bottom.saturating_sub(self.top))
  }

  fn rows(self, size: Size<u32>) -> io::Result<Rows> {
    match (self.left < self.right
      && self.top < self.bottom
      && self.right <= size.width
      && self.bottom <= size.height)
      .then_some(())
    {
      Some(()) => {}
      None => return invalid_pixels(),
    }

    let pixel = u64::from(PIXEL);
    let Ok(width) = self.width();
    let Ok(height) = self.height();
    let stride = u64::from(size.width).saturating_mul(pixel);
    let left = u64::from(self.left).saturating_mul(pixel);
    let wide = u64::from(width).saturating_mul(pixel);

    Ok(Rows {
      stride,
      left,
      wide,
      top: self.top,
      height,
    })
  }
}

#[derive(Clone, Copy)]
pub struct Geometry {
  pub logical: Size<u32>,
  pub device: Size<u32>,
}

impl Geometry {
  pub fn area(self, part: Part) -> Result<Option<Area>, Never> {
    let Ok(damage) = on_the_device(part, self.logical, self.device);
    let Ok(left) = fitted::<_, u32>(damage.at.x.max(0));
    let Ok(top) = fitted::<_, u32>(damage.at.y.max(0));
    let Ok(right) = fitted::<_, u32>(damage.at.x.saturating_add(damage.size.width).max(0));
    let Ok(bottom) = fitted::<_, u32>(damage.at.y.saturating_add(damage.size.height).max(0));
    let area = Area {
      left,
      top,
      right: right.min(self.device.width),
      bottom: bottom.min(self.device.height),
    };

    Ok((area.left < area.right && area.top < area.bottom).then_some(area))
  }
}

#[derive(Clone, Copy)]
pub struct Tile {
  pub part: Part,
  pub area: Area,
}

impl Tile {
  #[doc = "Fit a device-pixel change inside a child surface with the parent's sampling phase."]
  #[doc = "The available region must also contain the unchanged texture-filter border."]
  pub fn aligned(
    geometry: Geometry,
    changed: Area,
    available: Area,
  ) -> Result<Option<Self>, Never> {
    let Ok(x) = AlignedAxis::of(
      AxisLengths {
        points: geometry.logical.width,
        device: geometry.device.width,
      },
      changed.left..changed.right,
    );
    let Ok(y) = AlignedAxis::of(
      AxisLengths {
        points: geometry.logical.height,
        device: geometry.device.height,
      },
      changed.top..changed.bottom,
    );
    let (x_axis, y_axis) = match x.zip(y) {
      Some(aligned) => aligned,
      None => return Ok(None),
    };
    let area = Area {
      left: x_axis.device.start,
      right: x_axis.device.end,
      top: y_axis.device.start,
      bottom: y_axis.device.end,
    };
    let fits = area.left >= available.left
      && area.right <= available.right
      && area.top >= available.top
      && area.bottom <= available.bottom;
    let Ok(x_origin) = fitted::<_, i32>(x_axis.points.start);
    let Ok(y_origin) = fitted::<_, i32>(y_axis.points.start);
    let part = Part {
      at: Point {
        x: x_origin,
        y: y_origin,
      },
      size: Size {
        width: x_axis.points.end.saturating_sub(x_axis.points.start),
        height: y_axis.points.end.saturating_sub(y_axis.points.start),
      },
    };

    Ok(fits.then_some(Self { part, area }))
  }
}

struct AlignedAxis {
  points: std::ops::Range<u32>,
  device: std::ops::Range<u32>,
}

#[derive(Clone, Copy)]
struct AxisLengths {
  points: u32,
  device: u32,
}

impl AlignedAxis {
  fn of(lengths: AxisLengths, changed: std::ops::Range<u32>) -> Result<Option<Self>, Never> {
    let Ok(common) = common_divisor(lengths);
    let divisor = match common {
      Some(divisor) => divisor,
      None => return Ok(None),
    };
    let device_step = lengths
      .device
      .checked_div(divisor.get())
      .and_then(NonZeroU32::new);
    let logical_step = lengths.points.checked_div(divisor.get());
    let (step, logical) = match (device_step, logical_step) {
      (Some(step), Some(logical)) => (step, logical),
      (None, _) | (_, None) => return Ok(None),
    };
    const TEXTURE_FILTER_BORDER: u32 = 2;
    let near = match changed
      .start
      .saturating_sub(TEXTURE_FILTER_BORDER)
      .checked_div(step.get())
    {
      Some(near) => near,
      None => return Ok(None),
    };
    let far = changed
      .end
      .saturating_add(TEXTURE_FILTER_BORDER)
      .min(lengths.device)
      .div_ceil(step.get());

    Ok(Some(Self {
      points: near.saturating_mul(logical)..far.saturating_mul(logical),
      device: near.saturating_mul(step.get())..far.saturating_mul(step.get()),
    }))
  }
}

fn common_divisor(lengths: AxisLengths) -> Result<Option<NonZeroU32>, Never> {
  let AxisLengths {
    points: mut a,
    device: mut b,
  } = lengths;

  match (a, b) {
    (0, _) | (_, 0) => return Ok(None),
    (1.., 1..) => {}
  }

  for _ in 0..64 {
    match NonZeroU32::new(b) {
      None => return Ok(NonZeroU32::new(a)),
      Some(divisor) => (a, b) = (b, a.wrapping_rem(divisor.get())),
    }
  }

  Ok(None)
}

struct Rows {
  stride: u64,
  left: u64,
  wide: u64,
  top: u32,
  height: u32,
}

impl Rows {
  fn span<'a>(&self, line: &'a [u8]) -> io::Result<&'a [u8]> {
    let Ok(left) = index(self.left);
    let Ok(right) = index(self.left.saturating_add(self.wide));

    match line.get(left..right) {
      Some(span) => Ok(span),
      None => invalid_pixels(),
    }
  }

  fn span_mut<'a>(&self, line: &'a mut [u8]) -> io::Result<&'a mut [u8]> {
    let Ok(left) = index(self.left);
    let Ok(right) = index(self.left.saturating_add(self.wide));

    match line.get_mut(left..right) {
      Some(span) => Ok(span),
      None => invalid_pixels(),
    }
  }
}

fn invalid_pixels<T>() -> io::Result<T> {
  Err(io::Error::new(
    io::ErrorKind::InvalidInput,
    "pixels do not fit the retained frame",
  ))
}

pub struct Patch {
  area: Area,
  pixels: Vec<u8>,
}

impl Patch {
  pub fn extract(size: Size<u32>, pixels: &[u8], area: Area) -> io::Result<Self> {
    require_size(size, pixels)?;
    let rows = area.rows(size)?;
    let Ok(stride) = index(rows.stride);
    let Ok(top) = index(rows.top);
    let Ok(height) = index(rows.height);
    let Ok(bytes) = index(rows.wide.saturating_mul(u64::from(rows.height)));
    let mut patch = Vec::with_capacity(bytes.min(pixels.len()));

    for line in pixels.chunks_exact(stride.max(1)).skip(top).take(height) {
      let span = rows.span(line)?;
      patch.extend_from_slice(span);
    }

    Ok(Self {
      area,
      pixels: patch,
    })
  }

  pub fn tile_size(&self) -> Result<Size<u32>, Never> {
    let Ok(width) = self.area.width();
    let Ok(height) = self.area.height();

    Ok(Size { width, height })
  }

  pub fn write_tile(&self, buffer: &Shared) -> io::Result<()> {
    buffer.write_pixels(&self.pixels)
  }

  fn apply(&self, size: Size<u32>, pixels: &mut [u8]) -> io::Result<()> {
    let rows = self.area.rows(size)?;
    let Ok(stride) = index(rows.stride);
    let Ok(top) = index(rows.top);
    let Ok(height) = index(rows.height);
    let Ok(wide) = index(rows.wide);
    let lines = pixels
      .chunks_exact_mut(stride.max(1))
      .skip(top)
      .take(height);

    for (target, source) in lines.zip(self.pixels.chunks_exact(wide.max(1))) {
      let span = rows.span_mut(target)?;

      #[cfg_attr(
        dylint_lib = "explicit056_no_hidden_panic_api",
        expect(
          explicit056_no_hidden_panic_api,
          reason = "Both rows use the same patch width. The source is an exact-width chunk and the destination is a checked range of that width."
        )
      )]
      span.copy_from_slice(source);
    }

    Ok(())
  }
}

fn require_size(size: Size<u32>, pixels: &[u8]) -> io::Result<()> {
  let bytes = u64::from(size.width)
    .saturating_mul(u64::from(size.height))
    .saturating_mul(u64::from(PIXEL));
  let Ok(length) = index(bytes);

  match (pixels.len() == length && length != 0).then_some(()) {
    Some(()) => Ok(()),
    None => invalid_pixels(),
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Change {
  Clean,
  Partial(Area),
  Full,
}

impl Change {
  #[must_use]
  fn union(self, other: Self) -> Result<Self, Never> {
    Ok(match (self, other) {
      (Self::Full, _) | (_, Self::Full) => Self::Full,
      (Self::Clean, remaining) | (remaining, Self::Clean) => remaining,
      (Self::Partial(a), Self::Partial(b)) => {
        let Ok(combined) = a.union(b);
        Self::Partial(combined)
      }
    })
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Revision(u64);

#[derive(Default)]
struct History {
  current: Revision,
  recent: VecDeque<Change>,
}

enum Request {
  Invalidate(Change),
  Commit {
    buffer: BufferRevision,
    shown: BufferRevision,
  },
}

#[derive(Clone, Copy)]
pub(super) struct Commit {
  repair: Change,
  damage: Change,
  revision: BufferRevision,
}

impl EffectDependencies for Commit {
  fn services(&self) -> Result<&'static [Service], Never> {
    Ok(&[])
  }
}

impl SingleEffectMachine for History {
  type Input = ();
  type State = Self;
  type Request = Request;
  type Effect = Commit;

  fn initialize(
    _input: &(),
    previous: Option<Self>,
  ) -> Result<SingleTransition<Self, Commit>, Never> {
    let state = match previous {
      Some(history) => history,
      None => Self::default(),
    };

    Ok(SingleTransition {
      state,
      effect: None,
    })
  }

  fn handle(mut state: Self, request: Request) -> Result<SingleTransition<Self, Commit>, Never> {
    let effect = match request {
      Request::Invalidate(Change::Clean) => None,
      Request::Invalidate(change @ (Change::Full | Change::Partial(_))) => {
        state.current = Revision(state.current.0.wrapping_add(1));
        let Ok(history_limit) = index(HISTORY);

        match (state.recent.len() == history_limit).then_some(()) {
          Some(()) => {
            let _oldest = state.recent.pop_front();
          }
          None => {}
        }

        state.recent.push_back(change);
        None
      }
      Request::Commit { buffer, shown } => {
        let revision = BufferRevision(Some(state.current));
        let Ok(repair) = state.since(buffer);
        let Ok(damage) = state.since(shown);
        Some(Commit {
          repair,
          damage,
          revision,
        })
      }
    };

    Ok(SingleTransition { state, effect })
  }
}

impl History {
  #[must_use]
  fn since(&self, revision: BufferRevision) -> Result<Change, Never> {
    let since = match revision.0 {
      Some(previous) => self.current.0.wrapping_sub(previous.0),
      None => return Ok(Change::Full),
    };
    let Ok(steps) = index(since);

    Ok(match (steps <= self.recent.len()).then_some(()) {
      Some(()) => {
        self
          .recent
          .iter()
          .rev()
          .take(steps)
          .copied()
          .fold(Change::Clean, |total, change| {
            let Ok(combined) = total.union(change);
            combined
          })
      }
      None => Change::Full,
    })
  }
}

#[doc = "The revision of a buffer owned by the same retained scene."]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BufferRevision(Option<Revision>);

#[derive(Clone, Copy)]
pub(super) enum Ownership {
  Released(BufferRevision),
  Submitted(BufferRevision),
}

impl Default for Ownership {
  fn default() -> Self {
    Self::Released(BufferRevision::default())
  }
}

pub(super) enum BufferEvent {
  Release,
  Inherit(BufferRevision),
  Submit(BufferRevision),
}

impl SingleEffectMachine for Ownership {
  type Input = ();
  type State = Self;
  type Request = BufferEvent;
  type Effect = Commit;

  fn initialize(
    _input: &(),
    previous: Option<Self>,
  ) -> Result<SingleTransition<Self, Commit>, Never> {
    let state = match previous {
      Some(ownership) => ownership,
      None => Self::default(),
    };

    Ok(SingleTransition {
      state,
      effect: None,
    })
  }

  fn handle(state: Self, request: BufferEvent) -> Result<SingleTransition<Self, Commit>, Never> {
    let next = match (state, request) {
      (Self::Submitted(revision) | Self::Released(revision), BufferEvent::Release) => {
        Self::Released(revision)
      }
      (Self::Released(_), BufferEvent::Inherit(revision)) => Self::Released(revision),
      (Self::Submitted(_), BufferEvent::Inherit(revision))
      | (Self::Released(_), BufferEvent::Submit(revision)) => Self::Submitted(revision),
      (Self::Submitted(_), BufferEvent::Submit(_)) => state,
    };

    Ok(SingleTransition {
      state: next,
      effect: None,
    })
  }
}

impl Ownership {
  #[must_use]
  pub(super) fn held(self) -> Result<Buffer, Never> {
    Ok(match self {
      Self::Submitted(_) => Buffer::Acquired,
      Self::Released(_) => Buffer::Released,
    })
  }

  #[must_use]
  pub(super) fn revision(self) -> Result<BufferRevision, Never> {
    Ok(match self {
      Self::Submitted(revision) | Self::Released(revision) => revision,
    })
  }

  pub(super) fn request(&mut self, event: BufferEvent) -> Result<(), Never> {
    let Ok(next) = Self::handle(*self, event);
    *self = next.state;

    Ok(())
  }
}

#[derive(Default)]
pub struct Retained {
  size: Size<u32>,
  pixels: Vec<u8>,
  history: History,
}

impl Retained {
  pub(super) fn inherit(
    &mut self,
    device: Size<u32>,
    pixels: Vec<u8>,
  ) -> io::Result<BufferRevision> {
    require_size(device, &pixels)?;
    self.size = device;
    self.pixels = pixels;
    let Ok(()) = self.invalidate(Change::Full);

    Ok(BufferRevision(Some(self.history.current)))
  }

  pub(super) fn paint(
    &mut self,
    area: Area,
    paint: impl FnOnce(&mut [u8]) -> Result<(), Never>,
  ) -> io::Result<()> {
    let _rows = area.rows(self.size)?;
    let Ok(()) = paint(&mut self.pixels);
    let Ok(()) = self.invalidate(Change::Partial(area));

    Ok(())
  }

  pub fn replace(&mut self, size: Size<u32>, pixels: Vec<u8>) -> io::Result<()> {
    require_size(size, &pixels)?;
    let change = match (size == self.size).then_some(()) {
      Some(()) => {
        let Ok(damage) = Area::between(size, &self.pixels, &pixels);
        damage.map_or(Change::Clean, Change::Partial)
      }
      None => Change::Full,
    };

    self.size = size;
    self.pixels = pixels;
    let Ok(()) = self.invalidate(change);

    Ok(())
  }

  pub fn patch(&mut self, patch: &Patch) -> io::Result<()> {
    patch.apply(self.size, &mut self.pixels)?;
    let Ok(()) = self.invalidate(Change::Partial(patch.area));

    Ok(())
  }

  pub fn copy_to(&mut self, buffer: &Shared, revision: &mut BufferRevision) -> io::Result<()> {
    let _damage = self.prepare(buffer, revision, BufferRevision::default())?;
    Ok(())
  }

  #[must_use]
  pub(super) fn damage_since(&self, revision: BufferRevision) -> Result<Change, Never> {
    self.history.since(revision)
  }

  pub(super) fn prepare(
    &mut self,
    buffer: &Shared,
    revision: &mut BufferRevision,
    shown: BufferRevision,
  ) -> io::Result<Change> {
    let request = Request::Commit {
      buffer: *revision,
      shown,
    };
    let Ok(next) = History::handle(std::mem::take(&mut self.history), request);
    self.history = next.state;
    let commit = match next.effect {
      Some(commit) => commit,
      None => return invalid_pixels(),
    };

    match commit.repair {
      Change::Clean => {}
      Change::Full => buffer.write_pixels(&self.pixels)?,
      Change::Partial(area) => {
        let rows = area.rows(self.size)?;
        let Ok(stride) = index(rows.stride);
        let Ok(top) = index(rows.top);
        let Ok(height) = index(rows.height);
        let sources = self
          .pixels
          .chunks_exact(stride.max(1))
          .enumerate()
          .skip(top)
          .take(height);

        for (row, source) in sources {
          let pixels = rows.span(source)?;
          let Ok(row_index) = fixed_index(row);
          let offset = row_index
            .saturating_mul(rows.stride)
            .saturating_add(rows.left);
          buffer.write_pixels_at(pixels, offset)?;
        }
      }
    }

    *revision = commit.revision;

    Ok(commit.damage)
  }

  #[must_use]
  pub(super) fn size(&self) -> Result<Size<u32>, Never> {
    Ok(self.size)
  }

  pub(super) fn invalidate(&mut self, change: Change) -> Result<(), Never> {
    let Ok(next) = History::handle(
      std::mem::take(&mut self.history),
      Request::Invalidate(change),
    );
    self.history = next.state;

    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn adopting_a_displayed_frame_keeps_the_first_update_damage_local() -> io::Result<()> {
    let size = Size {
      width: 4,
      height: 2,
    };
    let mut retained = Retained::default();
    let shown = retained.inherit(size, vec![7; 32])?;
    let destination = Shared::of(32)?;
    let mut reusable = BufferRevision::default();
    let cursor = Area {
      left: 1,
      top: 0,
      right: 2,
      bottom: 2,
    };

    retained.paint(cursor, |pixels| {
      for row in pixels.chunks_exact_mut(16) {
        match row.get_mut(4..8) {
          Some(pixel) => pixel.fill(255),
          None => {}
        }
      }

      Ok(())
    })?;
    let damage = retained.prepare(&destination, &mut reusable, shown)?;
    let mut actual = [0; 32];
    destination.read_pixels_at(&mut actual, 0)?;

    assert_eq!(damage, Change::Partial(cursor));
    assert_eq!(
      actual,
      [
        7, 7, 7, 7, 255, 255, 255, 255, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 255, 255, 255, 255,
        7, 7, 7, 7, 7, 7, 7, 7,
      ],
    );

    Ok(())
  }

  #[test]
  fn a_released_buffer_repairs_missed_pixels_without_expanding_commit_damage() -> io::Result<()> {
    let size = Size {
      width: 4,
      height: 2,
    };
    let destination = Shared::of(32)?;
    let mut retained = Retained::default();
    retained.replace(size, vec![0; 32])?;
    let mut older = BufferRevision::default();
    let mut shown = BufferRevision::default();
    retained.copy_to(&destination, &mut older)?;
    let initial_frame = Shared::of(32)?;
    retained.copy_to(&initial_frame, &mut shown)?;

    let first = Area {
      left: 0,
      top: 0,
      right: 1,
      bottom: 1,
    };
    let second = Area {
      left: 3,
      top: 1,
      right: 4,
      bottom: 2,
    };
    let first_patch = Patch::extract(size, &[1; 32], first)?;
    let second_patch = Patch::extract(size, &[2; 32], second)?;
    retained.patch(&first_patch)?;
    let another = Shared::of(32)?;
    retained.copy_to(&another, &mut shown)?;
    retained.patch(&second_patch)?;

    let damage = retained.prepare(&destination, &mut older, shown)?;
    let mut actual = [0; 32];
    destination.read_pixels_at(&mut actual, 0)?;
    let mut expected = [0; 32];
    first_patch.apply(size, &mut expected)?;
    second_patch.apply(size, &mut expected)?;

    assert_eq!(damage, Change::Partial(second));
    assert_eq!(actual, expected);

    Ok(())
  }
}
