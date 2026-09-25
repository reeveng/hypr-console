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
//!
//! ## A key arrives as a code and is read through the person's own keymap
//!
//! The compositor hands over the keymap it is driving and then keycodes
//! against it, and what a panel wants to know is which keysym a press means --
//! which is a question the keymap answers and a table here would get wrong for
//! anybody typing in a language this device was not set up in. The one number
//! that is ours is the eight between an evdev code and an xkb one, which is
//! historical and never changes.
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
//! ## A closed socket is not quiet
//!
//! `poll` asked only about `POLLIN` returns immediately and forever on a
//! descriptor whose far end has gone, because the hangup is on `POLLHUP` and
//! nothing reads it. That is a program at a whole core drawing on a screen that
//! is not there. It is asked about by name here.

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::{fitted, index, toward_zero_i32};
use rustix::event::{Nsecs, PollFd, PollFlags, Secs, Timespec, poll};
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::time::Duration;

use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_shm, wl_shm_pool,
    wl_surface, wl_touch,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, delegate_noop};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1, wp_fractional_scale_v1,
};
use wayland_protocols::wp::presentation_time::client::{wp_presentation, wp_presentation_feedback};

use console_response_times::frames::{Buffer, Committed, Frames, Presented};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use xkbcommon::xkb;

pub use xkb::Keysym;

use crate::fingers::{Fingers, Touch};
use crate::memory::Shared;
use crate::scale::Scale;

const EVDEV_IS_EIGHT_BEHIND_XKB: u32 = 8;
const SECONDS_IN_THE_HIGH_WORD: u64 = 0x1_0000_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Top,
    TopRight,
    Bottom,
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
    let (right, bottom) = (left + f64::from(part.size.width), top + f64::from(part.size.height));
    let Ok(left) = toward_zero_i32((left * across).floor());
    let Ok(top) = toward_zero_i32((top * down).floor());
    let Ok(right) = toward_zero_i32((right * across).ceil());
    let Ok(bottom) = toward_zero_i32((bottom * down).ceil());

    Ok(Damage {
        at: Point { x: left, y: top },
        size: Size { width: right.saturating_sub(left), height: bottom.saturating_sub(top) },
    })
}

struct Up {
    surface: wl_surface::WlSurface,
    layer: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    viewport: wp_viewport::WpViewport,
    fraction: Option<wp_fractional_scale_v1::WpFractionalScaleV1>,
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
        let Ok(held) = shared.held();
        let Ok(many) = fitted::<u32, i32>(bytes);
        let pool = shm.create_pool(held.as_fd(), many, hand, ());
        let Ok(wide) = fitted::<u32, i32>(size.width);
        let Ok(tall) = fitted::<u32, i32>(size.height);
        let Ok(across) = fitted::<u32, i32>(stride);
        let buffer =
            pool.create_buffer(0, wide, tall, across, wl_shm::Format::Argb8888, hand, ());

        Ok(Frame { _shared: shared, pool, buffer, size, held: Buffer::Released })
    }
}

fn free(spare: &mut Vec<Frame>, shm: &wl_shm::WlShm, hand: &QueueHandle<Bound>, device: Size<u32>) -> Result<Frame, SurfaceError> {
    let Ok((taken, kept)) = picked(std::mem::take(spare), device, |frame| (frame.size, frame.held));

    *spare = kept;

    match taken {
        Some(frame) => Ok(frame),
        None => Frame::new(shm, hand, device),
    }
}

fn picked<T>(spare: Vec<T>, device: Size<u32>, state: impl Fn(&T) -> (Size<u32>, Buffer)) -> Result<(Option<T>, Vec<T>), Never> {
    let (mut free, mut busy): (Vec<T>, Vec<T>) = spare
        .into_iter()
        .filter(|frame| state(frame).0 == device)
        .partition(|frame| state(frame).1 == Buffer::Released);

    let taken = free.pop();

    busy.extend(free);

    Ok((taken, busy))
}

impl Bound {
    fn showing(&mut self, frame: Frame) -> Result<(), Never> {
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
    shm: wl_shm::WlShm,
    shell: zwlr_layer_shell_v1::ZwlrLayerShellV1,
    viewporter: wp_viewporter::WpViewporter,
    fractions: Option<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1>,
    presentation: Option<wp_presentation::WpPresentation>,
    clock: Option<u32>,
    timing: Option<Frames>,
    up: Option<Up>,
    shown: Option<Frame>,
    spare: Vec<Frame>,
    logical: Option<Size<u32>>,
    asked: Option<Size<u32>>,
    anchor: Anchor,
    room: Room,
    scale: Scale,
    closed: Closed,
    pointer_events: Vec<PointerEvent>,
    pointer_at: (f64, f64),
    fingers: Fingers,
    reading: xkb::Context,
    typing: Option<xkb::State>,
    keyboard_events: Vec<KeyboardEvent>,
}

pub struct Surface {
    connection: Connection,
    queue: EventQueue<Bound>,
    bound: Bound,
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
        let shm = globals
            .bind::<wl_shm::WlShm, _, _>(&hand, 1..=1, ())
            .map_err(|_| SurfaceError::Global("wl_shm"))?;
        let shell = globals
            .bind::<zwlr_layer_shell_v1::ZwlrLayerShellV1, _, _>(&hand, 1..=4, ())
            .map_err(|_| SurfaceError::Global("zwlr_layer_shell_v1, which is what makes a surface ours"))?;
        let viewporter = globals
            .bind::<wp_viewporter::WpViewporter, _, _>(&hand, 1..=1, ())
            .map_err(|_| SurfaceError::Global("wp_viewporter, which is the whole of drawing at 2.5"))?;
        let fractions = match globals
            .bind::<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1, _, _>(
                &hand,
                1..=1,
                (),
            ) {
            Ok(manager) => Some(manager),
            Err(_) => {
                eprintln!(
                    "this compositor has no wp_fractional_scale_v1, so every surface here \
                     is drawn at whole pixels and a screen driven at a fraction will be soft"
                );

                None
            }
        };
        let presentation = match globals.bind::<wp_presentation::WpPresentation, _, _>(&hand, 1..=1, ()) {
            Ok(presentation) => Some(presentation),
            Err(_) => {
                eprintln!(
                    "this compositor has no wp_presentation, so every frame here is counted \
                     and none of them is timed"
                );

                None
            }
        };
        let _ = globals.bind::<wl_seat::WlSeat, _, _>(&hand, 1..=7, ());

        let bound = Bound {
            compositor,
            shm,
            shell,
            viewporter,
            fractions,
            presentation,
            clock: None,
            timing: None,
            up: None,
            shown: None,
            spare: Vec::new(),
            logical: None,
            asked: None,
            anchor: Anchor::Whole,
            room: Room::Over,
            scale: Scale::ONE,
            closed: Closed::No,
            pointer_events: Vec::new(),
            pointer_at: (0.0, 0.0),
            fingers: Fingers::default(),
            reading: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
            typing: None,
            keyboard_events: Vec::new(),
        };

        Ok(Surface { connection, queue, bound })
    }

    pub fn show(&mut self, wanted: &Wanted) -> Result<(), SurfaceError> {
        match self.bound.up {
            Some(_) => return Ok(()),
            None => {},
        }

        let hand = self.queue.handle();
        let surface = self.bound.compositor.create_surface(&hand, ());
        let viewport = self.bound.viewporter.get_viewport(&surface, &hand, ());
        let fraction = self
            .bound
            .fractions
            .as_ref()
            .map(|manager| manager.get_fractional_scale(&surface, &hand, ()));
        let Ok(among) = among(wanted.under);
        let layer = self.bound.shell.get_layer_surface(
            &surface,
            None,
            among,
            wanted.namespace.clone(),
            &hand,
            (),
        );
        let Ok(held) = held(wanted.anchor, wanted.size);

        layer.set_size(held.width, held.height);
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
        self.bound.up = Some(Up { surface, layer, viewport, fraction });
        let Ok(timing) = Frames::shown(&wanted.namespace);

        self.bound.timing = Some(timing);

        while self.bound.logical.is_none() && self.bound.closed == Closed::No {
            self.queue.blocking_dispatch(&mut self.bound).map_err(SurfaceError::Went)?;
        }

        Ok(())
    }

    pub fn resize(&mut self, size: Size<u32>) -> Result<(), Never> {
        let up = match self.bound.up.as_ref() {
            Some(up) => up,
            None => return Ok(()),
        };

        match self.bound.asked {
            Some(asked) => match asked == size {
                true => return Ok(()),
                false => {},
            },
            None => {},
        }

        let Ok(held) = held(self.bound.anchor, size);

        up.layer.set_size(held.width, held.height);
        up.surface.commit();
        let _ = self.connection.flush();

        self.bound.asked = Some(size);

        Ok(())
    }

    pub fn room(&mut self, room: Room) -> Result<(), Never> {
        match self.bound.room == room {
            true => return Ok(()),
            false => {},
        }

        self.bound.room = room;

        let up = match self.bound.up.as_ref() {
            Some(up) => up,
            None => return Ok(()),
        };
        let asked = match self.bound.logical {
            Some(logical) => logical,
            None => Size { width: 1, height: 1 },
        };
        let Ok(zone) = reserved(room, self.bound.anchor, asked);
        let anything = Size { width: 0, height: 0 };

        up.layer.set_exclusive_zone(zone);
        up.layer.set_size(anything.width, anything.height);
        up.surface.commit();
        self.bound.asked = Some(anything);
        self.bound.logical = None;
        let _ = self.connection.flush();

        Ok(())
    }

    pub fn hide(&mut self) -> Result<(), Never> {
        let up = match self.bound.up.take() {
            Some(up) => up,
            None => return Ok(()),
        };

        match up.fraction {
            Some(fraction) => fraction.destroy(),
            None => {},
        }

        up.viewport.destroy();
        up.layer.destroy();
        up.surface.destroy();
        self.bound.shown = None;
        self.bound.spare = Vec::new();

        match self.bound.timing.take() {
            Some(timing) => {
                let Ok(()) = timing.done();
            }
            None => {},
        }

        self.bound.logical = None;
        self.bound.asked = None;
        let _ = self.connection.flush();

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
        Ok(std::mem::take(&mut self.bound.keyboard_events))
    }

    pub fn pointer_events(&mut self) -> Result<Vec<PointerEvent>, Never> {
        Ok(std::mem::take(&mut self.bound.pointer_events))
    }


    pub fn draw(
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
        let Ok(painting) = painting(self.bound.timing.as_ref());
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
        let Ok(()) = self.bound.showing(frame);
        let Ok(()) = timed(Timing { bound: &mut self.bound, painting, held, hand: &hand });
        let up = match self.bound.up.as_ref() {
            Some(up) => up,
            None => return Ok(()),
        };

        up.surface.commit();
        let _ = self.connection.flush();

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
            Some(shown) => match shown.size == device {
                true => shown,
                false => return Ok(Visible::NotYet),
            },
            None => return Ok(Visible::NotYet),
        };
        let Ok(before) = under._shared.pixels();
        let before = before.to_vec();
        let mut frame = free(&mut self.bound.spare, &self.bound.shm, &hand, device)?;
        let Ok(painting) = painting(self.bound.timing.as_ref());
        let Ok(pixels) = frame._shared.pixels();

        pixels.copy_from_slice(&before);
        let Ok(()) = paint(pixels, device, scale);

        let Ok(across) = fitted::<u32, i32>(logical.width);
        let Ok(down) = fitted::<u32, i32>(logical.height);
        let Ok(damaged) = on_the_device(part, logical, device);

        up.surface.attach(Some(&frame.buffer), 0, 0);
        up.surface.set_buffer_scale(1);
        up.viewport.set_destination(across, down);
        up.surface.damage_buffer(damaged.at.x, damaged.at.y, damaged.size.width, damaged.size.height);
        let held = std::mem::replace(&mut frame.held, Buffer::Acquired);
        let Ok(()) = self.bound.showing(frame);
        let Ok(()) = timed(Timing { bound: &mut self.bound, painting, held, hand: &hand });
        let up = match self.bound.up.as_ref() {
            Some(up) => up,
            None => return Ok(Visible::NotYet),
        };

        up.surface.commit();
        let _ = self.connection.flush();

        Ok(Visible::Yes)
    }

    pub fn wait(&mut self, also: &[BorrowedFd<'_>], until: Option<Duration>) -> Result<(), SurfaceError> {
        self.queue.dispatch_pending(&mut self.bound).map_err(SurfaceError::Went)?;
        let _ = self.connection.flush();

        let guard = match self.connection.prepare_read() {
            Some(guard) => guard,
            None => {
                self.queue.dispatch_pending(&mut self.bound).map_err(SurfaceError::Went)?;

                return Ok(());
            }
        };

        let socket = self.connection.as_fd();
        let mut watch = vec![PollFd::from_borrowed_fd(socket, PollFlags::IN)];

        watch.extend(also.iter().map(|fd| PollFd::from_borrowed_fd(*fd, PollFlags::IN)));

        let wait = match until {
            None => None,
            Some(long) => {
                let Ok(seconds) = fitted::<u64, Secs>(long.as_secs());
                let Ok(nanoseconds) = fitted::<u32, Nsecs>(long.subsec_nanos());

                Some(Timespec { tv_sec: seconds, tv_nsec: nanoseconds })
            }
        };

        match poll(&mut watch, wait.as_ref()) {
            Ok(_) => {},
            Err(_) => {
                drop(guard);

                return Ok(());
            }
        }

        let hung = watch
            .first()
            .is_some_and(|first| first.revents().intersects(PollFlags::HUP | PollFlags::ERR));

        match hung {
            true => {
                drop(guard);
                self.bound.closed = Closed::Yes;

                return Err(SurfaceError::Hung);
            }
            false => {},
        }

        let readable = watch.first().is_some_and(|first| first.revents().contains(PollFlags::IN));

        match readable {
            true => {
                let carried_on = match guard.read() {
                    Ok(_) => true,
                    Err(wayland_client::backend::WaylandError::Io(why)) => why.kind() == io::ErrorKind::WouldBlock,
                    Err(_) => false,
                };

                match carried_on {
                    true => {}
                    false => {
                        self.bound.closed = Closed::Yes;

                        return Err(SurfaceError::Hung);
                    }
                }
            }
            false => drop(guard),
        }

        self.queue.dispatch_pending(&mut self.bound).map_err(SurfaceError::Went)?;

        Ok(())
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        let Ok(()) = self.hide();
    }
}

fn painting(timing: Option<&Frames>) -> Result<Option<console_response_times::frames::Painting>, Never> {
    Ok(match timing {
        Some(timing) => {
            let Ok(painting) = timing.painting();

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
}

fn timed(timing: Timing<'_>) -> Result<(), Never> {
    let Timing { bound, painting, held, hand } = timing;

    let (tally, painting) = match (bound.timing.as_mut(), painting) {
        (Some(tally), Some(painting)) => (tally, painting),
        (None, _) | (_, None) => return Ok(()),
    };

    let Ok(committed) = tally.committed(painting, held);

    match (committed, bound.presentation.as_ref(), bound.up.as_ref()) {
        (Some(committed), Some(presentation), Some(up)) => {
            let _ = presentation.feedback(&up.surface, hand, committed);
        }
        (None, _, _) | (_, None, _) | (_, _, None) => {},
    }

    Ok(())
}

fn reserved(room: Room, anchor: Anchor, logical: Size<u32>) -> Result<i32, Never> {
    let along = match anchor {
        Anchor::Top | Anchor::Bottom => logical.height,
        Anchor::TopRight | Anchor::Whole => logical.width,
    };
    let Ok(many) = fitted::<u32, i32>(along);

    Ok(match room {
        Room::Reserves => many,
        Room::Over => -1,
        Room::Around => 0,
    })
}

fn held(anchor: Anchor, size: Size<u32>) -> Result<Size<u32>, Never> {
    Ok(match anchor {
        Anchor::Whole => Size { width: 0, height: 0 },
        Anchor::Top | Anchor::Bottom => Size { width: 0, height: size.height },
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
        Anchor::TopRight => {
            zwlr_layer_surface_v1::Anchor::Top | zwlr_layer_surface_v1::Anchor::Right
        }
        Anchor::Bottom => {
            zwlr_layer_surface_v1::Anchor::Bottom
                | zwlr_layer_surface_v1::Anchor::Left
                | zwlr_layer_surface_v1::Anchor::Right
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
    #[test]
    fn a_part_redrawn_is_damaged_where_it_lands_on_the_glass_and_nowhere_else() {
        let part = super::Part { at: Point { x: 10, y: 20 }, size: Size { width: 100, height: 50 } };
        let logical = Size { width: 1000, height: 600 };

        assert_eq!(
            super::on_the_device(part, logical, Size { width: 1500, height: 900 }),
            Ok(super::Damage { at: Point { x: 15, y: 30 }, size: Size { width: 150, height: 75 } }),
        );
        assert_eq!(
            super::on_the_device(part, logical, logical),
            Ok(super::Damage { at: Point { x: 10, y: 20 }, size: Size { width: 100, height: 50 } }),
        );
    }

    #[test]
    fn a_part_on_a_fractional_scale_is_damaged_out_to_the_whole_pixels_it_touches() {
        let part = super::Part { at: Point { x: 1, y: 1 }, size: Size { width: 1, height: 1 } };
        let damaged = super::on_the_device(part, Size { width: 4, height: 4 }, Size { width: 5, height: 5 });

        assert_eq!(damaged, Ok(super::Damage { at: Point { x: 1, y: 1 }, size: Size { width: 2, height: 2 } }));
    }

    use super::*;

    #[test]
    fn a_bar_along_the_top_reserves_how_deep_it_is_and_not_how_wide() {
        let Ok(zone) = reserved(Room::Reserves, Anchor::Top, Size { width: 1024, height: 38 });

        assert_eq!(zone, 38);
    }

    #[test]
    fn a_surface_that_takes_no_room_reserves_nothing_whichever_edge_it_is_on() {
        for anchor in [Anchor::Top, Anchor::TopRight, Anchor::Bottom, Anchor::Whole] {
            let Ok(zone) = reserved(Room::Over, anchor, Size { width: 1024, height: 38 });

            assert_eq!(zone, -1, "{anchor:?}");
        }
    }

    #[test]
    fn a_surface_held_by_opposite_edges_is_never_given_a_size_along_them() {
        let screen = Size { width: 1024, height: 640 };

        assert_eq!(held(Anchor::Whole, screen), Ok(Size { width: 0, height: 0 }), "a keyboard coming up would push it off the screen");
        assert_eq!(held(Anchor::Top, screen), Ok(Size { width: 0, height: 640 }));
        assert_eq!(held(Anchor::Bottom, screen), Ok(Size { width: 0, height: 640 }));
        assert_eq!(held(Anchor::TopRight, screen), Ok(screen));
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
        let size = Size { width: 4, height: 2 };
        let smaller = Size { width: 2, height: 2 };
        let spare = vec![("held", size, Buffer::Acquired), ("other size", smaller, Buffer::Released), ("free", size, Buffer::Released)];

        let Ok((taken, kept)) = picked(spare, size, |frame| (frame.1, frame.2));
        let kept: Vec<&str> = kept.iter().map(|frame| frame.0).collect();

        assert_eq!(taken.map(|frame| frame.0), Some("free"));
        assert_eq!(kept, vec!["held"]);

        let Ok((taken, kept)) = picked(vec![("held", size, Buffer::Acquired)], size, |frame| (frame.1, frame.2));

        assert_eq!(taken, None, "the only buffer is still on the screen, so a new one has to be cut");
        assert_eq!(kept.len(), 1);
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
            zwlr_layer_surface_v1::Event::Configure { serial, width, height } => {
                layer.ack_configure(serial);

                let logical = Size { width: width.max(1), height: height.max(1) };
                let Ok(zone) = reserved(bound.room, bound.anchor, logical);

                layer.set_exclusive_zone(zone);

                bound.logical = Some(logical);
            }
            zwlr_layer_surface_v1::Event::Closed => bound.closed = Closed::Yes,
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
                match has.contains(wl_seat::Capability::Pointer) {
                    true => {
                        let _ = seat.get_pointer(hand, ());
                    }
                    false => {},
                }

                match has.contains(wl_seat::Capability::Touch) {
                    true => {
                        let _ = seat.get_touch(hand, ());
                    }
                    false => {},
                }

                match has.contains(wl_seat::Capability::Keyboard) {
                    true => {
                        let _ = seat.get_keyboard(hand, ());
                    }
                    false => {},
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
            } => {
                let Ok(many) = index(size);

                // SAFETY: the descriptor is the compositor's own keymap, handed
                // over for exactly this and owned here from now on, and the
                // length is the one it said in the same event.
                let read = unsafe {
                    xkb::Keymap::new_from_fd(
                        &bound.reading,
                        fd,
                        many,
                        xkb::KEYMAP_FORMAT_TEXT_V1,
                        xkb::KEYMAP_COMPILE_NO_FLAGS,
                    )
                };

                match read {
                    Ok(Some(keymap)) => bound.typing = Some(xkb::State::new(&keymap)),
                    Ok(None) => eprintln!("the compositor's keymap would not compile"),
                    Err(why) => eprintln!("no keymap to read: {why}"),
                }
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => match bound.typing.as_mut() {
                Some(typing) => {
                    typing.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);
                }
                None => {},
            },
            wl_keyboard::Event::Key {
                key,
                state: wayland_client::WEnum::Value(wl_keyboard::KeyState::Pressed),
                ..
            } => match bound.typing.as_ref() {
                Some(typing) => {
                    let code = xkb::Keycode::new(key.saturating_add(EVDEV_IS_EIGHT_BEHIND_XKB));
                    let said = typing.key_get_one_sym(code);

                    bound.keyboard_events.push(KeyboardEvent::Down { key: said });
                }
                None => {},
            },
            _ => {}
        }
    }
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
            wl_pointer::Event::Enter { surface_x, surface_y, .. } => {
                bound.pointer_at = (surface_x, surface_y);
            }
            wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
                bound.pointer_at = (surface_x, surface_y);

                bound.pointer_events.push(PointerEvent::Moved { at: bound.pointer_at });
            }
            wl_pointer::Event::Leave { .. } => bound.pointer_events.push(PointerEvent::Left),
            wl_pointer::Event::Axis { axis, value, .. } => match axis {
                wayland_client::WEnum::Value(wl_pointer::Axis::VerticalScroll) => {
                    bound.pointer_events.push(PointerEvent::Scrolled { by: value });
                }
                wayland_client::WEnum::Value(wl_pointer::Axis::HorizontalScroll)
                | wayland_client::WEnum::Value(_)
                | wayland_client::WEnum::Unknown(_) => {},
            },
            wl_pointer::Event::Button { state, .. } => match state {
                wayland_client::WEnum::Value(wl_pointer::ButtonState::Pressed) => {
                    bound.pointer_events.push(PointerEvent::Down { at: bound.pointer_at });
                }
                wayland_client::WEnum::Value(wl_pointer::ButtonState::Released)
                | wayland_client::WEnum::Unknown(_)
                | wayland_client::WEnum::Value(_) => bound.pointer_events.push(PointerEvent::Up),
            },
            _ => {}
        }
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
        let Ok(heard) = bound.fingers.heard(touch);

        bound.pointer_events.extend(heard);
    }
}

delegate_noop!(Bound: ignore wl_compositor::WlCompositor);
delegate_noop!(Bound: ignore wl_surface::WlSurface);
delegate_noop!(Bound: ignore wl_shm::WlShm);
delegate_noop!(Bound: ignore wl_shm_pool::WlShmPool);
delegate_noop!(Bound: ignore zwlr_layer_shell_v1::ZwlrLayerShellV1);
delegate_noop!(Bound: ignore wp_viewporter::WpViewporter);
delegate_noop!(Bound: ignore wp_viewport::WpViewport);
delegate_noop!(Bound: ignore wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1);

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
                for frame in bound.shown.iter_mut().chain(bound.spare.iter_mut()) {
                    match frame.buffer == *buffer {
                        true => frame.held = Buffer::Released,
                        false => {},
                    }
                }
            },
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
            wp_presentation_feedback::Event::Presented { tv_sec_hi, tv_sec_lo, tv_nsec, refresh, .. } => {
                let seconds = u64::from(tv_sec_hi).saturating_mul(SECONDS_IN_THE_HIGH_WORD).saturating_add(u64::from(tv_sec_lo));
                let at = Duration::new(seconds, tv_nsec);
                let refresh = Duration::from_nanos(u64::from(refresh));

                let Ok(()) = tally.presented(*committed, Presented { clock, at, refresh });
            }
            wp_presentation_feedback::Event::Discarded => {
                let Ok(()) = tally.discarded();
            }
            _ => {}
        }
    }
}
