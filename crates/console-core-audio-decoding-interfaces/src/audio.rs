//! Decoded audio: which channels a stream has and the buffers it is decoded into.
//!
//! A decoder writes into an [`AudioBuffer`], planar, one plane per channel and
//! each plane as long as the capacity the buffer was made with, so a frame is
//! written to every plane at the same index. What a caller sees of a plane is
//! only the frames rendered so far. A player that wants interleaved samples of
//! another type copies the buffer out through a [`SampleBuffer`].
//!
//! Counts and positions here are `u32`: a plane holds a number of frames no
//! decoder has come near the end of, and the one place they meet the width the
//! standard library measures a list in is `console-core-number-conversion`.
//! Nothing here panics for a count that is wrong. A channel that is not there
//! is an empty slice, as `get` on a list would say; a render past the capacity
//! is a limit that was reached.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt;

use console_core_never::Never;
use console_core_number_conversion::index;

use crate::conv::{ConvertibleSample, FromSample, IntoSample};
use crate::errors::{Result, limit_error};
use crate::sample::{Sample, i24, u24};
use crate::units::Duration;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub struct Channels(u32);

impl Channels {
  pub const FRONT_LEFT: Self = Self(0x0000_0001);
  pub const FRONT_RIGHT: Self = Self(0x0000_0002);
  pub const FRONT_CENTRE: Self = Self(0x0000_0004);
  pub const LFE1: Self = Self(0x0000_0008);
  pub const REAR_LEFT: Self = Self(0x0000_0010);
  pub const REAR_RIGHT: Self = Self(0x0000_0020);
  pub const FRONT_LEFT_CENTRE: Self = Self(0x0000_0040);
  pub const FRONT_RIGHT_CENTRE: Self = Self(0x0000_0080);
  pub const REAR_CENTRE: Self = Self(0x0000_0100);
  pub const SIDE_LEFT: Self = Self(0x0000_0200);
  pub const SIDE_RIGHT: Self = Self(0x0000_0400);
  pub const TOP_CENTRE: Self = Self(0x0000_0800);
  pub const TOP_FRONT_LEFT: Self = Self(0x0000_1000);
  pub const TOP_FRONT_CENTRE: Self = Self(0x0000_2000);
  pub const TOP_FRONT_RIGHT: Self = Self(0x0000_4000);
  pub const TOP_REAR_LEFT: Self = Self(0x0000_8000);
  pub const TOP_REAR_CENTRE: Self = Self(0x0001_0000);
  pub const TOP_REAR_RIGHT: Self = Self(0x0002_0000);
  pub const REAR_LEFT_CENTRE: Self = Self(0x0004_0000);
  pub const REAR_RIGHT_CENTRE: Self = Self(0x0008_0000);
  pub const FRONT_LEFT_WIDE: Self = Self(0x0010_0000);
  pub const FRONT_RIGHT_WIDE: Self = Self(0x0020_0000);
  pub const FRONT_LEFT_HIGH: Self = Self(0x0040_0000);
  pub const FRONT_CENTRE_HIGH: Self = Self(0x0080_0000);
  pub const FRONT_RIGHT_HIGH: Self = Self(0x0100_0000);
  pub const LFE2: Self = Self(0x0200_0000);

  pub fn empty() -> Infallible<Self> {
    Ok(Self(0))
  }

  pub fn from_bits(bits: u32) -> Infallible<Option<Self>> {
    Ok(match bits & !0x03ff_ffff {
      0 => Some(Self(bits)),
      _unknown => None,
    })
  }

  pub fn bits(self) -> Infallible<u32> {
    Ok(self.0)
  }

  pub fn count(self) -> Infallible<u32> {
    Ok(self.0.count_ones())
  }
}

impl std::ops::BitOr for Channels {
  type Output = Self;

  fn bitor(self, other: Self) -> Self {
    Self(self.0 | other.0)
  }
}

impl std::ops::BitOrAssign for Channels {
  fn bitor_assign(&mut self, other: Self) {
    self.0 |= other.0;
  }
}

impl std::ops::BitAnd for Channels {
  type Output = Self;

  fn bitand(self, other: Self) -> Self {
    Self(self.0 & other.0)
  }
}

impl fmt::Debug for Channels {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self.0 {
      0 => return to.write_str("(empty)"),
      _channels => {}
    }

    let mut separator = "";

    for (channel, name) in [
      (Self::FRONT_LEFT, "FRONT_LEFT"),
      (Self::FRONT_RIGHT, "FRONT_RIGHT"),
      (Self::FRONT_CENTRE, "FRONT_CENTRE"),
      (Self::LFE1, "LFE1"),
      (Self::REAR_LEFT, "REAR_LEFT"),
      (Self::REAR_RIGHT, "REAR_RIGHT"),
      (Self::FRONT_LEFT_CENTRE, "FRONT_LEFT_CENTRE"),
      (Self::FRONT_RIGHT_CENTRE, "FRONT_RIGHT_CENTRE"),
      (Self::REAR_CENTRE, "REAR_CENTRE"),
      (Self::SIDE_LEFT, "SIDE_LEFT"),
      (Self::SIDE_RIGHT, "SIDE_RIGHT"),
      (Self::TOP_CENTRE, "TOP_CENTRE"),
      (Self::TOP_FRONT_LEFT, "TOP_FRONT_LEFT"),
      (Self::TOP_FRONT_CENTRE, "TOP_FRONT_CENTRE"),
      (Self::TOP_FRONT_RIGHT, "TOP_FRONT_RIGHT"),
      (Self::TOP_REAR_LEFT, "TOP_REAR_LEFT"),
      (Self::TOP_REAR_CENTRE, "TOP_REAR_CENTRE"),
      (Self::TOP_REAR_RIGHT, "TOP_REAR_RIGHT"),
      (Self::REAR_LEFT_CENTRE, "REAR_LEFT_CENTRE"),
      (Self::REAR_RIGHT_CENTRE, "REAR_RIGHT_CENTRE"),
      (Self::FRONT_LEFT_WIDE, "FRONT_LEFT_WIDE"),
      (Self::FRONT_RIGHT_WIDE, "FRONT_RIGHT_WIDE"),
      (Self::FRONT_LEFT_HIGH, "FRONT_LEFT_HIGH"),
      (Self::FRONT_CENTRE_HIGH, "FRONT_CENTRE_HIGH"),
      (Self::FRONT_RIGHT_HIGH, "FRONT_RIGHT_HIGH"),
      (Self::LFE2, "LFE2"),
    ] {
      match self.0 & channel.0 {
        0 => {}
        _present => {
          to.write_str(separator)?;
          to.write_str(name)?;

          separator = " | ";
        }
      }
    }

    Ok(())
  }
}

impl fmt::Display for Channels {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:#032b}", self.0)
  }
}

#[derive(Copy, Clone, Debug)]
pub enum Layout {
  Mono,
  Stereo,
  TwoPointOne,
  FivePointOne,
}

impl Layout {
  #[must_use]
  pub fn into_channels(self) -> std::result::Result<Channels, console_core_never::Never> {
    Ok(match self {
      Layout::Mono => Channels::FRONT_LEFT,
      Layout::Stereo => Channels::FRONT_LEFT | Channels::FRONT_RIGHT,
      Layout::TwoPointOne => Channels::FRONT_LEFT | Channels::FRONT_RIGHT | Channels::LFE1,
      Layout::FivePointOne => {
        Channels::FRONT_LEFT
          | Channels::FRONT_RIGHT
          | Channels::FRONT_CENTRE
          | Channels::REAR_LEFT
          | Channels::REAR_RIGHT
          | Channels::LFE1
      }
    })
  }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SignalSpec {
  pub rate: u32,
  pub channels: Channels,
}

impl SignalSpec {
  #[must_use]
  pub fn new(
    rate: u32,
    channels: Channels,
  ) -> std::result::Result<Self, console_core_never::Never> {
    Ok(SignalSpec { rate, channels })
  }

  #[must_use]
  pub fn new_with_layout(
    rate: u32,
    layout: Layout,
  ) -> std::result::Result<Self, console_core_never::Never> {
    let Ok(value_1) = layout.into_channels();

    Ok(SignalSpec {
      rate,
      channels: value_1,
    })
  }
}

pub struct AudioPlanes<'a, S: 'a + Sample> {
  planes: Vec<&'a [S]>,
}

impl<'a, S: Sample> AudioPlanes<'a, S> {
  #[must_use]
  pub fn planes(&self) -> std::result::Result<&[&'a [S]], console_core_never::Never> {
    Ok(&self.planes)
  }
}

pub struct AudioPlanesMut<'a, S: 'a + Sample> {
  planes: Vec<&'a mut [S]>,
}

impl<'a, S: Sample> AudioPlanesMut<'a, S> {
  pub fn planes(&mut self) -> std::result::Result<&mut [&'a mut [S]], console_core_never::Never> {
    Ok(&mut self.planes)
  }
}

#[derive(Clone)]
pub struct AudioBuffer<S: Sample> {
  buf: Vec<S>,
  spec: SignalSpec,
  n_frames: u32,
  n_capacity: u32,
}

type Infallible<T> = std::result::Result<T, Never>;

fn filled<S: Sample>(samples: u32) -> Result<Vec<S>> {
  let Ok(wanted) = index(samples);

  match std::alloc::Layout::array::<S>(wanted) {
    Ok(_layout) => {}
    Err(_too_large) => return limit_error("duration too large"),
  }

  let mut buf = Vec::new();

  #[cfg_attr(
    dylint_lib = "explicit088_no_arriving_length",
    expect(
      explicit088_no_arriving_length,
      reason = "Layout::array checks the sample count against the allocator's addressable byte limit before reserving"
    )
  )]
  let reserved = buf.try_reserve_exact(wanted);

  match reserved {
    Ok(()) => {
      buf.resize(wanted.min(buf.capacity()), S::MID);

      Ok(buf)
    }
    Err(_refused) => limit_error("duration too large"),
  }
}

impl<S: Sample> AudioBuffer<S> {
  pub fn new(duration: Duration, spec: SignalSpec) -> Result<Self> {
    let capacity = match u32::try_from(duration) {
      Ok(capacity) => capacity,
      Err(_too_long) => return limit_error("duration too large"),
    };
    let Ok(value_2) = spec.channels.count();

    let samples = match capacity.checked_mul(value_2) {
      Some(samples) => samples,
      None => return limit_error("duration too large"),
    };

    let buf = filled(samples)?;

    Ok(AudioBuffer {
      buf,
      spec,
      n_frames: 0,
      n_capacity: capacity,
    })
  }

  fn lane(&self, channel: u32) -> Infallible<&[S]> {
    let Ok(at) = index(channel);
    let Ok(stride) = index(self.n_capacity);
    let Ok(len) = index(self.n_frames);

    Ok(
      match self
        .buf
        .get(at.saturating_mul(stride)..)
        .and_then(|plane| plane.get(..len))
      {
        Some(lane) => lane,
        None => &[],
      },
    )
  }

  fn lane_mut(&mut self, channel: u32) -> Infallible<&mut [S]> {
    let Ok(at) = index(channel);
    let Ok(stride) = index(self.n_capacity);
    let Ok(len) = index(self.n_frames);

    Ok(
      match self
        .buf
        .get_mut(at.saturating_mul(stride)..)
        .and_then(|plane| plane.get_mut(..len))
      {
        Some(lane) => lane,
        None => &mut [],
      },
    )
  }

  fn ordered_pair_mut(&mut self, [low, high]: [u32; 2]) -> Infallible<(&mut [S], &mut [S])> {
    let Ok(low_2) = index(low);
    let Ok(high_2) = index(high);
    let Ok(stride) = index(self.n_capacity);
    let Ok(len) = index(self.n_frames);

    Ok(
      match self.buf.split_at_mut_checked(high_2.saturating_mul(stride)) {
        Some((head, tail)) => match (
          head
            .get_mut(low_2.saturating_mul(stride)..)
            .and_then(|plane| plane.get_mut(..len)),
          tail.get_mut(..len),
        ) {
          (Some(low_3), Some(high_3)) => (low_3, high_3),
          (None, _) | (_, None) => (&mut [], &mut []),
        },
        None => (&mut [], &mut []),
      },
    )
  }
}

impl<S: Sample> AudioBuffer<S> {
  #[must_use]
  pub fn unused() -> std::result::Result<Self, console_core_never::Never> {
    let Ok(value_3) = Channels::empty();

    let Ok(value_10) = SignalSpec::new(0, value_3);

    Ok(AudioBuffer {
      buf: Vec::new(),
      spec: value_10,
      n_frames: 0,
      n_capacity: 0,
    })
  }

  #[must_use]
  pub fn spec(&self) -> std::result::Result<&SignalSpec, console_core_never::Never> {
    Ok(&self.spec)
  }

  #[must_use]
  pub fn capacity(&self) -> std::result::Result<u32, console_core_never::Never> {
    Ok(self.n_capacity)
  }

  #[must_use]
  pub fn planes(&self) -> std::result::Result<AudioPlanes<'_, S>, console_core_never::Never> {
    let Ok(value_4) = self.spec.channels.count();

    Ok(AudioPlanes {
      planes: (0..value_4)
        .map(|channel| {
          let Ok(lane) = self.lane(channel);

          lane
        })
        .collect(),
    })
  }

  pub fn make_equivalent<E: Sample>(&self) -> Result<AudioBuffer<E>> {
    AudioBuffer::<E>::new(Duration::from(self.n_capacity), self.spec)
  }
}

enum BufferFit {
  Fits,
  Incompatible,
}

impl From<bool> for BufferFit {
  fn from(fits: bool) -> Self {
    match bool::then_some(fits, ()) {
      Some(()) => Self::Fits,
      None => Self::Incompatible,
    }
  }
}

impl<S: Sample> AudioBuffer<S> {
  pub fn convert<T: Sample>(&self, dest: &mut AudioBuffer<T>) -> Result<()>
  where
    S: IntoSample<T>,
  {
    let fit = BufferFit::from(dest.n_capacity >= self.n_capacity && dest.spec == self.spec);

    match fit {
      BufferFit::Fits => {}
      BufferFit::Incompatible => return limit_error("the destination cannot hold the source"),
    }

    dest.n_frames = self.n_frames;

    let Ok(value_5) = self.spec.channels.count();

    for channel in 0..value_5 {
      let Ok(from) = self.lane(channel);
      let Ok(to) = dest.lane_mut(channel);

      for (d, s) in to.iter_mut().zip(from) {
        *d = (*s).into_sample();
      }
    }

    Ok(())
  }
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one definition forwards a call to the buffer of whichever sample type a reference holds, which is ten arms that would otherwise be written out for every method"
  )
)]
macro_rules! impl_audio_buffer_ref_func {
  ($var:expr, $buf:ident,$expr:expr) => {
    match $var {
      AudioBufferRef::U8($buf) => $expr,
      AudioBufferRef::U16($buf) => $expr,
      AudioBufferRef::U24($buf) => $expr,
      AudioBufferRef::U32($buf) => $expr,
      AudioBufferRef::S8($buf) => $expr,
      AudioBufferRef::S16($buf) => $expr,
      AudioBufferRef::S24($buf) => $expr,
      AudioBufferRef::S32($buf) => $expr,
      AudioBufferRef::F32($buf) => $expr,
      AudioBufferRef::F64($buf) => $expr,
    }
  };
}

#[derive(Clone)]
pub enum AudioBufferRef<'a> {
  U8(Cow<'a, AudioBuffer<u8>>),
  U16(Cow<'a, AudioBuffer<u16>>),
  U24(Cow<'a, AudioBuffer<u24>>),
  U32(Cow<'a, AudioBuffer<u32>>),
  S8(Cow<'a, AudioBuffer<i8>>),
  S16(Cow<'a, AudioBuffer<i16>>),
  S24(Cow<'a, AudioBuffer<i24>>),
  S32(Cow<'a, AudioBuffer<i32>>),
  F32(Cow<'a, AudioBuffer<f32>>),
  F64(Cow<'a, AudioBuffer<f64>>),
}

impl AudioBufferRef<'_> {
  #[must_use]
  pub fn spec(&self) -> std::result::Result<&SignalSpec, console_core_never::Never> {
    impl_audio_buffer_ref_func!(self, buf, buf.spec())
  }

  #[must_use]
  pub fn capacity(&self) -> std::result::Result<u32, console_core_never::Never> {
    impl_audio_buffer_ref_func!(self, buf, buf.capacity())
  }

  #[must_use]
  pub fn frames(&self) -> std::result::Result<u32, console_core_never::Never> {
    impl_audio_buffer_ref_func!(self, buf, buf.frames())
  }

  pub fn convert<T>(&self, dest: &mut AudioBuffer<T>) -> Result<()>
  where
    T: Sample
      + FromSample<u8>
      + FromSample<u16>
      + FromSample<u24>
      + FromSample<u32>
      + FromSample<i8>
      + FromSample<i16>
      + FromSample<i24>
      + FromSample<i32>
      + FromSample<f32>
      + FromSample<f64>,
  {
    impl_audio_buffer_ref_func!(self, buf, buf.convert(dest))
  }

  pub fn make_equivalent<E: Sample>(&self) -> Result<AudioBuffer<E>> {
    impl_audio_buffer_ref_func!(self, buf, buf.make_equivalent::<E>())
  }
}

pub trait AsAudioBufferRef {
  fn as_audio_buffer_ref(
    &self,
  ) -> std::result::Result<AudioBufferRef<'_>, console_core_never::Never>;
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one definition lends a buffer of each of ten sample types as the matching reference variant"
  )
)]
macro_rules! impl_as_audio_buffer_ref {
  ($fmt:ty, $ref:path) => {
    impl AsAudioBufferRef for AudioBuffer<$fmt> {
      fn as_audio_buffer_ref(&self) -> std::result::Result<AudioBufferRef<'_>, Never> {
        Ok($ref(Cow::Borrowed(self)))
      }
    }
  };
}

impl_as_audio_buffer_ref!(u8, AudioBufferRef::U8);
impl_as_audio_buffer_ref!(u16, AudioBufferRef::U16);
impl_as_audio_buffer_ref!(u24, AudioBufferRef::U24);
impl_as_audio_buffer_ref!(u32, AudioBufferRef::U32);
impl_as_audio_buffer_ref!(i8, AudioBufferRef::S8);
impl_as_audio_buffer_ref!(i16, AudioBufferRef::S16);
impl_as_audio_buffer_ref!(i24, AudioBufferRef::S24);
impl_as_audio_buffer_ref!(i32, AudioBufferRef::S32);
impl_as_audio_buffer_ref!(f32, AudioBufferRef::F32);
impl_as_audio_buffer_ref!(f64, AudioBufferRef::F64);

#[cfg_attr(
  dylint_lib = "explicit024_no_swappable_params",
  allow(
    explicit024_no_swappable_params,
    reason = "two channels and a start and an end are the arguments every decoder already passes in this order"
  )
)]
pub trait Signal<S: Sample> {
  fn frames(&self) -> std::result::Result<u32, console_core_never::Never>;

  fn clear(&mut self) -> std::result::Result<(), console_core_never::Never>;

  fn chan(&self, channel: u32) -> std::result::Result<&[S], console_core_never::Never>;

  fn chan_mut(
    &mut self,
    channel: u32,
  ) -> std::result::Result<&mut [S], console_core_never::Never>;

  fn chan_pair_mut(
    &mut self,
    first: u32,
    second: u32,
  ) -> std::result::Result<(&mut [S], &mut [S]), console_core_never::Never>;

  fn render_silence(&mut self, n_frames: Option<u32>) -> Result<()>;

  fn render_reserved(&mut self, n_frames: Option<u32>) -> Result<()>;

  fn render<'a, F>(&'a mut self, n_frames: Option<u32>, render: F) -> Result<()>
  where
    F: FnMut(&mut AudioPlanesMut<'a, S>, u32) -> Result<()>;

  #[inline]
  fn fill<'a, F>(&'a mut self, fill: F) -> Result<()>
  where
    F: FnMut(&mut AudioPlanesMut<'a, S>, u32) -> Result<()>,
  {
    let Ok(()) = self.clear();
    self.render(None, fill)
  }

  fn transform<F>(&mut self, f: F) -> std::result::Result<(), console_core_never::Never>
  where
    F: Fn(S) -> S;

  fn truncate(&mut self, n_frames: u32) -> std::result::Result<(), console_core_never::Never>;

  fn shift(&mut self, shift: u32) -> std::result::Result<(), console_core_never::Never>;

  fn trim(&mut self, start: u32, end: u32) -> std::result::Result<(), console_core_never::Never> {
    let Ok(frames) = self.frames();
    let Ok(()) = self.truncate(frames.saturating_sub(end));

    let Ok(()) = self.shift(start);

    Ok(())
  }
}

fn rendered_end([have, capacity]: [u32; 2], asked: Option<u32>) -> Result<u32> {
  let wanted = match asked {
    Some(wanted) => wanted,
    None => capacity.saturating_sub(have),
  };

  match have.checked_add(wanted) {
    Some(end) => match end.cmp(&capacity) {
      Ordering::Less | Ordering::Equal => Ok(end),
      Ordering::Greater => limit_error("capacity will be exceeded"),
    },
    None => limit_error("capacity will be exceeded"),
  }
}

impl<S: Sample> Signal<S> for AudioBuffer<S> {
  fn clear(&mut self) -> std::result::Result<(), console_core_never::Never> {
    self.n_frames = 0;

    Ok(())
  }

  fn frames(&self) -> std::result::Result<u32, console_core_never::Never> {
    Ok(self.n_frames)
  }

  fn chan(&self, channel: u32) -> std::result::Result<&[S], console_core_never::Never> {
    let Ok(lane) = self.lane(channel);

    Ok(lane)
  }

  fn chan_mut(
    &mut self,
    channel: u32,
  ) -> std::result::Result<&mut [S], console_core_never::Never> {
    let Ok(lane) = self.lane_mut(channel);

    Ok(lane)
  }

  fn chan_pair_mut(
    &mut self,
    first: u32,
    second: u32,
  ) -> std::result::Result<(&mut [S], &mut [S]), console_core_never::Never> {
    Ok(match first.cmp(&second) {
      Ordering::Less => {
        let Ok(pair) = self.ordered_pair_mut([first, second]);

        pair
      }
      Ordering::Greater => {
        let Ok((low, high)) = self.ordered_pair_mut([second, first]);

        (high, low)
      }
      Ordering::Equal => (&mut [], &mut []),
    })
  }

  fn render_silence(&mut self, n_frames: Option<u32>) -> Result<()> {
    let end = rendered_end([self.n_frames, self.n_capacity], n_frames)?;
    let from = self.n_frames;

    self.n_frames = end;

    let Ok(value_6) = self.spec.channels.count();

    for channel in 0..value_6 {
      let Ok(plane) = self.lane_mut(channel);
      let Ok(first) = index(from);

      match plane.get_mut(first..) {
        Some(silent) => silent.fill(S::MID),
        None => {}
      }
    }

    Ok(())
  }

  fn render_reserved(&mut self, n_frames: Option<u32>) -> Result<()> {
    let end = rendered_end([self.n_frames, self.n_capacity], n_frames)?;

    self.n_frames = end;

    Ok(())
  }

  fn render<'a, F>(&'a mut self, n_frames: Option<u32>, mut render: F) -> Result<()>
  where
    F: FnMut(&mut AudioPlanesMut<'a, S>, u32) -> Result<()>,
  {
    let end = rendered_end([self.n_frames, self.n_capacity], n_frames)?;
    let Ok(first) = index(self.n_frames);
    let Ok(last) = index(end);
    let Ok(stride) = index(self.n_capacity);

    let mut planes = AudioPlanesMut {
      planes: self
        .buf
        .chunks_exact_mut(stride.max(1))
        .filter_map(|plane| plane.get_mut(first..last))
        .collect(),
    };

    for frame in self.n_frames..end {
      render(&mut planes, frame)?;
      self.n_frames = frame.saturating_add(1);
    }

    Ok(())
  }

  fn transform<F>(&mut self, f: F) -> std::result::Result<(), console_core_never::Never>
  where
    F: Fn(S) -> S,
  {
    let Ok(value_7) = self.spec.channels.count();

    for channel in 0..value_7 {
      let Ok(lane) = self.lane_mut(channel);

      for sample in lane {
        *sample = f(*sample);
      }
    }

    Ok(())
  }

  fn truncate(&mut self, n_frames: u32) -> std::result::Result<(), console_core_never::Never> {
    self.n_frames = self.n_frames.min(n_frames);

    Ok(())
  }

  fn shift(&mut self, shift: u32) -> std::result::Result<(), console_core_never::Never> {
    match (shift.cmp(&self.n_frames), shift) {
      (Ordering::Equal | Ordering::Greater, _) => {
        let Ok(()) = self.clear();
      }
      (Ordering::Less, 1..) => {
        let Ok(from) = index(shift);

        let Ok(value_8) = self.spec.channels.count();

        for channel in 0..value_8 {
          let Ok(lane) = self.lane_mut(channel);

          match lane.len().cmp(&from) {
            Ordering::Greater => lane.copy_within(from.., 0),
            Ordering::Less | Ordering::Equal => {}
          }
        }

        self.n_frames = self.n_frames.saturating_sub(shift);
      }
      (Ordering::Less, 0) => {}
    };

    Ok(())
  }
}

pub struct SampleBuffer<S: Sample> {
  buf: Box<[S]>,
  n_written: u32,
}

impl<S: Sample> SampleBuffer<S> {
  pub fn new(duration: Duration, spec: SignalSpec) -> Result<SampleBuffer<S>> {
    let capacity = match u32::try_from(duration) {
      Ok(capacity) => capacity,
      Err(_too_long) => return limit_error("duration too large"),
    };
    let Ok(value_9) = spec.channels.count();

    let samples = match capacity.checked_mul(value_9) {
      Some(samples) => samples,
      None => return limit_error("duration too large"),
    };

    let filled = filled(samples)?;
    let buf = filled.into_boxed_slice();

    Ok(SampleBuffer { buf, n_written: 0 })
  }

  #[must_use]
  pub fn samples(&self) -> std::result::Result<&[S], console_core_never::Never> {
    let Ok(written) = index(self.n_written);

    Ok(match self.buf.get(..written) {
      Some(samples) => samples,
      None => &[],
    })
  }

  pub fn copy_interleaved_ref(&mut self, src: AudioBufferRef) -> Result<()>
  where
    S: ConvertibleSample,
  {
    match src {
      AudioBufferRef::U8(buf) => self.copy_interleaved_typed(&buf),
      AudioBufferRef::U16(buf) => self.copy_interleaved_typed(&buf),
      AudioBufferRef::U24(buf) => self.copy_interleaved_typed(&buf),
      AudioBufferRef::U32(buf) => self.copy_interleaved_typed(&buf),
      AudioBufferRef::S8(buf) => self.copy_interleaved_typed(&buf),
      AudioBufferRef::S16(buf) => self.copy_interleaved_typed(&buf),
      AudioBufferRef::S24(buf) => self.copy_interleaved_typed(&buf),
      AudioBufferRef::S32(buf) => self.copy_interleaved_typed(&buf),
      AudioBufferRef::F32(buf) => self.copy_interleaved_typed(&buf),
      AudioBufferRef::F64(buf) => self.copy_interleaved_typed(&buf),
    }
  }

  pub fn copy_interleaved_typed<F>(&mut self, src: &AudioBuffer<F>) -> Result<()>
  where
    F: Sample + IntoSample<S>,
  {
    let Ok(channels) = src.spec.channels.count();
    let samples = src.n_frames.saturating_mul(channels);
    let Ok(stride) = index(channels);
    let Ok(wanted) = index(samples);

    match self.buf.len().cmp(&wanted) {
      Ordering::Equal | Ordering::Greater => {}
      Ordering::Less => return limit_error("the samples do not fit the buffer"),
    }

    for channel in 0..channels {
      let Ok(first) = index(channel);
      let Ok(lane) = src.lane(channel);

      match self.buf.get_mut(first..) {
        Some(rest) => {
          for (dst, sample) in rest.iter_mut().step_by(stride.max(1)).zip(lane) {
            *dst = (*sample).into_sample();
          }
        }
        None => {}
      }
    }

    self.n_written = samples;

    Ok(())
  }
}
