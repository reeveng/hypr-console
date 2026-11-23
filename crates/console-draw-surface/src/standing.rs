//! A layer surface, and the frame behind it.
//!
//! ## Why the buffer scale is always one
//!
//! The obvious way to draw at 2.5 is to hand the compositor a buffer two and a
//! half times the logical size and say so, and there is no way to say it:
//! `set_buffer_scale` takes an integer and always has. What `wp_viewporter`
//! adds is the other end of the same sentence. The buffer is whatever size we
//! cut it to, the buffer scale stays at one, and `set_destination` says how
//! large that buffer is to appear in logical pixels. The ratio between the two
//! is the fractional scale, expressed without either side having to name it.
//!
//! So [`Scale`] is asked of `wp_fractional_scale_v1` and spent in exactly one
//! place -- cutting the frame -- and everything above this crate goes on
//! measuring in points, which is what every number in this tree already is.
//!
//! ## A surface that reserves room takes it off the edge it is anchored to
//!
//! The exclusive zone is one number and the compositor reads it against the
//! anchor, so a bar along the top hands over its height and a strip down a side
//! would hand over its width. Getting that the wrong way round is a desktop
//! whose windows start a screen's width in, which is why the anchor decides it
//! here rather than at a call site.
//!
//! ## The first frame may be at the wrong scale, and that is not a fault
//!
//! `preferred_scale` is not part of the configure handshake and can arrive
//! after it. A surface that waited for one would hang on a compositor that
//! never sends it, which is every compositor without the staging protocol. So
//! the scale starts at one, the first frame is drawn at one if that is all we
//! have been told, and a scale arriving later makes the frame stale the same
//! way a resize does. What that looks like once is a card that is soft for a
//! frame; what waiting for it looks like is a desktop that does not come up.
//!
//! ## What a surface goes over, and what goes over it
//!
//! The layer shell has four of them and this desktop uses three. A card and a
//! panel are `Overlay`, which is over everything including a game someone has
//! put full screen. The bar is `Top`, which is over the windows and under the
//! overlay -- so the keyboard comes up over it and a panel covers it, which is
//! what happened when the bar was waybar's and is what anyone holding the
//! device already expects. The home screen is `Bottom`, which is over the
//! wallpaper and under everything else: it is the desktop rather than a thing
//! on top of it, so a window opened on the workspace covers it the way a
//! window covers a desktop. [`Under`] is that choice said as what may cover
//! this surface rather than as a number in someone else's protocol.
//!
//! ## A pointer that goes says so, and a finger that lifts does not
//!
//! A surface hears a pointer only while it is over it, so a highlight that
//! follows the pointer has no way to learn that it has gone: the last thing
//! said is a motion at the edge, and the highlight stays on a square nobody is
//! pointing at any more. `wl_pointer` has the other half of that, a leave, and
//! [`PointerEvent::Left`] is it, beside the presses rather than off to one
//! side in a state to be asked after -- where the pointer is is one story, and
//! a surface that reads it as one gets the order right for nothing. A finger
//! is not in it: a touch that ends is an up, which is a gesture finishing
//! rather than a pointer that is now somewhere else.
//!
//! A pointer arriving is not a pointer moving. A surface that comes up under
//! a pointer resting on the screen is told the pointer entered, at wherever it
//! rests, and that used to be read as a motion: every panel opened with the
//! row under the hidden pointer highlighted, which on the device -- whose
//! pointer rests in the middle of the glass -- was a panel opening halfway
//! down itself. So entering says where the pointer is and nothing more, and
//! the highlight follows it once it moves, as a menu opened under a mouse does.
//! Hyprland also sends a motion at the entered position when a layer opens.
//! That is still a resting pointer: only a change in position is a move.
//!
//! ## A key arrives as a code and is read through the person's own keymap
//!
//! The compositor hands over the keymap it is driving and then keycodes
//! against it, and what a panel wants to know is which keysym a press means --
//! which is a question the keymap answers and a table here would get wrong for
//! anybody typing in a language this device was not set up in. The eight
//! between an evdev code and an xkb one is historical and never changes, and
//! `Keycode::from_evdev` is where it is written.
//!
//! The keymap is read by `console-input-keymaps`, which compiles the text the
//! compositor hands over into a table of what each key says under each
//! modifier and group. Which modifiers and group are held is the compositor's
//! to say, and it says it in every modifiers event, so the table is asked with
//! what it last said rather than with a state kept here and kept in step.
//!
//! Whether the surface holds the keyboard is kept as well, from the enter and
//! leave the compositor sends, because a key pressed before the enter goes to
//! whoever held the keyboard before, and a check that presses the moment a
//! panel has drawn is pressing at a surface that may not be listening yet.
//!
//! A key that is held down arrives once. Repeat is the client's to run, from
//! the rate the compositor sends in `repeat_info`, and nothing above this has
//! asked for one: a d-pad held on a list is a step per press today. When
//! something does ask, the rate is already on the wire and the timer belongs
//! to whoever is waiting, not here.
//!
//! ## Every frame is timed to the glass
//!
//! Each commit asks `wp_presentation` for word of when it was shown, and
//! `console_response_times::frames` keeps the tally: how long the painting
//! took, how long the compositor took after it, whether the frame came a
//! refresh late, and whether it was painted into a buffer the compositor had
//! not handed back yet. A compositor without the protocol gets its frames
//! counted and not timed. The surface's namespace is who the lines are about,
//! and the tally is written down when the surface goes.
//!
//! ## Completion is presentation, rather than a successful commit
//!
//! `draw_notifying` registers feedback before committing the frame. A caller
//! receives Presented only when the compositor confirms it; Discarded keeps
//! the request pending for a replacement frame. Missing presentation support
//! reports Unavailable. Hiding or replacing the request reports Cancelled,
//! and feedback from that old request cannot complete its replacement.
//!
//! ## A frame is painted into a buffer the compositor has let go of
//!
//! There used to be one buffer per surface, painted over where it stood. A
//! commit hands the compositor the pixels and a `release` hands them back, and
//! in between it may be reading them: a frame painted in that window reaches
//! the screen cleared and half drawn, which on a list scrolled under a finger
//! was the whole menu blinking out. The tally below counted those frames on
//! every opening of the launcher. So the surface keeps the buffer on screen
//! and spares beside it, paints into a spare the compositor has released, and
//! cuts another when none is, which is as many as the compositor holds at once
//! and no more. Painting over part of a frame starts from a copy of the one on
//! the screen, because the spare holds an older one.
//!
//! ## A lock is the same surface with another role
//!
//! `ext-session-lock-v1` hands out a surface the compositor shows instead of
//! the whole desktop, and everything after it is sized, scaled and painted the
//! way a layer is -- so it is a second [`Role`] on the one surface rather than
//! a second surface type. What is its own is the order: the compositor sends
//! `locked` only once a frame is on every screen, `unlock_and_destroy` before
//! that is a protocol error, and so [`Surface::unlock`] answers
//! [`Unlocked::NotYet`] until [`Lock::Acquired`] rather than letting go of a lock
//! that was never held. A lock surface is sized by the compositor and nothing
//! else, which is why resizing one and giving it room are nothing.
//!
//! The lock is bound to the first screen only. A handheld has one, and an
//! output the lock has no surface on is painted a solid color by the
//! compositor rather than shown -- which is the right fault for a lock to have.
//!
//! ## A window is the same surface with a third role
//!
//! An app -- the books, the files, the pictures -- is somewhere a person stays,
//! and a layer over the whole screen was a place they could only leave by
//! putting it away: nothing could be opened beside it, and the settings opened
//! over it fought it for the top. `xdg_toplevel` is the role every other
//! program's window has, and the compositor places it, tiles it and puts it on
//! a workspace of its own the way it does theirs. Everything after the role is
//! a layer's: sized by the compositor, scaled and painted here.
//!
//! What is the window's own is that the compositor may leave its size to us. A
//! window is not on the screen until a buffer is, and a compositor that tiles
//! it says how large it is only once it is; so a size of nothing is drawn at
//! one pixel, which is enough to be placed, and the size the compositor gives
//! it next is a resize like any other. A window can be asked to close, and that
//! is [`Closed::Yes`], the same answer a layer gets when its screen goes.
//!
//! ## A closed socket is not quiet
//!
//! `poll` asked only about `POLLIN` returns immediately and forever on a
//! descriptor whose far end has gone, because the hangup is on `POLLHUP` and
//! nothing reads it. That is a program at a whole core drawing on a screen that
//! is not there. It is asked about by name here.
//!
//! Each recorded `KeyPress` includes the effective XKB modifiers at the
//! time of the press.

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_i32};
use rustix::event::{Nsecs, PollFd, PollFlags, Secs, Timespec, poll};
use std::io;
use std::ops::ControlFlow;
use std::os::fd::{AsFd, BorrowedFd};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::time::Duration;

use wayland_client::backend::WaylandError;
use wayland_client::globals::{BindError, GlobalList, GlobalListContents, registry_queue_init};
use wayland_client::protocol::{
  wl_buffer, wl_compositor, wl_keyboard, wl_output, wl_pointer, wl_region, wl_registry, wl_seat,
  wl_shm, wl_shm_pool, wl_subcompositor, wl_subsurface, wl_surface, wl_touch,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, delegate_noop};
use wayland_protocols::ext::session_lock::v1::client::{
  ext_session_lock_manager_v1, ext_session_lock_surface_v1, ext_session_lock_v1,
};
use wayland_protocols::wp::fractional_scale::v1::client::{
  wp_fractional_scale_manager_v1, wp_fractional_scale_v1,
};
use wayland_protocols::wp::presentation_time::client::{
  wp_presentation, wp_presentation_feedback,
};
use wayland_protocols::wp::text_input::zv3::client::{
  zwp_text_input_manager_v3, zwp_text_input_v3,
};

use console_response_times::frames::{Buffer, Committed, Frames, Presented};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};
use wayland_protocols_wlr::layer_shell::v1::client::{
  zwlr_layer_shell_v1, zwlr_layer_surface_v1,
};

use console_input_keymaps::{
  Components, Context, Diagnostic, GroupIndex, Keycode, LookupTable, ModifierMask, Severity,
};

pub use console_input_key_symbols::Keysym;

use crate::fingers::{Fingers, Touch};
use crate::memory::Shared;
use crate::region::{self, Fit};
use crate::scale::Scale;
use crate::text_input::{Composition, Input, TextFocus, TextInputChange, TextInputState};

const SECONDS_IN_THE_HIGH_WORD: u64 = 0x1_0000_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
  Top,
  TopRight,
  Bottom,
  Left,
  Right,
  Whole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Under {
  AWindow,
  Anything,
  None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyboard {
  Takes,
  OnDemand,
  Declines,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardFocus {
  Here,
  Elsewhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Margin {
  pub top: i32,
  pub right: i32,
  pub bottom: i32,
  pub left: i32,
}

impl Margin {
  pub fn none() -> Result<Margin, Never> {
    Ok(Margin::default())
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Room {
  Reserves,
  Over,
  Around,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
  pub namespace: String,
  pub anchor: Anchor,
  pub size: Size<u32>,
  pub margin: Margin,
  pub keyboard: Keyboard,
  pub room: Room,
  pub under: Under,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
  pub app_id: String,
  pub title: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerEvent {
  Down { at: (f64, f64) },
  Moved { at: (f64, f64) },
  Scrolled { by: f64 },
  Pinched { by: f64 },
  Up,
  Left,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardEvent {
  Down { key: Keysym },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPress {
  pub key: Keysym,
  pub modifiers: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lock {
  Waiting,
  Acquired,
  Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unlocked {
  Yes,
  NotYet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closed {
  Yes,
  No,
}

#[derive(Debug)]
pub enum SurfaceError {
  Compositor(wayland_client::ConnectError),
  Global(&'static str),
  Went(wayland_client::DispatchError),
  Hung,
  Memory(io::Error),
}

impl std::fmt::Display for SurfaceError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      SurfaceError::Compositor(_) => write!(to, "no compositor answered on WAYLAND_DISPLAY"),
      SurfaceError::Global(what) => write!(to, "the compositor has no {what}"),
      SurfaceError::Went(why) => write!(to, "the compositor went away: {why}"),
      SurfaceError::Hung => write!(to, "the compositor closed the connection"),
      SurfaceError::Memory(why) => write!(to, "no memory for a frame: {why}"),
    }
  }
}

impl std::error::Error for SurfaceError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Part {
  pub at: Point<i32>,
  pub size: Size<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Damage {
  pub at: Point<i32>,
  pub size: Size<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visible {
  Yes,
  NotYet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerSupport {
  Available,
  Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drawing {
  Presented,
  Discarded,
  Unavailable,
  Cancelled,
}

pub struct DrawingSender(SyncSender<Drawing>);

pub fn drawing_notifications() -> Result<(DrawingSender, Receiver<Drawing>), Never> {
  let (sender, receiver) = sync_channel(2);
  Ok((DrawingSender(sender), receiver))
}

impl DrawingSender {
  fn notify(&self, drawing: Drawing) -> Result<(), Never> {
    match self.0.try_send(drawing) {
      Ok(()) => Ok(()),
      Err(TrySendError::Disconnected(_listener_has_left)) => Ok(()),
      Err(TrySendError::Full(_notifications_are_already_waiting)) => Ok(()),
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct DrawingId(u64);

struct PendingDrawing {
  id: DrawingId,
  notice: DrawingSender,
  reported_discard: bool,
}

impl PendingDrawing {
  fn new(id: DrawingId, sender: DrawingSender) -> Result<Self, Never> {
    Ok(Self {
      id,
      notice: sender,
      reported_discard: false,
    })
  }
}

fn cancel_drawing(pending: &mut Option<PendingDrawing>) -> Result<(), Never> {
  match pending.take() {
    Some(drawing) => drawing.notice.notify(Drawing::Cancelled),
    None => Ok(()),
  }
}

fn finish_drawing(
  pending: &mut Option<PendingDrawing>,
  received: DrawingId,
  finished: Drawing,
) -> Result<(), Never> {
  let current = match pending.as_mut() {
    Some(drawing) => drawing,
    None => return Ok(()),
  };

  match ::core::primitive::bool::then_some(current.id == received, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => return Ok(()),
  }

  match (finished, current.reported_discard) {
    (Drawing::Discarded, true) => return Ok(()),
    (Drawing::Discarded, false) => {
      current.reported_discard = true;

      current.notice.notify(finished)
    }
    (Drawing::Presented | Drawing::Cancelled | Drawing::Unavailable, _) => {
      let Ok(()) = current.notice.notify(finished);

      *pending = None;

      Ok(())
    }
  }
}

pub fn on_the_device(part: Part, logical: Size<u32>, device: Size<u32>) -> Result<Damage, Never> {
  let across = match logical.width {
    0 => 1.0,
    wide => f64::from(device.width) / f64::from(wide),
  };
  let down = match logical.height {
    0 => 1.0,
    tall => f64::from(device.height) / f64::from(tall),
  };
  let (left, top) = (f64::from(part.at.x), f64::from(part.at.y));
  let (right, bottom) = (
    left + f64::from(part.size.width),
    top + f64::from(part.size.height),
  );
  let Ok(left_2) = toward_zero_i32((left * across).floor());
  let Ok(top_2) = toward_zero_i32((top * down).floor());
  let Ok(right_2) = toward_zero_i32((right * across).ceil());
  let Ok(bottom_2) = toward_zero_i32((bottom * down).ceil());

  Ok(Damage {
    at: Point {
      x: left_2,
      y: top_2,
    },
    size: Size {
      width: right_2.saturating_sub(left_2),
      height: bottom_2.saturating_sub(top_2),
    },
  })
}

enum Role {
  Layer(zwlr_layer_surface_v1::ZwlrLayerSurfaceV1),
  Lock(ext_session_lock_surface_v1::ExtSessionLockSurfaceV1),
  Window(xdg_surface::XdgSurface, xdg_toplevel::XdgToplevel),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ready {
  No,
  Yes,
}

struct Up {
  surface: wl_surface::WlSurface,
  role: Role,
  viewport: wp_viewport::WpViewport,
  fraction: Option<wp_fractional_scale_v1::WpFractionalScaleV1>,
}

struct RegionLayer {
  surface: wl_surface::WlSurface,
  role: wl_subsurface::WlSubsurface,
  viewport: wp_viewport::WpViewport,
  at: Option<Point<i32>>,
  shown: Option<Frame>,
  spare: Vec<Frame>,
}

impl Drop for RegionLayer {
  fn drop(&mut self) {
    self.role.destroy();
    self.viewport.destroy();
    self.surface.destroy();
  }
}

struct Frame {
  _shared: Shared,
  pool: wl_shm_pool::WlShmPool,
  buffer: wl_buffer::WlBuffer,
  size: Size<u32>,
  held: Buffer,
}

impl Frame {
  fn new(
    shm: &wl_shm::WlShm,
    hand: &QueueHandle<Bound>,
    size: Size<u32>,
  ) -> Result<Frame, SurfaceError> {
    let stride = size.width.saturating_mul(4);
    let long = stride.saturating_mul(size.height);
    let bytes = long.max(4);
    let shared = Shared::of(u64::from(bytes)).map_err(SurfaceError::Memory)?;
    let Ok(held) = shared.descriptor();
    let Ok(many) = fitted::<u32, i32>(bytes);
    let pool = shm.create_pool(held.as_fd(), many, hand, ());
    let Ok(wide) = fitted::<u32, i32>(size.width);
    let Ok(tall) = fitted::<u32, i32>(size.height);
    let Ok(across) = fitted::<u32, i32>(stride);
    let buffer = pool.create_buffer(0, wide, tall, across, wl_shm::Format::Argb8888, hand, ());

    Ok(Frame {
      _shared: shared,
      pool,
      buffer,
      size,
      held: Buffer::Released,
    })
  }
}

fn free(
  spare: &mut Vec<Frame>,
  shm: &wl_shm::WlShm,
  hand: &QueueHandle<Bound>,
  device: Size<u32>,
) -> Result<Frame, SurfaceError> {
  let Ok((taken, kept)) = pick_free(std::mem::take(spare), device, |frame| {
    (frame.size, frame.held)
  });

  *spare = kept;

  match taken {
    Some(frame) => Ok(frame),
    None => Frame::new(shm, hand, device),
  }
}

fn pick_free<T>(
  spare: Vec<T>,
  device: Size<u32>,
  state: impl Fn(&T) -> (Size<u32>, Buffer),
) -> Result<(Option<T>, Vec<T>), Never> {
  let (mut free, mut busy): (Vec<T>, Vec<T>) = spare
    .into_iter()
    .filter(|frame| state(frame).0 == device)
    .partition(|frame| state(frame).1 == Buffer::Released);

  let taken = free.pop();

  busy.extend(free);

  Ok((taken, busy))
}

impl Bound {
  fn cancel_drawing(&mut self) -> Result<(), Never> {
    cancel_drawing(&mut self.drawing)
  }

  fn present(&mut self, frame: Frame) -> Result<(), Never> {
    let before = self.shown.replace(frame);

    self.spare.extend(before);

    Ok(())
  }
}

impl Drop for Frame {
  fn drop(&mut self) {
    self.buffer.destroy();
    self.pool.destroy();
  }
}

pub struct Bound {
  compositor: wl_compositor::WlCompositor,
  subcompositor: Option<wl_subcompositor::WlSubcompositor>,
  shm: wl_shm::WlShm,
  shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
  seat: Option<wl_seat::WlSeat>,
  windows: Option<xdg_wm_base::XdgWmBase>,
  locker: Option<ext_session_lock_manager_v1::ExtSessionLockManagerV1>,
  output: Option<wl_output::WlOutput>,
  session: Option<ext_session_lock_v1::ExtSessionLockV1>,
  lock: Lock,
  viewporter: wp_viewporter::WpViewporter,
  fractions: Option<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1>,
  presentation: Option<wp_presentation::WpPresentation>,
  clock: Option<u32>,
  timing: Option<Frames>,
  drawing: Option<PendingDrawing>,
  drawing_id: u64,
  up: Option<Up>,
  region: Option<RegionLayer>,
  shown: Option<Frame>,
  spare: Vec<Frame>,
  logical: Option<Size<u32>>,
  asked: Option<Size<u32>>,
  given: Size<u32>,
  anchor: Anchor,
  room: Room,
  scale: Scale,
  closed: Closed,
  pointer_events: Vec<PointerEvent>,
  pointer_at: (f64, f64),
  pointer_serial: Option<u32>,
  fingers: Fingers,
  typing: Option<Typing>,
  text_input: Option<Input>,
  keyboard_events: Vec<KeyPress>,
  focus: KeyboardFocus,
}

pub struct Surface {
  connection: Connection,
  queue: EventQueue<Bound>,
  bound: Bound,
}

struct Extensions {
  seat: Option<wl_seat::WlSeat>,
  windows: Option<xdg_wm_base::XdgWmBase>,
  locker: Option<ext_session_lock_manager_v1::ExtSessionLockManagerV1>,
  output: Option<wl_output::WlOutput>,
  fractions: Option<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1>,
  presentation: Option<wp_presentation::WpPresentation>,
}

fn extensions(globals: &GlobalList, hand: &QueueHandle<Bound>) -> Result<Extensions, Never> {
  let fractions = match globals
    .bind::<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1, _, _>(hand, 1..=1, ())
  {
    Ok(manager) => Some(manager),
    Err(_not_offered) => {
      console_core_logging::warn!(
        "this compositor has no wp_fractional_scale_v1, so every surface here \
                   is drawn at whole pixels and a screen driven at a fraction will be soft"
      );

      None
    }
  };
  let presentation = match globals.bind::<wp_presentation::WpPresentation, _, _>(hand, 1..=1, ())
  {
    Ok(presentation) => Some(presentation),
    Err(_not_offered) => {
      console_core_logging::warn!(
        "this compositor has no wp_presentation, so every frame here is counted \
                   and none of them is timed"
      );

      None
    }
  };
  let seat = match globals.bind::<wl_seat::WlSeat, _, _>(hand, 1..=7, ()) {
    Ok(seat) => Some(seat),
    Err(_no_seat_has_been_offered) => None,
  };
  let windows = match globals.bind::<xdg_wm_base::XdgWmBase, _, _>(hand, 1..=6, ()) {
    Ok(windows) => Some(windows),
    Err(_not_offered) => None,
  };
  let locker = match globals.bind::<ext_session_lock_manager_v1::ExtSessionLockManagerV1, _, _>(
    hand,
    1..=1,
    (),
  ) {
    Ok(locker) => Some(locker),
    Err(_not_offered) => None,
  };
  let output = match globals.bind::<wl_output::WlOutput, _, _>(hand, 1..=4, ()) {
    Ok(output) => Some(output),
    Err(_not_offered) => None,
  };

  Ok(Extensions {
    seat,
    windows,
    locker,
    output,
    fractions,
    presentation,
  })
}

enum Flushed {
  Sent,
  Pending,
}

fn text_input_for(
  globals: &GlobalList,
  hand: &QueueHandle<Bound>,
  seat: Option<&wl_seat::WlSeat>,
) -> Result<Option<Input>, Never> {
  let input_seat = match seat {
    Some(available_seat) => available_seat,
    None => return Ok(None),
  };
  let manager =
    match globals.bind::<zwp_text_input_manager_v3::ZwpTextInputManagerV3, _, _>(hand, 1..=1, ())
    {
      Ok(manager) => manager,
      Err(BindError::NotPresent) => return Ok(None),
      Err(fault @ BindError::UnsupportedVersion) => {
        console_core_logging::warn!("Wayland text input is unavailable: {fault}");
        return Ok(None);
      }
    };
  let protocol = manager.get_text_input(input_seat, hand, ());
  manager.destroy();
  let Ok(input) = Input::new(protocol);

  Ok(Some(input))
}

impl Surface {
  pub fn connect() -> Result<Surface, SurfaceError> {
    let connection = Connection::connect_to_env().map_err(SurfaceError::Compositor)?;
    let (globals, queue) = registry_queue_init::<Bound>(&connection)
      .map_err(|_| SurfaceError::Global("a list of globals"))?;
    let hand = queue.handle();

    let compositor = globals
      .bind::<wl_compositor::WlCompositor, _, _>(&hand, 1..=6, ())
      .map_err(|_| SurfaceError::Global("wl_compositor"))?;
    let subcompositor =
      match globals.bind::<wl_subcompositor::WlSubcompositor, _, _>(&hand, 1..=1, ()) {
        Ok(manager) => Some(manager),
        Err(BindError::NotPresent) => None,
        Err(fault @ BindError::UnsupportedVersion) => {
          console_core_logging::warn!("Wayland surface layers are unavailable: {fault}");
          None
        }
      };
    let shm = globals
      .bind::<wl_shm::WlShm, _, _>(&hand, 1..=1, ())
      .map_err(|_| SurfaceError::Global("wl_shm"))?;
    let shell =
      match globals.bind::<zwlr_layer_shell_v1::ZwlrLayerShellV1, _, _>(&hand, 1..=4, ()) {
        Ok(shell) => Some(shell),
        Err(_layer_shell_is_not_offered) => None,
      };
    let viewporter = globals
      .bind::<wp_viewporter::WpViewporter, _, _>(&hand, 1..=1, ())
      .map_err(|_| SurfaceError::Global("wp_viewporter, which is the whole of drawing at 2.5"))?;
    let Ok(extensions) = extensions(&globals, &hand);
    let Ok(text_input) = text_input_for(&globals, &hand, extensions.seat.as_ref());

    let bound = Bound {
      compositor,
      subcompositor,
      shm,
      shell,
      seat: extensions.seat,
      windows: extensions.windows,
      locker: extensions.locker,
      output: extensions.output,
      session: None,
      lock: Lock::Waiting,
      viewporter,
      fractions: extensions.fractions,
      presentation: extensions.presentation,
      clock: None,
      timing: None,
      drawing: None,
      drawing_id: 0,
      up: None,
      region: None,
      shown: None,
      spare: Vec::new(),
      logical: None,
      asked: None,
      given: Size {
        width: 0,
        height: 0,
      },
      anchor: Anchor::Whole,
      room: Room::Over,
      scale: Scale::ONE,
      closed: Closed::No,
      pointer_events: Vec::new(),
      pointer_at: (0.0, 0.0),
      pointer_serial: None,
      fingers: Fingers::default(),
      typing: None,
      text_input,
      keyboard_events: Vec::new(),
      focus: KeyboardFocus::Elsewhere,
    };

    Ok(Surface {
      connection,
      queue,
      bound,
    })
  }

  pub fn show(&mut self, wanted: &Wanted) -> Result<(), SurfaceError> {
    match self.bound.up {
      Some(_) => return Ok(()),
      None => {}
    }

    let shell = self
      .bound
      .shell
      .as_ref()
      .ok_or(SurfaceError::Global("zwlr_layer_shell_v1"))?;

    let hand = self.queue.handle();
    let surface = self.bound.compositor.create_surface(&hand, ());
    let viewport = self.bound.viewporter.get_viewport(&surface, &hand, ());
    let fraction = self
      .bound
      .fractions
      .as_ref()
      .map(|manager| manager.get_fractional_scale(&surface, &hand, ()));
    let Ok(among) = among(wanted.under);
    let layer =
      shell.get_layer_surface(&surface, None, among, wanted.namespace.clone(), &hand, ());
    let Ok(held) = exclusive_zone(wanted.anchor, wanted.size);
    let Ok(zone) = reserved_size(wanted.room, wanted.anchor, wanted.size);

    layer.set_size(held.width, held.height);
    layer.set_exclusive_zone(zone);
    let Ok(edges) = edges(wanted.anchor);

    layer.set_anchor(edges);
    layer.set_margin(
      wanted.margin.top,
      wanted.margin.right,
      wanted.margin.bottom,
      wanted.margin.left,
    );
    layer.set_keyboard_interactivity(match wanted.keyboard {
      Keyboard::Takes => zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive,
      Keyboard::OnDemand => zwlr_layer_surface_v1::KeyboardInteractivity::OnDemand,
      Keyboard::Declines => zwlr_layer_surface_v1::KeyboardInteractivity::None,
    });
    surface.commit();

    self.bound.logical = None;
    self.bound.asked = Some(wanted.size);
    self.bound.anchor = wanted.anchor;
    self.bound.room = wanted.room;
    self.bound.up = Some(Up {
      surface,
      role: Role::Layer(layer),
      viewport,
      fraction,
    });
    let Ok(timing) = Frames::shown(&wanted.namespace);

    self.bound.timing = Some(timing);

    self.dispatched_until(|bound| match (bound.logical, bound.closed) {
      (None, Closed::No) => Ready::No,
      (Some(_), _) | (None, Closed::Yes) => Ready::Yes,
    })
  }

  pub fn show_window(&mut self, window: &Window) -> Result<(), SurfaceError> {
    self.show_window_sized(
      window,
      Size {
        width: 1,
        height: 1,
      },
    )
  }

  pub fn show_window_sized(
    &mut self,
    window: &Window,
    size: Size<u32>,
  ) -> Result<(), SurfaceError> {
    match self.bound.up {
      Some(_) => return Ok(()),
      None => {}
    }

    let windows = match self.bound.windows.as_ref() {
      Some(windows) => windows,
      None => {
        return Err(SurfaceError::Global(
          "xdg_wm_base, which is what makes a surface a window",
        ));
      }
    };
    let hand = self.queue.handle();
    let surface = self.bound.compositor.create_surface(&hand, ());
    let viewport = self.bound.viewporter.get_viewport(&surface, &hand, ());
    let fraction = self
      .bound
      .fractions
      .as_ref()
      .map(|manager| manager.get_fractional_scale(&surface, &hand, ()));
    let framed = windows.get_xdg_surface(&surface, &hand, ());
    let toplevel = framed.get_toplevel(&hand, ());

    toplevel.set_app_id(window.app_id.clone());
    toplevel.set_title(window.title.clone());
    surface.commit();

    self.bound.logical = None;
    self.bound.asked = Some(size);
    self.bound.given = Size {
      width: 0,
      height: 0,
    };
    self.bound.up = Some(Up {
      surface,
      role: Role::Window(framed, toplevel),
      viewport,
      fraction,
    });
    let Ok(timing) = Frames::shown(&window.app_id);

    self.bound.timing = Some(timing);

    self.dispatched_until(|bound| match (bound.logical, bound.closed) {
      (None, Closed::No) => Ready::No,
      (Some(_), _) | (None, Closed::Yes) => Ready::Yes,
    })
  }

  #[cfg_attr(
    dylint_lib = "explicit055_no_unbounded_loop",
    expect(
      explicit055_no_unbounded_loop,
      reason = "the compositor completes the role's handshake with a configure or close event, and dispatch returns a connection failure if the peer goes away"
    )
  )]
  fn dispatched_until(&mut self, settled: impl Fn(&Bound) -> Ready) -> Result<(), SurfaceError> {
    let dispatched = std::iter::repeat(()).try_fold(self, |this, ()| {
      match settled(&this.bound) {
        Ready::Yes => return ControlFlow::Break(Ok(())),
        Ready::No => {}
      }

      match this.queue.blocking_dispatch(&mut this.bound) {
        Ok(_dispatched) => ControlFlow::Continue(this),
        Err(fault) => ControlFlow::Break(Err(SurfaceError::Went(fault))),
      }
    });

    match dispatched {
      ControlFlow::Break(settled_2) => settled_2,
      ControlFlow::Continue(_endless) => Ok(()),
    }
  }

  pub fn lock(&mut self, namespace: &str) -> Result<Lock, SurfaceError> {
    match self.bound.up {
      Some(_) => return Ok(self.bound.lock),
      None => {}
    }

    let locker = match self.bound.locker.as_ref() {
      Some(locker) => locker,
      None => {
        return Err(SurfaceError::Global(
          "ext_session_lock_manager_v1, which is what makes a lock screen a lock",
        ));
      }
    };
    let output = match self.bound.output.as_ref() {
      Some(output) => output,
      None => {
        return Err(SurfaceError::Global(
          "wl_output, which is the screen a lock is shown on",
        ));
      }
    };
    let hand = self.queue.handle();
    let session = locker.lock(&hand, ());
    let surface = self.bound.compositor.create_surface(&hand, ());
    let viewport = self.bound.viewporter.get_viewport(&surface, &hand, ());
    let fraction = self
      .bound
      .fractions
      .as_ref()
      .map(|manager| manager.get_fractional_scale(&surface, &hand, ()));
    let role = session.get_lock_surface(&surface, output, &hand, ());

    self.bound.logical = None;
    self.bound.asked = None;
    self.bound.lock = Lock::Waiting;
    self.bound.session = Some(session);
    self.bound.up = Some(Up {
      surface,
      role: Role::Lock(role),
      viewport,
      fraction,
    });
    let Ok(timing) = Frames::shown(namespace);

    self.bound.timing = Some(timing);

    self.dispatched_until(|bound| match (bound.logical, bound.lock) {
      (None, Lock::Waiting) => Ready::No,
      (Some(_), _) | (None, Lock::Acquired | Lock::Denied) => Ready::Yes,
    })?;

    Ok(self.bound.lock)
  }

  pub fn locked(&self) -> Result<Lock, Never> {
    Ok(self.bound.lock)
  }

  pub fn unlock(&mut self) -> Result<Unlocked, SurfaceError> {
    match self.bound.lock {
      Lock::Acquired => {}
      Lock::Waiting | Lock::Denied => return Ok(Unlocked::NotYet),
    }

    let session = match self.bound.session.take() {
      Some(session) => session,
      None => return Ok(Unlocked::NotYet),
    };

    session.unlock_and_destroy();
    let Ok(()) = self.hide();

    self.bound.lock = Lock::Waiting;
    self
      .queue
      .roundtrip(&mut self.bound)
      .map_err(SurfaceError::Went)?;

    Ok(Unlocked::Yes)
  }

  pub fn resize(&mut self, size: Size<u32>) -> Result<(), Never> {
    let up = match self.bound.up.as_ref() {
      Some(up) => up,
      None => return Ok(()),
    };

    match self.bound.asked {
      Some(asked) => match ::core::primitive::bool::then_some(asked == size, ()) {
        ::core::option::Option::Some(()) => return Ok(()),
        ::core::option::Option::None => {}
      },
      None => {}
    }

    let layer = match &up.role {
      Role::Layer(layer) => layer,
      Role::Lock(_) | Role::Window(_, _) => return Ok(()),
    };
    let Ok(held) = exclusive_zone(self.bound.anchor, size);

    layer.set_size(held.width, held.height);
    up.surface.commit();
    let _closed_if_the_flush_failed = self.flush();

    self.bound.asked = Some(size);

    Ok(())
  }

  pub fn room(&mut self, room: Room) -> Result<(), Never> {
    match ::core::primitive::bool::then_some(self.bound.room == room, ()) {
      ::core::option::Option::Some(()) => return Ok(()),
      ::core::option::Option::None => {}
    }

    self.bound.room = room;

    let up = match self.bound.up.as_ref() {
      Some(up) => up,
      None => return Ok(()),
    };
    let asked = match self.bound.logical {
      Some(logical) => logical,
      None => Size {
        width: 1,
        height: 1,
      },
    };
    let layer = match &up.role {
      Role::Layer(layer) => layer,
      Role::Lock(_) | Role::Window(_, _) => return Ok(()),
    };
    let Ok(zone) = reserved_size(room, self.bound.anchor, asked);
    let anything = Size {
      width: 0,
      height: 0,
    };

    layer.set_exclusive_zone(zone);
    layer.set_size(anything.width, anything.height);
    up.surface.commit();
    self.bound.asked = Some(anything);
    self.bound.logical = None;
    let _closed_if_the_flush_failed = self.flush();

    Ok(())
  }

  pub fn hide(&mut self) -> Result<(), Never> {
    let Ok(()) = self.bound.cancel_drawing();
    let Ok(()) = self.clear_layer();

    match self.bound.text_input.as_mut() {
      Some(input) => {
        let Ok(()) = input.focus(TextFocus::Left);
      }
      None => {}
    }

    let up = match self.bound.up.take() {
      Some(up) => up,
      None => return Ok(()),
    };

    match up.fraction {
      Some(fraction) => fraction.destroy(),
      None => {}
    }

    up.viewport.destroy();

    match up.role {
      Role::Layer(layer) => layer.destroy(),
      Role::Lock(lock) => lock.destroy(),
      Role::Window(framed, toplevel) => {
        toplevel.destroy();
        framed.destroy();
      }
    }

    up.surface.destroy();
    self.bound.focus = KeyboardFocus::Elsewhere;
    self.bound.keyboard_events.clear();
    self.bound.shown = None;
    self.bound.spare = Vec::new();

    match self.bound.timing.take() {
      Some(timing) => {
        let Ok(()) = timing.finish();
      }
      None => {}
    }

    self.bound.logical = None;
    self.bound.asked = None;
    let _closed_if_the_flush_failed = self.flush();

    Ok(())
  }

  pub fn logical(&self) -> Result<Option<Size<u32>>, Never> {
    Ok(self.bound.logical)
  }

  pub fn scale(&self) -> Result<Scale, Never> {
    Ok(self.bound.scale)
  }

  pub fn closed(&self) -> Result<Closed, Never> {
    Ok(self.bound.closed)
  }

  pub fn keyboard_events(&mut self) -> Result<Vec<KeyboardEvent>, Never> {
    let Ok(presses) = self.key_presses();
    Ok(
      presses
        .into_iter()
        .map(|press| KeyboardEvent::Down { key: press.key })
        .collect(),
    )
  }

  pub fn set_text_input(&mut self, state: Option<TextInputState>) -> Result<(), Never> {
    match self.bound.text_input.as_mut() {
      Some(input) => input.set(state),
      None => Ok(()),
    }
  }

  pub fn text_input_changes(&mut self) -> Result<Vec<TextInputChange>, Never> {
    let changes = match self.bound.text_input.as_mut() {
      Some(input) => std::mem::take(&mut input.received),
      None => Vec::new(),
    };
    if !changes.is_empty() {
      self.handled_input();
    }
    Ok(changes)
  }

  pub fn key_presses(&mut self) -> Result<Vec<KeyPress>, Never> {
    let presses = std::mem::take(&mut self.bound.keyboard_events);
    if !presses.is_empty() {
      self.handled_input();
    }
    Ok(presses)
  }

  pub fn set_window_title(&self, title: &str) -> Result<(), Never> {
    match self.bound.up.as_ref().map(|up| &up.role) {
      Some(Role::Window(_, toplevel)) => toplevel.set_title(title.to_owned()),
      Some(Role::Layer(_) | Role::Lock(_)) | None => {}
    }

    Ok(())
  }

  pub fn move_window(&mut self) -> Result<(), Never> {
    match (
      self.bound.up.as_ref().map(|up| &up.role),
      &self.bound.seat,
      self.bound.pointer_serial.take(),
    ) {
      (Some(Role::Window(_, toplevel)), Some(seat), Some(serial)) => toplevel._move(seat, serial),
      (None | Some(Role::Layer(_) | Role::Lock(_)), _, _)
      | (Some(Role::Window(_, _)), None, _)
      | (Some(Role::Window(_, _)), Some(_), None) => {}
    }

    Ok(())
  }

  pub fn keyboard_focus(&self) -> Result<KeyboardFocus, Never> {
    Ok(self.bound.focus)
  }

  pub fn pointer_events(&mut self) -> Result<Vec<PointerEvent>, Never> {
    let events = std::mem::take(&mut self.bound.pointer_events);
    if events.iter().any(|event| {
      matches!(
        event,
        PointerEvent::Down { .. }
          | PointerEvent::Up
          | PointerEvent::Scrolled { .. }
          | PointerEvent::Pinched { .. }
      )
    }) {
      self.handled_input();
    }
    Ok(events)
  }

  fn handled_input(&mut self) {
    if let Some(timing) = self.bound.timing.as_mut() {
      let Ok(()) = timing.handled_input();
    }
  }

  pub fn draw(
    &mut self,
    paint: impl FnOnce(&mut [u8], Size<u32>, Scale) -> Result<(), Never>,
  ) -> Result<(), SurfaceError> {
    self.paint(paint)
  }

  #[must_use]
  pub fn layer_support(&self) -> Result<LayerSupport, Never> {
    Ok(match self.bound.subcompositor.as_ref() {
      Some(_) => LayerSupport::Available,
      None => LayerSupport::Unavailable,
    })
  }

  pub fn clear_layer(&mut self) -> Result<(), Never> {
    match self.bound.region.take() {
      Some(region) => {
        drop(region);

        match self.flush() {
          Ok(_sent_or_pending) => {}
          Err(fault) => {
            console_core_logging::warn!("could not clear a Wayland surface layer: {fault}")
          }
        }
      }
      None => {}
    }

    Ok(())
  }

  pub fn draw_layer(
    &mut self,
    part: Part,
    paint: impl FnOnce(&mut [u8], Size<u32>, Scale) -> Result<(), Never>,
  ) -> Result<Visible, SurfaceError> {
    let (manager, up, logical) = match (
      self.bound.subcompositor.as_ref(),
      self.bound.up.as_ref(),
      self.bound.logical,
    ) {
      (Some(manager), Some(up), Some(logical)) => (manager, up, logical),
      (None, _, _) | (_, None, _) | (_, _, None) => return Ok(Visible::NotYet),
    };
    let Ok(fit) = region::fit(logical, part);

    match fit {
      Fit::Inside => {}
      Fit::Outside => return Ok(Visible::NotYet),
    }

    let hand = self.queue.handle();
    let scale = self.bound.scale;
    let Ok(device) = scale.device(part.size);
    let Ok(painting) = start_painting(self.bound.timing.as_ref());
    let parent = up.surface.clone();
    let layer = self.bound.region.get_or_insert_with(|| {
      let surface = self.bound.compositor.create_surface(&hand, ());
      let input = self.bound.compositor.create_region(&hand, ());
      surface.set_input_region(Some(&input));
      input.destroy();
      let role = manager.get_subsurface(&surface, &parent, &hand, ());
      let viewport = self.bound.viewporter.get_viewport(&surface, &hand, ());

      RegionLayer {
        surface,
        role,
        viewport,
        at: None,
        shown: None,
        spare: Vec::new(),
      }
    });
    let mut frame = free(&mut layer.spare, &self.bound.shm, &hand, device)?;
    let Ok(pixels) = frame._shared.pixels();
    let Ok(()) = paint(pixels, device, scale);
    let Ok(wide) = fitted::<u32, i32>(device.width);
    let Ok(tall) = fitted::<u32, i32>(device.height);
    let Ok(across) = fitted::<u32, i32>(part.size.width);
    let Ok(down) = fitted::<u32, i32>(part.size.height);
    let positioned = layer.at != Some(part.at);

    match ::core::primitive::bool::then_some(positioned, ()) {
      ::core::option::Option::Some(()) => {
        layer.role.set_sync();
        layer.role.set_position(part.at.x, part.at.y);
        layer.at = Some(part.at);
      }
      ::core::option::Option::None => {}
    }

    layer.surface.attach(Some(&frame.buffer), 0, 0);
    layer.viewport.set_destination(across, down);
    layer.surface.damage_buffer(0, 0, wide, tall);
    let held = std::mem::replace(&mut frame.held, Buffer::Acquired);
    layer.spare.extend(layer.shown.replace(frame));
    let child = layer.surface.clone();
    let role = layer.role.clone();
    let Ok(()) = record_frame(Timing {
      bound: &mut self.bound,
      painting,
      held,
      hand: &hand,
      region: Some(&child),
    });
    child.commit();

    match ::core::primitive::bool::then_some(positioned, ()) {
      ::core::option::Option::Some(()) => {
        parent.commit();
        role.set_desync();
      }
      ::core::option::Option::None => {}
    }

    let _flushed = self.flush()?;

    Ok(Visible::Yes)
  }

  pub fn draw_notifying(
    &mut self,
    finished: DrawingSender,
    paint: impl FnOnce(&mut [u8], Size<u32>, Scale) -> Result<(), Never>,
  ) -> Result<(), SurfaceError> {
    match (self.bound.logical, self.bound.up.as_ref()) {
      (Some(_), Some(_)) => {}
      (None, _) | (_, None) => {
        return Err(SurfaceError::Global(
          "a configured surface for drawing completion",
        ));
      }
    }

    let Ok(()) = self.bound.cancel_drawing();

    let id = match self.bound.drawing_id.checked_add(1) {
      Some(id) => id,
      None => {
        let Ok(()) = finished.notify(Drawing::Unavailable);

        return self.paint(paint);
      }
    };
    self.bound.drawing_id = id;
    let Ok(drawing) = PendingDrawing::new(DrawingId(id), finished);
    self.bound.drawing = Some(drawing);

    match self.paint(paint) {
      Ok(()) => Ok(()),
      Err(fault) => {
        self.bound.drawing = None;

        Err(fault)
      }
    }
  }

  fn paint(
    &mut self,
    paint: impl FnOnce(&mut [u8], Size<u32>, Scale) -> Result<(), Never>,
  ) -> Result<(), SurfaceError> {
    let logical = match self.bound.logical {
      Some(logical) => logical,
      None => return Ok(()),
    };

    let up = match self.bound.up.as_ref() {
      Some(up) => up,
      None => return Ok(()),
    };

    let hand = self.queue.handle();
    let scale = self.bound.scale;
    let Ok(device) = scale.device(logical);

    let mut frame = free(&mut self.bound.spare, &self.bound.shm, &hand, device)?;
    let Ok(painting) = start_painting(self.bound.timing.as_ref());
    let Ok(pixels) = frame._shared.pixels();

    let Ok(()) = paint(pixels, device, scale);

    let Ok(wide) = fitted::<u32, i32>(device.width);
    let Ok(tall) = fitted::<u32, i32>(device.height);
    let Ok(across) = fitted::<u32, i32>(logical.width);
    let Ok(down) = fitted::<u32, i32>(logical.height);

    up.surface.attach(Some(&frame.buffer), 0, 0);
    up.surface.set_buffer_scale(1);
    up.viewport.set_destination(across, down);
    up.surface.damage_buffer(0, 0, wide, tall);
    let held = std::mem::replace(&mut frame.held, Buffer::Acquired);
    let Ok(()) = self.bound.present(frame);
    let Ok(()) = record_frame(Timing {
      bound: &mut self.bound,
      painting,
      held,
      hand: &hand,
      region: None,
    });
    let up_3 = match self.bound.up.as_ref() {
      Some(up_2) => up_2,
      None => return Ok(()),
    };

    up_3.surface.commit();
    let _flushed = self.flush()?;

    Ok(())
  }

  pub fn draw_over(
    &mut self,
    part: Part,
    paint: impl FnOnce(&mut [u8], Size<u32>, Scale) -> Result<(), Never>,
  ) -> Result<Visible, SurfaceError> {
    let logical = match self.bound.logical {
      Some(logical) => logical,
      None => return Ok(Visible::NotYet),
    };

    let up = match self.bound.up.as_ref() {
      Some(up) => up,
      None => return Ok(Visible::NotYet),
    };

    let scale = self.bound.scale;
    let Ok(device) = scale.device(logical);

    let hand = self.queue.handle();
    let under = match self.bound.shown.as_mut() {
      Some(shown) => match ::core::primitive::bool::then_some(shown.size == device, ()) {
        ::core::option::Option::Some(()) => shown,
        ::core::option::Option::None => return Ok(Visible::NotYet),
      },
      None => return Ok(Visible::NotYet),
    };
    let Ok(painting) = start_painting(self.bound.timing.as_ref());
    let Ok(before) = under._shared.pixels();
    let mut frame = free(&mut self.bound.spare, &self.bound.shm, &hand, device)?;
    let Ok(pixels) = frame._shared.pixels();

    match ::core::primitive::bool::then_some(pixels.len() == before.len(), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        return Err(SurfaceError::Memory(io::Error::new(
          io::ErrorKind::InvalidData,
          "the previous frame and its replacement have different byte lengths",
        )));
      }
    }

    use std::io::Write;

    let mut destination = &mut *pixels;
    destination
      .write_all(before)
      .map_err(SurfaceError::Memory)?;

    let Ok(()) = paint(pixels, device, scale);

    let Ok(across) = fitted::<u32, i32>(logical.width);
    let Ok(down) = fitted::<u32, i32>(logical.height);
    let Ok(damaged) = on_the_device(part, logical, device);

    up.surface.attach(Some(&frame.buffer), 0, 0);
    up.surface.set_buffer_scale(1);
    up.viewport.set_destination(across, down);
    up.surface.damage_buffer(
      damaged.at.x,
      damaged.at.y,
      damaged.size.width,
      damaged.size.height,
    );
    let held = std::mem::replace(&mut frame.held, Buffer::Acquired);
    let Ok(()) = self.bound.present(frame);
    let Ok(()) = record_frame(Timing {
      bound: &mut self.bound,
      painting,
      held,
      hand: &hand,
      region: None,
    });
    let up_3 = match self.bound.up.as_ref() {
      Some(up_2) => up_2,
      None => return Ok(Visible::NotYet),
    };

    up_3.surface.commit();
    let _flushed = self.flush()?;

    Ok(Visible::Yes)
  }

  fn flush(&mut self) -> Result<Flushed, SurfaceError> {
    let fault = match self.connection.flush() {
      Ok(()) => return Ok(Flushed::Sent),
      Err(WaylandError::Protocol(fault)) => WaylandError::Protocol(fault),
      Err(WaylandError::Io(fault)) => match ::core::primitive::bool::then_some(fault.kind() == io::ErrorKind::WouldBlock, ()) {
        ::core::option::Option::Some(()) => return Ok(Flushed::Pending),
        ::core::option::Option::None => WaylandError::Io(fault),
      },
    };

    self.bound.closed = Closed::Yes;

    Err(SurfaceError::Went(wayland_client::DispatchError::Backend(
      fault,
    )))
  }

  pub fn wait(
    &mut self,
    also: &[BorrowedFd<'_>],
    until: Option<Duration>,
  ) -> Result<(), SurfaceError> {
    if let Some(timing) = self.bound.timing.as_mut() {
      let Ok(()) = timing.end_input_turn();
    }
    let dispatched = self
      .queue
      .dispatch_pending(&mut self.bound)
      .map_err(SurfaceError::Went)?;
    let flushed = self.flush()?;
    if dispatched != 0 {
      return Ok(());
    }

    let guard = match self.connection.prepare_read() {
      Some(guard) => guard,
      None => {
        self
          .queue
          .dispatch_pending(&mut self.bound)
          .map_err(SurfaceError::Went)?;

        return Ok(());
      }
    };

    let socket = self.connection.as_fd();
    let interest = match flushed {
      Flushed::Sent => PollFlags::IN,
      Flushed::Pending => PollFlags::IN | PollFlags::OUT,
    };
    let mut watch = vec![PollFd::from_borrowed_fd(socket, interest)];

    watch.extend(
      also
        .iter()
        .map(|fd| PollFd::from_borrowed_fd(*fd, PollFlags::IN)),
    );

    let wait = match until {
      None => None,
      Some(long) => {
        let Ok(seconds) = fitted::<u64, Secs>(long.as_secs());
        let Ok(nanoseconds) = fitted::<u32, Nsecs>(long.subsec_nanos());

        Some(Timespec {
          tv_sec: seconds,
          tv_nsec: nanoseconds,
        })
      }
    };

    match poll(&mut watch, wait.as_ref()) {
      Ok(_) => {}
      Err(_the_poll_failed) => {
        drop(guard);

        return Ok(());
      }
    }

    let hung = watch
      .first()
      .is_some_and(|first| first.revents().intersects(PollFlags::HUP | PollFlags::ERR));

    match ::core::primitive::bool::then_some(hung, ()) {
      ::core::option::Option::Some(()) => {
        drop(guard);
        self.bound.closed = Closed::Yes;

        return Err(SurfaceError::Hung);
      }
      ::core::option::Option::None => {}
    }

    let readiness = match watch.first() {
      Some(first) => first.revents(),
      None => PollFlags::empty(),
    };
    let readable = readiness.contains(PollFlags::IN);

    match ::core::primitive::bool::then_some(readable, ()) {
      ::core::option::Option::Some(()) => {
        let carried_on = match guard.read() {
          Ok(_) => true,
          Err(wayland_client::backend::WaylandError::Io(why)) => {
            why.kind() == io::ErrorKind::WouldBlock
          }
          Err(_the_connection_failed) => false,
        };

        match ::core::primitive::bool::then_some(carried_on, ()) {
          ::core::option::Option::Some(()) => {}
          ::core::option::Option::None => {
            self.bound.closed = Closed::Yes;

            return Err(SurfaceError::Hung);
          }
        }
      }
      ::core::option::Option::None => drop(guard),
    }

    match ::core::primitive::bool::then_some(readiness.contains(PollFlags::OUT), ()) {
      ::core::option::Option::Some(()) => {
        let _retry = self.flush()?;
      }
      ::core::option::Option::None => {}
    }

    self
      .queue
      .dispatch_pending(&mut self.bound)
      .map_err(SurfaceError::Went)?;

    Ok(())
  }
}

impl Drop for Surface {
  fn drop(&mut self) {
    let Ok(()) = self.hide();
  }
}

fn start_painting(
  timing: Option<&Frames>,
) -> Result<Option<console_response_times::frames::Painting>, Never> {
  Ok(match timing {
    Some(timing_2) => {
      let Ok(painting) = timing_2.painting();

      Some(painting)
    }
    None => None,
  })
}

struct Timing<'a> {
  bound: &'a mut Bound,
  painting: Option<console_response_times::frames::Painting>,
  held: Buffer,
  hand: &'a QueueHandle<Bound>,
  region: Option<&'a wl_surface::WlSurface>,
}

fn record_frame(timing: Timing<'_>) -> Result<(), Never> {
  let Timing {
    bound,
    painting,
    held,
    hand,
    region,
  } = timing;

  match (
    bound.drawing.as_ref().filter(|_| region.is_none()),
    bound.presentation.as_ref(),
    bound.up.as_ref(),
  ) {
    (Some(notice), Some(presentation), Some(up)) => {
      let _feedback = presentation.feedback(&up.surface, hand, notice.id);
    }
    (Some(notice), None, _) | (Some(notice), _, None) => {
      let Ok(()) = notice.notice.notify(Drawing::Unavailable);

      bound.drawing = None;
    }
    (None, _, _) => {}
  }

  let (tally, painting_3) = match (bound.timing.as_mut(), painting) {
    (Some(tally), Some(painting_2)) => (tally, painting_2),
    (None, _) | (_, None) => return Ok(()),
  };

  let Ok(committed) = tally.committed(painting_3, held);

  let surface = region.or_else(|| bound.up.as_ref().map(|up| &up.surface));

  match (committed, bound.presentation.as_ref(), surface) {
    (Some(committed_2), Some(presentation), Some(surface_2)) => {
      let _ = presentation.feedback(surface_2, hand, committed_2);
    }
    (None, _, _) | (_, None, _) | (_, _, None) => {}
  }

  Ok(())
}

fn reserved_size(room: Room, anchor: Anchor, logical: Size<u32>) -> Result<i32, Never> {
  let along = match anchor {
    Anchor::Top | Anchor::Bottom => logical.height,
    Anchor::Left | Anchor::Right | Anchor::TopRight | Anchor::Whole => logical.width,
  };
  let Ok(many) = fitted::<u32, i32>(along);

  Ok(match room {
    Room::Reserves => many,
    Room::Over => -1,
    Room::Around => 0,
  })
}

fn exclusive_zone(anchor: Anchor, size: Size<u32>) -> Result<Size<u32>, Never> {
  Ok(match anchor {
    Anchor::Whole => Size {
      width: 0,
      height: 0,
    },
    Anchor::Top | Anchor::Bottom => Size {
      width: 0,
      height: size.height,
    },
    Anchor::Left | Anchor::Right => Size {
      width: size.width,
      height: 0,
    },
    Anchor::TopRight => size,
  })
}

fn among(under: Under) -> Result<zwlr_layer_shell_v1::Layer, Never> {
  Ok(match under {
    Under::AWindow => zwlr_layer_shell_v1::Layer::Bottom,
    Under::Anything => zwlr_layer_shell_v1::Layer::Top,
    Under::None => zwlr_layer_shell_v1::Layer::Overlay,
  })
}

fn edges(anchor: Anchor) -> Result<zwlr_layer_surface_v1::Anchor, Never> {
  Ok(match anchor {
    Anchor::Top => {
      zwlr_layer_surface_v1::Anchor::Top
        | zwlr_layer_surface_v1::Anchor::Left
        | zwlr_layer_surface_v1::Anchor::Right
    }
    Anchor::TopRight => zwlr_layer_surface_v1::Anchor::Top | zwlr_layer_surface_v1::Anchor::Right,
    Anchor::Bottom => {
      zwlr_layer_surface_v1::Anchor::Bottom
        | zwlr_layer_surface_v1::Anchor::Left
        | zwlr_layer_surface_v1::Anchor::Right
    }
    Anchor::Left => {
      zwlr_layer_surface_v1::Anchor::Left
        | zwlr_layer_surface_v1::Anchor::Top
        | zwlr_layer_surface_v1::Anchor::Bottom
    }
    Anchor::Right => {
      zwlr_layer_surface_v1::Anchor::Right
        | zwlr_layer_surface_v1::Anchor::Top
        | zwlr_layer_surface_v1::Anchor::Bottom
    }
    Anchor::Whole => {
      zwlr_layer_surface_v1::Anchor::Top
        | zwlr_layer_surface_v1::Anchor::Bottom
        | zwlr_layer_surface_v1::Anchor::Left
        | zwlr_layer_surface_v1::Anchor::Right
    }
  })
}

#[cfg(test)]
mod tests {
  use super::{Damage, LayerSupport, Part, Surface, SurfaceError, Visible, Window};
  use console_core_never::Never;
  use console_core_number_conversion::indices;
  use std::time::{Duration, Instant};

  type TestFailure = Box<dyn std::error::Error>;
  const FRAME_GAP: Duration = Duration::from_millis(24);

  #[test]
  #[ignore = "requires a live Wayland compositor"]
  fn region_presentation() -> Result<(), TestFailure> {
    let mut surface = Surface::connect()?;
    surface.show_window_sized(
      &Window {
        app_id: "org.console.RegionCheck".to_string(),
        title: "Console region check".to_string(),
      },
      Size {
        width: 800,
        height: 600,
      },
    )?;
    assert_eq!(surface.layer_support(), Ok(LayerSupport::Available));
    surface.draw(|pixels, _, _| {
      pixels.fill(255);
      Ok(())
    })?;
    let Ok(configured) = surface.logical();
    let logical = configured.ok_or("missing configured dimensions")?;
    let part = Part { at: Point { x: 16, y: 20 }, size: Size { width: 320, height: 240 } };
    let base = measure_redraws(&mut surface, |target, shade| {
      target.draw_over(part, |pixels, device, _| {
        let Ok(damage) = super::on_the_device(part, logical, device);
        shade_damage(pixels, device, damage, shade)
      })
    })?;
    let baseline_late_frames = late_frames(&surface)?;

    let child = measure_redraws(&mut surface, |target, shade| {
      target.draw_layer(part, |pixels, _, _| shade_pixels(pixels, shade))
    })?;
    let total_late_frames = late_frames(&surface)?;
    let child_late_frames = total_late_frames.saturating_sub(baseline_late_frames);
    let layer = surface
      .bound
      .region
      .as_ref()
      .ok_or("missing child surface")?;
    assert!(
      layer.spare.len() < 12,
      "the compositor never released a child buffer"
    );
    let Ok(scale) = surface.scale();
    let Ok(main) = scale.device(logical);
    let bytes = u64::from(main.width)
      .saturating_mul(u64::from(main.height))
      .saturating_mul(4);
    println!(
      "partial redraw: {bytes} preserved bytes copied per frame, median {:?}, {baseline_late_frames} late frames",
      base.get(6)
    );
    println!(
      "child surface: 0 preserved bytes copied per frame, median {:?}, {child_late_frames} late frames",
      child.get(6)
    );
    let Ok(()) = surface.hide();

    Ok(())
  }

  fn measure_redraws(
    surface: &mut Surface,
    mut draw: impl FnMut(&mut Surface, u8) -> Result<Visible, SurfaceError>,
  ) -> Result<Vec<Duration>, SurfaceError> {
    let mut times = Vec::new();

    for shade in 0..12 {
      let started = Instant::now();
      let shown = draw(surface, shade)?;
      assert_eq!(shown, Visible::Yes);
      times.push(started.elapsed());
      surface.wait(&[], Some(FRAME_GAP))?;
    }

    for _ in 0..3 {
      surface.wait(&[], Some(FRAME_GAP))?;
    }

    times.sort_unstable();
    Ok(times)
  }

  fn late_frames(surface: &Surface) -> Result<u64, TestFailure> {
    let timing = surface
      .bound
      .timing
      .as_ref()
      .ok_or("missing frame timing")?;
    let Ok(count) = timing.late_frames();
    Ok(count)
  }

  fn shade_damage(
    pixels: &mut [u8],
    device: Size<u32>,
    damage: Damage,
    shade: u8,
  ) -> Result<(), Never> {
    use console_core_number_conversion::fitted;

    let stride = u64::from(device.width).saturating_mul(4);
    let Ok(x) = fitted::<_, u32>(damage.at.x.max(0));
    let Ok(y) = fitted::<_, u32>(damage.at.y.max(0));
    let Ok(wide) = fitted::<_, u32>(damage.size.width.max(0));
    let Ok(tall) = fitted::<_, u32>(damage.size.height.max(0));
    let left = u64::from(x).saturating_mul(4);
    let right = left.saturating_add(u64::from(wide).saturating_mul(4));

    match (right <= stride).then_some(()) {
      Some(()) => {}
      None => return Ok(()),
    }

    for scanline in y..y.saturating_add(tall).min(device.height) {
      let row = u64::from(scanline).saturating_mul(stride);
      let Ok(columns) = indices(row.saturating_add(left)..row.saturating_add(right));

      match pixels.get_mut(columns) {
        Some(part) => {
          let Ok(()) = shade_pixels(part, shade);
        }
        None => {}
      }
    }

    Ok(())
  }

  fn shade_pixels(pixels: &mut [u8], shade: u8) -> Result<(), Never> {
    let (colors, _tail) = pixels.as_chunks_mut::<4>();

    for pixel in colors {
      *pixel = [shade, 32, 128, 255];
    }

    Ok(())
  }

  #[test]
  fn a_part_redrawn_is_damaged_where_it_lands_on_the_glass_and_nowhere_else() {
    let part = super::Part {
      at: Point { x: 10, y: 20 },
      size: Size {
        width: 100,
        height: 50,
      },
    };
    let logical = Size {
      width: 1000,
      height: 600,
    };

    assert_eq!(
      super::on_the_device(
        part,
        logical,
        Size {
          width: 1500,
          height: 900
        }
      ),
      Ok(super::Damage {
        at: Point { x: 15, y: 30 },
        size: Size {
          width: 150,
          height: 75
        }
      }),
    );
    assert_eq!(
      super::on_the_device(part, logical, logical),
      Ok(super::Damage {
        at: Point { x: 10, y: 20 },
        size: Size {
          width: 100,
          height: 50
        }
      }),
    );
  }

  #[test]
  fn a_part_on_a_fractional_scale_is_damaged_out_to_the_whole_pixels_it_touches() {
    let part = super::Part {
      at: Point { x: 1, y: 1 },
      size: Size {
        width: 1,
        height: 1,
      },
    };
    let damaged = super::on_the_device(
      part,
      Size {
        width: 4,
        height: 4,
      },
      Size {
        width: 5,
        height: 5,
      },
    );

    assert_eq!(
      damaged,
      Ok(super::Damage {
        at: Point { x: 1, y: 1 },
        size: Size {
          width: 2,
          height: 2
        }
      })
    );
  }

  use super::*;

  #[test]
  fn a_discarded_frame_does_not_finish_the_drawing() {
    let Ok((sender, received)) = drawing_notifications();
    let notice = DrawingId(1);
    let Ok(drawing) = PendingDrawing::new(notice, sender);
    let mut pending = Some(drawing);

    let Ok(()) = finish_drawing(&mut pending, notice, Drawing::Discarded);

    assert_eq!(received.try_recv(), Ok(Drawing::Discarded));
    assert!(pending.is_some());

    let Ok(()) = finish_drawing(&mut pending, notice, Drawing::Discarded);

    assert_eq!(
      received.try_recv(),
      Err(std::sync::mpsc::TryRecvError::Empty)
    );

    let Ok(()) = finish_drawing(&mut pending, notice, Drawing::Presented);

    assert_eq!(received.try_recv(), Ok(Drawing::Presented));
    assert!(pending.is_none());

    let Ok(()) = finish_drawing(&mut pending, notice, Drawing::Presented);

    assert_eq!(
      received.try_recv(),
      Err(std::sync::mpsc::TryRecvError::Disconnected)
    );
  }

  #[test]
  fn a_late_frame_cannot_complete_the_request_that_replaced_it() {
    let Ok((old_sender, old_received)) = drawing_notifications();
    let old = DrawingId(1);
    let Ok(drawing) = PendingDrawing::new(old, old_sender);
    let mut pending = Some(drawing);

    let Ok(()) = cancel_drawing(&mut pending);

    assert_eq!(old_received.try_recv(), Ok(Drawing::Cancelled));

    let Ok((sender, received)) = drawing_notifications();
    let current = DrawingId(2);
    let Ok(replacement) = PendingDrawing::new(current, sender);

    pending = Some(replacement);

    let Ok(()) = finish_drawing(&mut pending, old, Drawing::Presented);

    assert_eq!(
      received.try_recv(),
      Err(std::sync::mpsc::TryRecvError::Empty)
    );
    assert_eq!(
      old_received.try_recv(),
      Err(std::sync::mpsc::TryRecvError::Disconnected)
    );

    let Ok(()) = finish_drawing(&mut pending, current, Drawing::Presented);

    assert_eq!(received.try_recv(), Ok(Drawing::Presented));
  }

  #[test]
  fn a_keymap_over_the_limit_is_refused_before_reading() -> Result<(), Box<dyn std::error::Error>>
  {
    let empty = std::fs::File::open("/dev/null")?;
    let too_large = read_keymap(std::os::fd::OwnedFd::from(empty), u32::MAX);

    assert!(
      matches!(too_large, Err(KeymapError::Read(fault)) if fault.kind() == io::ErrorKind::InvalidData)
    );

    Ok(())
  }

  #[test]
  fn a_bar_along_the_top_reserves_how_deep_it_is_and_not_how_wide() {
    let Ok(zone) = reserved_size(
      Room::Reserves,
      Anchor::Top,
      Size {
        width: 1024,
        height: 38,
      },
    );

    assert_eq!(zone, 38);
  }

  #[test]
  fn a_sidebar_stretches_vertically_and_reserves_its_width_on_its_edge() {
    let screen = Size {
      width: 86,
      height: 640,
    };

    for (anchor, edge, opposite) in [
      (
        Anchor::Left,
        zwlr_layer_surface_v1::Anchor::Left,
        zwlr_layer_surface_v1::Anchor::Right,
      ),
      (
        Anchor::Right,
        zwlr_layer_surface_v1::Anchor::Right,
        zwlr_layer_surface_v1::Anchor::Left,
      ),
    ] {
      let Ok(anchored) = edges(anchor);
      assert!(anchored.contains(
        edge | zwlr_layer_surface_v1::Anchor::Top | zwlr_layer_surface_v1::Anchor::Bottom
      ));
      assert!(!anchored.contains(opposite));
      assert_eq!(
        exclusive_zone(anchor, screen),
        Ok(Size {
          width: 86,
          height: 0
        })
      );
      assert_eq!(reserved_size(Room::Reserves, anchor, screen), Ok(86));
    }
  }

  #[test]
  fn a_surface_that_takes_no_room_reserves_nothing_whichever_edge_it_is_on() {
    for anchor in [Anchor::Top, Anchor::TopRight, Anchor::Bottom, Anchor::Whole] {
      let Ok(zone) = reserved_size(
        Room::Over,
        anchor,
        Size {
          width: 1024,
          height: 38,
        },
      );

      assert_eq!(zone, -1, "{anchor:?}");
    }
  }

  #[test]
  fn a_surface_held_by_opposite_edges_is_never_given_a_size_along_them() {
    let screen = Size {
      width: 1024,
      height: 640,
    };

    assert_eq!(
      exclusive_zone(Anchor::Whole, screen),
      Ok(Size {
        width: 0,
        height: 0
      }),
      "a keyboard coming up would push it off the screen"
    );
    assert_eq!(
      exclusive_zone(Anchor::Top, screen),
      Ok(Size {
        width: 0,
        height: 640
      })
    );
    assert_eq!(
      exclusive_zone(Anchor::Bottom, screen),
      Ok(Size {
        width: 0,
        height: 640
      })
    );
    assert_eq!(exclusive_zone(Anchor::TopRight, screen), Ok(screen));
  }

  #[test]
  fn a_bar_is_anchored_to_three_edges_so_that_it_is_as_wide_as_the_screen() {
    let Ok(edges) = edges(Anchor::Top);

    assert!(edges.contains(zwlr_layer_surface_v1::Anchor::Left));
    assert!(edges.contains(zwlr_layer_surface_v1::Anchor::Right));
    assert!(edges.contains(zwlr_layer_surface_v1::Anchor::Top));
    assert!(!edges.contains(zwlr_layer_surface_v1::Anchor::Bottom));
  }

  #[test]
  fn a_card_is_over_everything_and_a_bar_is_under_what_comes_up_over_it() {
    let Ok(card) = among(Under::None);
    let Ok(bar) = among(Under::Anything);
    let Ok(home) = among(Under::AWindow);

    assert_eq!(card, zwlr_layer_shell_v1::Layer::Overlay);
    assert_eq!(bar, zwlr_layer_shell_v1::Layer::Top);
    assert_eq!(home, zwlr_layer_shell_v1::Layer::Bottom);
  }

  #[test]
  fn a_frame_is_never_painted_into_a_buffer_the_compositor_still_holds() {
    let size = Size {
      width: 4,
      height: 2,
    };
    let smaller = Size {
      width: 2,
      height: 2,
    };
    let spare = vec![
      ("held", size, Buffer::Acquired),
      ("other size", smaller, Buffer::Released),
      ("free", size, Buffer::Released),
    ];

    let Ok((taken, kept)) = pick_free(spare, size, |frame| (frame.1, frame.2));
    let kept_2: Vec<&str> = kept.iter().map(|frame| frame.0).collect();

    assert_eq!(taken.map(|frame| frame.0), Some("free"));
    assert_eq!(kept_2, vec!["held"]);

    let Ok((taken_2, kept_3)) =
      pick_free(vec![("held", size, Buffer::Acquired)], size, |frame| {
        (frame.1, frame.2)
      });

    assert_eq!(
      taken_2, None,
      "the only buffer is still on the screen, so a new one has to be cut"
    );
    assert_eq!(kept_3.len(), 1);
  }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Bound {
  fn event(
    _bound: &mut Self,
    _registry: &wl_registry::WlRegistry,
    _event: wl_registry::Event,
    _held: &GlobalListContents,
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
  }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for Bound {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    bound: &mut Self,
    layer: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    event: zwlr_layer_surface_v1::Event,
    _held: &(),
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    match event {
      zwlr_layer_surface_v1::Event::Configure {
        serial,
        width,
        height,
      } => {
        layer.ack_configure(serial);

        let logical = Size {
          width: width.max(1),
          height: height.max(1),
        };
        let Ok(zone) = reserved_size(bound.room, bound.anchor, logical);

        layer.set_exclusive_zone(zone);

        bound.logical = Some(logical);
      }
      zwlr_layer_surface_v1::Event::Closed => bound.closed = Closed::Yes,
      _ => {}
    }
  }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for Bound {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    _bound: &mut Self,
    windows: &xdg_wm_base::XdgWmBase,
    event: xdg_wm_base::Event,
    _held: &(),
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    match event {
      xdg_wm_base::Event::Ping { serial } => windows.pong(serial),
      _ => {}
    }
  }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for Bound {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    bound: &mut Self,
    framed: &xdg_surface::XdgSurface,
    event: xdg_surface::Event,
    _held: &(),
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    match event {
      xdg_surface::Event::Configure { serial } => {
        framed.ack_configure(serial);

        let fallback = match bound.logical.or(bound.asked) {
          Some(size) => size,
          None => Size {
            width: 1,
            height: 1,
          },
        };
        bound.logical = Some(Size {
          width: match bound.given.width {
            0 => fallback.width.max(1),
            width => width,
          },
          height: match bound.given.height {
            0 => fallback.height.max(1),
            height => height,
          },
        });
      }
      _ => {}
    }
  }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for Bound {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    bound: &mut Self,
    _toplevel: &xdg_toplevel::XdgToplevel,
    event: xdg_toplevel::Event,
    _held: &(),
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    match event {
      xdg_toplevel::Event::Configure {
        width,
        height,
        states: _,
      } => {
        let Ok(width_2) = fitted::<i32, u32>(width);
        let Ok(height_2) = fitted::<i32, u32>(height);

        bound.given = Size {
          width: width_2,
          height: height_2,
        };
      }
      xdg_toplevel::Event::Close => bound.closed = Closed::Yes,
      _ => {}
    }
  }
}

impl Dispatch<ext_session_lock_v1::ExtSessionLockV1, ()> for Bound {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    bound: &mut Self,
    _session: &ext_session_lock_v1::ExtSessionLockV1,
    event: ext_session_lock_v1::Event,
    _held: &(),
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    match event {
      ext_session_lock_v1::Event::Locked => bound.lock = Lock::Acquired,
      ext_session_lock_v1::Event::Finished => bound.lock = Lock::Denied,
      _ => {}
    }
  }
}

impl Dispatch<ext_session_lock_surface_v1::ExtSessionLockSurfaceV1, ()> for Bound {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    bound: &mut Self,
    surface: &ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
    event: ext_session_lock_surface_v1::Event,
    _held: &(),
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    match event {
      ext_session_lock_surface_v1::Event::Configure {
        serial,
        width,
        height,
      } => {
        surface.ack_configure(serial);

        bound.logical = Some(Size {
          width: width.max(1),
          height: height.max(1),
        });
      }
      _ => {}
    }
  }
}

impl Dispatch<wp_fractional_scale_v1::WpFractionalScaleV1, ()> for Bound {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    bound: &mut Self,
    _fraction: &wp_fractional_scale_v1::WpFractionalScaleV1,
    event: wp_fractional_scale_v1::Event,
    _held: &(),
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    match event {
      wp_fractional_scale_v1::Event::PreferredScale { scale } => {
        let Ok(said) = Scale::of(scale);

        bound.scale = said;
      }
      _ => {}
    }
  }
}

impl Dispatch<wl_seat::WlSeat, ()> for Bound {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    _bound: &mut Self,
    seat: &wl_seat::WlSeat,
    event: wl_seat::Event,
    _held: &(),
    _connection: &Connection,
    hand: &QueueHandle<Self>,
  ) {
    match event {
      wl_seat::Event::Capabilities {
        capabilities: wayland_client::WEnum::Value(has),
      } => {
        match ::core::primitive::bool::then_some(has.contains(wl_seat::Capability::Pointer), ()) {
          ::core::option::Option::Some(()) => {
            let _ = seat.get_pointer(hand, ());
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some(has.contains(wl_seat::Capability::Touch), ()) {
          ::core::option::Option::Some(()) => {
            let _ = seat.get_touch(hand, ());
          }
          ::core::option::Option::None => {}
        }

        match ::core::primitive::bool::then_some(has.contains(wl_seat::Capability::Keyboard), ()) {
          ::core::option::Option::Some(()) => {
            let _ = seat.get_keyboard(hand, ());
          }
          ::core::option::Option::None => {}
        }
      }
      _ => {}
    }
  }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for Bound {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    bound: &mut Self,
    _keyboard: &wl_keyboard::WlKeyboard,
    event: wl_keyboard::Event,
    _held: &(),
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    match event {
      wl_keyboard::Event::Keymap {
        format: wayland_client::WEnum::Value(wl_keyboard::KeymapFormat::XkbV1),
        fd,
        size,
      } => match read_keymap(fd, size) {
        Ok(typing) => bound.typing = Some(typing),
        Err(why) => console_core_logging::warn!("{why}"),
      },
      wl_keyboard::Event::Enter { surface, .. } => {
        match bound.up.as_ref().filter(|up| up.surface == surface) {
          Some(_ours) => bound.focus = KeyboardFocus::Here,
          None => {}
        }
      }
      wl_keyboard::Event::Leave { .. } => bound.focus = KeyboardFocus::Elsewhere,
      wl_keyboard::Event::Modifiers {
        mods_depressed,
        mods_latched,
        mods_locked,
        group,
        ..
      } => match bound.typing.as_mut() {
        Some(typing) => {
          typing.held.mods_pressed = ModifierMask(mods_depressed);
          typing.held.mods_latched = ModifierMask(mods_latched);
          typing.held.mods_locked = ModifierMask(mods_locked);
          typing.held.group_locked = GroupIndex(group);

          let Ok(()) = typing.held.update_effective();
        }
        None => {}
      },
      wl_keyboard::Event::Key {
        key,
        state: wayland_client::WEnum::Value(wl_keyboard::KeyState::Pressed),
        ..
      } => match bound.typing.as_ref() {
        Some(typing) => {
          let Ok(said) = typing.said(key);

          bound.keyboard_events.push(KeyPress {
            key: said,
            modifiers: typing.held.mods.0,
          });
        }
        None => {}
      },
      _ => {}
    }
  }
}

struct Typing {
  table: LookupTable,
  held: Components,
}

impl Typing {
  fn said(&self, key: u32) -> Result<Keysym, Never> {
    let Ok(code) = Keycode::from_evdev(key);
    let Ok(mut found) = self.table.lookup(self.held.group, self.held.mods, code);

    match (found.next(), found.next()) {
      (Some(one), None) => Ok(one),
      (None, _) | (Some(_), Some(_)) => Ok(Keysym::NoSymbol),
    }
  }
}

#[derive(Debug)]
enum KeymapError {
  Read(io::Error),
  Parse(Diagnostic),
}

impl std::fmt::Display for KeymapError {
  fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      KeymapError::Read(why) => write!(to, "no keymap to read: {why}"),
      KeymapError::Parse(why) => write!(to, "the compositor's keymap would not compile: {why}"),
    }
  }
}

fn read_keymap(fd: std::os::fd::OwnedFd, size: u32) -> Result<Typing, KeymapError> {
  const MAX_KEYMAP_BYTES: u32 = 4 * 1024 * 1024;

  match ::core::primitive::bool::then_some(size <= MAX_KEYMAP_BYTES, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      return Err(KeymapError::Read(io::Error::new(
        io::ErrorKind::InvalidData,
        "the compositor's keymap exceeds 4 MiB",
      )));
    }
  }

  let file = std::fs::File::from(fd);
  let Ok(many) = index(size);
  #[cfg_attr(
    dylint_lib = "explicit088_no_arriving_length",
    expect(
      explicit088_no_arriving_length,
      reason = "larger lengths have already been refused against the 4 MiB keymap limit"
    )
  )]
  let mut text = vec![0_u8; many];

  match std::os::unix::fs::FileExt::read_exact_at(&file, &mut text, 0) {
    Ok(()) => {}
    Err(why) => return Err(KeymapError::Read(why)),
  }

  match text.iter().position(|byte| *byte == 0) {
    Some(end) => text.truncate(end),
    None => {}
  }

  let compiled = Context::default().keymap_from_bytes(
    (Vec::<Diagnostic>::new(), Severity::Error),
    None,
    &text,
  );

  let keymap = match compiled {
    Ok(keymap) => keymap,
    Err(why) => return Err(KeymapError::Parse(why)),
  };
  let Ok(builder) = keymap.to_builder();
  let Ok(table) = builder.build_lookup_table();

  Ok(Typing {
    table,
    held: Components::default(),
  })
}

fn pointer_moved(was: &mut (f64, f64), at: (f64, f64)) -> Result<Option<PointerEvent>, Never> {
  let moved = match ::core::primitive::bool::then_some(*was == at, ()) {
    ::core::option::Option::Some(()) => None,
    ::core::option::Option::None => Some(PointerEvent::Moved { at }),
  };

  *was = at;

  Ok(moved)
}

impl Dispatch<wl_pointer::WlPointer, ()> for Bound {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    bound: &mut Self,
    _pointer: &wl_pointer::WlPointer,
    event: wl_pointer::Event,
    _held: &(),
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    match event {
      wl_pointer::Event::Enter {
        surface_x,
        surface_y,
        ..
      } => {
        bound.pointer_at = (surface_x, surface_y);
      }
      wl_pointer::Event::Motion {
        surface_x,
        surface_y,
        ..
      } => {
        let Ok(moved) = pointer_moved(&mut bound.pointer_at, (surface_x, surface_y));

        bound.pointer_events.extend(moved);
      }
      wl_pointer::Event::Leave { .. } => {
        bound.pointer_serial = None;
        bound.pointer_events.push(PointerEvent::Left);
      }
      wl_pointer::Event::Axis { axis, value, .. } => match axis {
        wayland_client::WEnum::Value(wl_pointer::Axis::VerticalScroll) => {
          bound
            .pointer_events
            .push(PointerEvent::Scrolled { by: value });
        }
        wayland_client::WEnum::Value(wl_pointer::Axis::HorizontalScroll)
        | wayland_client::WEnum::Value(_)
        | wayland_client::WEnum::Unknown(_) => {}
      },
      wl_pointer::Event::Button {
        state,
        serial,
        button,
        ..
      } => match state {
        wayland_client::WEnum::Value(wl_pointer::ButtonState::Pressed) => {
          bound.pointer_serial = (button == 0x110).then_some(serial);
          bound.pointer_events.push(PointerEvent::Down {
            at: bound.pointer_at,
          });
        }
        wayland_client::WEnum::Value(wl_pointer::ButtonState::Released)
        | wayland_client::WEnum::Unknown(_)
        | wayland_client::WEnum::Value(_) => {
          bound.pointer_serial = None;
          bound.pointer_events.push(PointerEvent::Up);
        }
      },
      _ => {}
    }
  }
}

#[cfg(test)]
mod pointer_tests {
  use super::*;

  #[test]
  fn a_motion_at_the_entered_position_leaves_the_highlight_alone() {
    let mut at = (512.0, 280.0);
    let Ok(moved) = pointer_moved(&mut at, (512.0, 280.0));

    assert_eq!(moved, None);
  }

  #[test]
  fn moving_after_entering_still_reaches_the_panel() {
    let mut at = (512.0, 280.0);
    let Ok(moved) = pointer_moved(&mut at, (512.0, 281.0));
    let Ok(resting) = pointer_moved(&mut at, (512.0, 281.0));
    let Ok(back) = pointer_moved(&mut at, (512.0, 280.0));

    assert_eq!(moved, Some(PointerEvent::Moved { at: (512.0, 281.0) }));
    assert_eq!(resting, None);
    assert_eq!(back, Some(PointerEvent::Moved { at: (512.0, 280.0) }));
    assert_eq!(at, (512.0, 280.0));
  }
}

impl Dispatch<wl_touch::WlTouch, ()> for Bound {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    bound: &mut Self,
    _touch: &wl_touch::WlTouch,
    event: wl_touch::Event,
    _held: &(),
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    let touch = match event {
      wl_touch::Event::Down { id, x, y, .. } => Touch::Down { id, at: (x, y) },
      wl_touch::Event::Motion { id, x, y, .. } => Touch::Moved { id, at: (x, y) },
      wl_touch::Event::Up { id, .. } => Touch::Up { id },
      wl_touch::Event::Cancel => Touch::Cancelled,
      _ => return,
    };
    let Ok(heard) = bound.fingers.handle_touch(touch);

    bound.pointer_events.extend(heard);
  }
}

delegate_noop!(Bound: ignore wl_compositor::WlCompositor);
delegate_noop!(Bound: ignore wl_surface::WlSurface);
delegate_noop!(Bound: ignore wl_region::WlRegion);
delegate_noop!(Bound: ignore wl_subcompositor::WlSubcompositor);
delegate_noop!(Bound: ignore wl_subsurface::WlSubsurface);
delegate_noop!(Bound: ignore zwp_text_input_manager_v3::ZwpTextInputManagerV3);
delegate_noop!(Bound: ignore wl_shm::WlShm);
delegate_noop!(Bound: ignore wl_shm_pool::WlShmPool);
delegate_noop!(Bound: ignore zwlr_layer_shell_v1::ZwlrLayerShellV1);
delegate_noop!(Bound: ignore ext_session_lock_manager_v1::ExtSessionLockManagerV1);
delegate_noop!(Bound: ignore wl_output::WlOutput);
delegate_noop!(Bound: ignore wp_viewporter::WpViewporter);
delegate_noop!(Bound: ignore wp_viewport::WpViewport);
delegate_noop!(Bound: ignore wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1);

impl Dispatch<zwp_text_input_v3::ZwpTextInputV3, ()> for Bound {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    expect(
      explicit016_no_wildcard_arm,
      reason = "Wayland text input events are non-exhaustive and future protocol events have no meaning in this version"
    )
  )]
  fn event(
    bound: &mut Self,
    _: &zwp_text_input_v3::ZwpTextInputV3,
    event: zwp_text_input_v3::Event,
    _: &(),
    _: &Connection,
    _: &QueueHandle<Self>,
  ) {
    let input = match bound.text_input.as_mut() {
      Some(input) => input,
      None => return,
    };

    use zwp_text_input_v3::Event;

    match event {
      Event::Enter { surface } => {
        let entered = bound.up.as_ref().is_some_and(|up| up.surface == surface);
        let focused = match entered.then_some(()) {
          Some(()) => TextFocus::Entered,
          None => TextFocus::Left,
        };
        let Ok(()) = input.focus(focused);
      }
      Event::Leave { .. } => {
        let Ok(()) = input.focus(TextFocus::Left);
      }
      Event::PreeditString { text, cursor_begin, cursor_end } => {
        input.pending.preedit = text.filter(|preedit| !preedit.is_empty()).map(|preedit| Composition {
          text: preedit, cursor_begin, cursor_end,
        });
      }
      Event::CommitString { text } => input.pending.commit = text,
      Event::DeleteSurroundingText { before_length, after_length } => {
        input.pending.delete_before = before_length;
        input.pending.delete_after = after_length;
      }
      Event::Done { serial } => {
        let Ok(()) = input.done(serial);
      }
      _ => {}
    }
  }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for Bound {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    bound: &mut Self,
    buffer: &wl_buffer::WlBuffer,
    event: wl_buffer::Event,
    _held: &(),
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    match event {
      wl_buffer::Event::Release => {
        let region_frames = bound.region.iter_mut().flat_map(|layer| layer.shown.iter_mut().chain(layer.spare.iter_mut()));

        for frame in bound.shown.iter_mut().chain(bound.spare.iter_mut()).chain(region_frames) {
          match ::core::primitive::bool::then_some(frame.buffer == *buffer, ()) {
            ::core::option::Option::Some(()) => frame.held = Buffer::Released,
            ::core::option::Option::None => {}
          }
        }
      }
      _ => {}
    }
  }
}

impl Dispatch<wp_presentation::WpPresentation, ()> for Bound {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    bound: &mut Self,
    _presentation: &wp_presentation::WpPresentation,
    event: wp_presentation::Event,
    _held: &(),
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    match event {
      wp_presentation::Event::ClockId { clk_id } => bound.clock = Some(clk_id),
      _ => {}
    }
  }
}

impl Dispatch<wp_presentation_feedback::WpPresentationFeedback, Committed> for Bound {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
    )
  )]
  fn event(
    bound: &mut Self,
    _feedback: &wp_presentation_feedback::WpPresentationFeedback,
    event: wp_presentation_feedback::Event,
    committed: &Committed,
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    let (tally, clock) = match (bound.timing.as_mut(), bound.clock) {
      (Some(tally), Some(clock)) => (tally, clock),
      (None, _) | (_, None) => return,
    };

    match event {
      wp_presentation_feedback::Event::Presented {
        tv_sec_hi,
        tv_sec_lo,
        tv_nsec,
        refresh,
        ..
      } => {
        let seconds = u64::from(tv_sec_hi)
          .saturating_mul(SECONDS_IN_THE_HIGH_WORD)
          .saturating_add(u64::from(tv_sec_lo));
        let at = Duration::new(seconds, tv_nsec);
        let refresh_2 = Duration::from_nanos(u64::from(refresh));

        let Ok(()) = tally.presented(
          *committed,
          Presented {
            clock,
            at,
            refresh: refresh_2,
          },
        );
      }
      wp_presentation_feedback::Event::Discarded => {
        let Ok(()) = tally.discarded(*committed);
      }
      _ => {}
    }
  }
}

impl Dispatch<wp_presentation_feedback::WpPresentationFeedback, DrawingId> for Bound {
  #[cfg_attr(
    dylint_lib = "explicit016_no_wildcard_arm",
    allow(
      explicit016_no_wildcard_arm,
      reason = "Wayland events are non_exhaustive; only Presented and Discarded complete this frame's feedback"
    )
  )]
  fn event(
    bound: &mut Self,
    _feedback: &wp_presentation_feedback::WpPresentationFeedback,
    event: wp_presentation_feedback::Event,
    notice: &DrawingId,
    _connection: &Connection,
    _hand: &QueueHandle<Self>,
  ) {
    let finished = match event {
      wp_presentation_feedback::Event::Presented { .. } => Drawing::Presented,
      wp_presentation_feedback::Event::Discarded => Drawing::Discarded,
      _ => return,
    };

    let Ok(()) = finish_drawing(&mut bound.drawing, *notice, finished);
  }
}
