//! The compositor, and the strip of screen the keyboard lives on.
//!
//! Everything here is what `main.c` does by hand against the C bindings: bind
//! the globals, make a surface, make it a layer surface anchored to the bottom
//! of the screen, wait to be told how big it is, and put pixels in a piece of
//! memory the compositor is reading at the same time.
//!
//! ## Why a layer surface and not a window
//!
//! A window is in the tiling layout and takes the focus, and a keyboard that
//! did either would be a keyboard that closes what you were typing into. A
//! layer surface is none of those things: it is anchored to an edge, it is
//! above or below the windows by rank rather than by order, and it can decline
//! the keyboard focus, which is exactly what a keyboard has to do -- the keys
//! it sends have to arrive at whatever was in front before it came up.
//!
//! It is also how the rest of this desktop knows the keyboard is there.
//! `console_door::is_open` asks the compositor for its list of layer surfaces
//! and looks for the name on this one, so [`NAMESPACE`] is a contract with the
//! controller daemon and with the bar, not a label.
//!
//! ## Hiding is going away
//!
//! There is no "hidden" for a layer surface, so hiding is destroying it and
//! showing is making another. That is what the C version does, and the desktop
//! is built on it: the door asks whether the compositor lists a keyboard, and
//! a keyboard that stayed listed while hidden would light the icon on the bar
//! for a keyboard nobody can see.


use console_number::fitted;
use std::os::fd::{AsFd, OwnedFd};

use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_pointer, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
    wl_touch,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, delegate_noop};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1, zwp_virtual_keyboard_v1,
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::shared_memory::{Mapped, drawing_buffer};

/// What the keyboard calls itself to the compositor.
///
/// `console_controller::mode::KEYBOARD` is the same word from the other side,
/// and `tests/the_namespace.rs` holds them together. Everything that lights up
/// while the keyboard is on the screen -- the icon on the bar, the daemon
/// standing down off the pad -- is downstream of this string.
pub const NAMESPACE: &str = "virtual-keyboard";

/// Four bytes a pixel, which is what both ends of this agree on.
///
/// Cairo's `ARgb32` and Wayland's `Argb8888` are the same thing said twice:
/// one 32-bit word per pixel, little-endian, so the bytes in memory run blue,
/// green, red, alpha. Nothing converts anything.
const DEEP: u32 = 4;

/// A thumb, arriving or leaving.
///
/// Touch and mouse are the same three things from here: something landed
/// somewhere, moved, or was lifted. The keyboard does not care which of them
/// it was, and a device that is tested with a mouse and used with a thumb is
/// better tested for it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Poke {
    /// Landed at these logical coordinates, measured from the top left of the
    /// keyboard rather than of the screen.
    Down { x: f64, y: f64 },
    /// Moved, still down.
    Moved { x: f64, y: f64 },
    /// Lifted.
    Up,
}

/// The keyboard's place on the screen, and everything the compositor gave us
/// to keep it there.
///
/// One struct rather than a handful, because wayland-rs dispatches events into
/// a single state and every one of these is touched from inside an event: the
/// buffer is remade when the size changes, and the size arrives as an event.
pub struct Screen {
    /// The connection, kept because the queue is flushed through it.
    connection: Connection,
    /// The events the compositor has for us.
    queue: EventQueue<Board>,
    /// Everything the events are about.
    board: Board,
}

/// The half of the screen that events are dispatched into.
pub struct Board {
    compositor: wl_compositor::WlCompositor,
    shm: wl_shm::WlShm,
    shell: zwlr_layer_shell_v1::ZwlrLayerShellV1,
    /// The seat, for the touches and the keys. Kept from the first binding
    /// because a seat that arrives later is a second seat, and this device has
    /// one.
    pub seat: wl_seat::WlSeat,
    /// The surface and its layer, or nothing at all while the keyboard is
    /// away. Hiding destroys both; showing makes them again.
    up: Option<Up>,
    /// The memory the pixels are in, kept across frames so that typing does
    /// not allocate. Remade when the size changes.
    frame: Option<Frame>,
    /// How big the compositor says the surface is, in logical units, and
    /// whether that has been acknowledged yet.
    pub size: Option<(u32, u32)>,
    /// Pixels per logical unit, from the output the keyboard is on.
    pub scale: i32,
    /// The compositor took the surface away. Nothing to do but go.
    pub closed: bool,
    /// The virtual keyboard factory, kept until somebody asks for one.
    pub typing: Option<zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1>,
    /// What the seat has said since the last time anybody looked.
    pub pokes: Vec<Poke>,
    /// Where the pointer was when it was last heard from, because a button
    /// press says which button and not where.
    pointer_at: (f64, f64),
    /// Whether a mouse button is down, so motion with nothing pressed is not
    /// read as a finger sliding across the keys.
    pointer_down: bool,
}

/// Whether the keyboard is on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Showing {
    /// It is up, and there is something to draw into.
    Yes,
    /// Nothing is on the screen.
    No,
}

/// Whether the compositor has taken the surface away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gone {
    /// It has, and this program is finished.
    Yes,
    /// The surface is still ours.
    No,
}

/// A surface that is on the screen.
struct Up {
    surface: wl_surface::WlSurface,
    layer: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
}

/// The pixels, and the two objects that let the compositor see them.
struct Frame {
    /// The file the pixels are in, kept because the pool holds it open.
    _file: OwnedFd,
    pool: wl_shm_pool::WlShmPool,
    buffer: wl_buffer::WlBuffer,
    pixels: Mapped,
    wide: u32,
    tall: u32,
}

/// What can go wrong before there is a keyboard to look at.
#[derive(Debug)]
pub enum Missing {
    /// No compositor answered. Started outside a session, or before one.
    Compositor(wayland_client::ConnectError),
    /// A compositor that does not do one of the things a keyboard needs. The
    /// layer shell is the one that matters: without it there is no way to put
    /// a surface at the bottom of the screen without taking the focus.
    Global(&'static str),
    /// The compositor went away while we were talking to it.
    Gone(wayland_client::DispatchError),
    /// No memory to draw into.
    Memory(std::io::Error),
}

impl Screen {
    /// Connect to the compositor, without taking any of the screen yet.
    ///
    /// Separate from [`Screen::show`] because of `--hidden`: the keyboard is
    /// started with the session and stays for it, and for most of that session
    /// there is nothing of it on the screen. A surface made and then destroyed
    /// would be a keyboard that flickers up at login and, worse, a moment in
    /// which everything that watches the compositor's layer list believes the
    /// keyboard is up.
    pub fn connect() -> Result<Screen, Missing> {
        let connection = Connection::connect_to_env().map_err(Missing::Compositor)?;
        let (globals, queue) = registry_queue_init::<Board>(&connection)
            .map_err(|_| Missing::Global("the compositor's list of globals"))?;
        let hand = queue.handle();

        // Versions are ranges rather than numbers because the compositor
        // decides: asking for exactly what this was written against is how a
        // client stops working when the compositor is updated.
        let compositor = globals
            .bind::<wl_compositor::WlCompositor, _, _>(&hand, 1..=6, ())
            .map_err(|_| Missing::Global("wl_compositor"))?;
        let shm = globals
            .bind::<wl_shm::WlShm, _, _>(&hand, 1..=1, ())
            .map_err(|_| Missing::Global("wl_shm"))?;
        let shell = globals
            .bind::<zwlr_layer_shell_v1::ZwlrLayerShellV1, _, _>(&hand, 1..=4, ())
            .map_err(|_| Missing::Global("zwlr_layer_shell_v1, which is what makes a keyboard"))?;
        let seat = globals
            .bind::<wl_seat::WlSeat, _, _>(&hand, 1..=7, ())
            .map_err(|_| Missing::Global("wl_seat"))?;
        // The one protocol that makes this a keyboard rather than a picture of
        // one. A compositor without it can show every key and type nothing.
        let typing = globals
            .bind::<zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1, _, _>(
                &hand,
                1..=1,
                (),
            )
            .map_or_else(
                |_| {
                    eprintln!(
                        "this compositor has no virtual keyboard protocol; \
                         the keyboard will draw and type nothing"
                    );
                    None
                },
                Some,
            );

        let board = Board {
            compositor,
            shm,
            shell,
            seat,
            up: None,
            frame: None,
            size: None,
            scale: 1,
            closed: false,
            typing,
            pokes: Vec::new(),
            pointer_at: (0.0, 0.0),
            pointer_down: false,
        };
        Ok(Screen { connection, queue, board })
    }

    /// Put the keyboard on the screen, `tall` logical units high.
    ///
    /// Blocks until the compositor has said how wide that makes it, because
    /// there is nothing sensible to draw before then and the first thing after
    /// this is drawing.
    pub fn show(&mut self, tall: u32) -> Result<(), Missing> {
        if self.board.up.is_some() {
            return Ok(());
        }

        let hand = self.queue.handle();
        let surface = self.board.compositor.create_surface(&hand, ());
        let layer = self.board.shell.get_layer_surface(
            &surface,
            None,
            zwlr_layer_shell_v1::Layer::Overlay,
            NAMESPACE.to_string(),
            &hand,
            (),
        );
        // Anchored along the bottom and to both sides, and zero wide, which is
        // how the layer shell is told "as wide as the screen": a size of zero
        // on an axis the surface is anchored to on both sides means the
        // compositor picks it, and the compositor is the one that knows.
        layer.set_size(0, tall);
        layer.set_anchor(
            zwlr_layer_surface_v1::Anchor::Bottom
                | zwlr_layer_surface_v1::Anchor::Left
                | zwlr_layer_surface_v1::Anchor::Right,
        );
        // The keys go to whoever had the focus. A keyboard that took it would
        // be typing into itself.
        layer.set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);
        // Push the windows up by exactly the keyboard's height, so what is
        // being typed into is not underneath it.
        layer.set_exclusive_zone(fitted(tall));
        surface.commit();

        self.board.size = None;
        self.board.up = Some(Up { surface, layer });

        // The compositor answers a first commit with a configure, and until
        // that arrives there is no size and nothing may be attached.
        while self.board.size.is_none() && !self.board.closed {
            self.queue.blocking_dispatch(&mut self.board).map_err(Missing::Gone)?;
        }

        Ok(())
    }

    /// Take it off the screen.
    ///
    /// The surface is destroyed rather than emptied, because that is the
    /// question the rest of the desktop asks: `console_door` reads the
    /// compositor's list of layers, and a keyboard that is not on the screen
    /// should not be in it.
    pub fn hide(&mut self) {
        let Some(up) = self.board.up.take() else { return };

        up.layer.destroy();
        up.surface.destroy();
        self.board.frame = None;
        self.board.size = None;
        let _ = self.connection.flush();
    }

    /// Whether the keyboard is on the screen.
    pub fn showing(&self) -> Showing {
        match self.board.up {
            Some(_) => Showing::Yes,
            None => Showing::No,
        }
    }

    /// How big the strip is, in logical units.
    pub fn size(&self) -> Option<(u32, u32)> {
        self.board.size
    }

    /// How many real pixels a logical one is, which the output decides.
    ///
    /// Read by the loop rather than only by `draw`, because it is part of what
    /// a frame is drawn from: a screen that changes scale without changing its
    /// logical size draws every key at the wrong size until something else
    /// asks for a frame.
    pub fn scale(&self) -> i32 {
        self.board.scale
    }

    /// The seat, for whatever reads the touches.
    pub fn seat(&self) -> &wl_seat::WlSeat {
        &self.board.seat
    }

    /// The factory for the virtual keyboard, if this compositor has one.
    pub fn typing(&self) -> Option<&zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1> {
        self.board.typing.as_ref()
    }

    /// Everything the seat has said since this was last asked.
    pub fn pokes(&mut self) -> Vec<Poke> {
        std::mem::take(&mut self.board.pokes)
    }

    /// A handle for making more objects on this queue.
    pub fn hand(&self) -> QueueHandle<Board> {
        self.queue.handle()
    }

    /// Whether the compositor has taken the surface away.
    pub fn closed(&self) -> Gone {
        match self.board.closed {
            true => Gone::Yes,
            false => Gone::No,
        }
    }

    /// Draw a frame, and put it on the screen.
    ///
    /// The pixels are handed to `paint` as the raw bytes of an ARGB image, in
    /// the same memory the compositor reads, so what `paint` writes is what is
    /// shown -- there is no copy anywhere in this.
    pub fn draw(&mut self, paint: impl FnOnce(&mut [u8], u32, u32, i32)) -> Result<(), Missing> {
        let Some((wide, tall)) = self.board.size else { return Ok(()) };

        let Some(up) = self.board.up.as_ref() else { return Ok(()) };

        let hand = self.queue.handle();
        let scale = self.board.scale;
        let across = wide * fitted::<i32, u32>(scale);
        let down = tall * fitted::<i32, u32>(scale);

        // A frame of the wrong size is thrown away rather than reused. The
        // size changes when the screen turns, which is twice a session, and
        // paying an allocation for it beats carrying two of everything.
        let stale = self.board.frame.as_ref().is_none_or(|f| f.wide != across || f.tall != down);

        if stale {
            self.board.frame = Some(Frame::new(&self.board.shm, &hand, across, down)?);
        }

        let Some(frame) = self.board.frame.as_mut() else { return Ok(()) };

        paint(frame.pixels.pixels(), across, down, scale);

        up.surface.attach(Some(&frame.buffer), 0, 0);
        up.surface.set_buffer_scale(scale);
        up.surface.damage_buffer(0, 0, fitted(across), fitted(down));
        up.surface.commit();
        let _ = self.connection.flush();
        Ok(())
    }

    /// Wait for the compositor, or for `also`, to have something to say.
    ///
    /// The keyboard has two things to listen to and no thread to spare: the
    /// compositor's socket, and the signals that show and hide it. Waiting on
    /// one of them means the other is answered whenever the first happens to
    /// speak -- a keyboard that comes up when you next touch the screen it is
    /// not on. So both are waited on at once, which is what `poll` is for, and
    /// wayland-rs hands over its socket for exactly this.
    ///
    /// Returns a bit per entry in `also`, set on the ones that spoke. Bit zero
    /// is `also[0]`. Nothing set means the compositor spoke, or `until` ran
    /// out, which the caller tells apart by asking what it was waiting for.
    ///
    /// `until` is how long it may wait at most. A keyboard nobody is touching
    /// wants to wait forever -- `None` -- and one with a direction held on the
    /// pad wants to wake when the next repeat is due, because a held stick
    /// reports once and then says nothing at all.
    pub fn wait_with(
        &mut self,
        also: &[std::os::fd::RawFd],
        until: Option<std::time::Duration>,
    ) -> Result<u32, Missing> {
        use std::os::fd::AsRawFd;

        self.queue.dispatch_pending(&mut self.board).map_err(Missing::Gone)?;
        let _ = self.connection.flush();

        // Between here and the read, anything that arrives is queued rather
        // than lost: that is the whole point of the guard, and reading the
        // socket without one is the race where a press arrives while the
        // keyboard is deciding to sleep and is answered on the next press.
        let Some(guard) = self.connection.prepare_read() else {
            self.queue.dispatch_pending(&mut self.board).map_err(Missing::Gone)?;
            return Ok(0);
        };

        let socket = self.connection.as_fd().as_raw_fd();
        // One allocation per wakeup, which is once per press: the list is
        // built rather than fixed because how many descriptors there are
        // depends on whether this machine has a pad.
        let mut watch = vec![libc::pollfd { fd: socket, events: libc::POLLIN, revents: 0 }];
        watch.extend(
            also.iter().map(|fd| libc::pollfd { fd: *fd, events: libc::POLLIN, revents: 0 }),
        );
        // Milliseconds, rounded up: rounding down gives a zero timeout, which
        // is a poll that returns at once and a loop that spins.
        let wait = match until {
            None => -1,
            Some(d) => i32::try_from(d.as_millis()).unwrap_or(i32::MAX).max(1),
        };
        // SAFETY: descriptors this process owns, and a count that matches.
        let ready = unsafe { libc::poll(watch.as_mut_ptr(), fitted(watch.len()), wait) };

        if ready < 0 {
            // Interrupted. Not a failure: something was delivered, and the
            // next turn round the loop asks again.
            drop(guard);
            return Ok(0);
        }

        match watch[0].revents & libc::POLLIN != 0 {
            true => {
                let _ = guard.read();
            },
            false => drop(guard),
        }

        self.queue.dispatch_pending(&mut self.board).map_err(Missing::Gone)?;
        let mut spoke = 0u32;

        for (bit, polled) in watch.iter().skip(1).enumerate() {
            if polled.revents & libc::POLLIN != 0 {
                spoke |= 1 << bit;
            }
        }

        Ok(spoke)
    }

    /// Wait for the compositor to say something, and answer it.
    ///
    /// This is the whole of the keyboard's idle: a keyboard nobody is touching
    /// costs nothing, because there is nothing to poll -- a press arrives as an
    /// event on this queue and so does the screen turning.
    pub fn wait(&mut self) -> Result<(), Missing> {
        self.queue.blocking_dispatch(&mut self.board).map(|_| ()).map_err(Missing::Gone)
    }

    /// Answer whatever has already arrived, without waiting for more.
    pub fn catch_up(&mut self) -> Result<(), Missing> {
        self.queue.roundtrip(&mut self.board).map(|_| ()).map_err(Missing::Gone)
    }
}

impl Frame {
    /// Memory for one screenful, and the two objects that show it.
    fn new(
        shm: &wl_shm::WlShm,
        hand: &QueueHandle<Board>,
        wide: u32,
        tall: u32,
    ) -> Result<Frame, Missing> {
        let stride = wide * DEEP;
        let long: usize = fitted(stride * tall);
        let file = drawing_buffer(long).map_err(Missing::Memory)?;
        let pixels = Mapped::of(&file, long).map_err(Missing::Memory)?;
        let pool = shm.create_pool(file.as_fd(), fitted(long), hand, ());
        let buffer = pool.create_buffer(
            0,
            fitted(wide),
            fitted(tall),
            fitted(stride),
            wl_shm::Format::Argb8888,
            hand,
            (),
        );
        Ok(Frame { _file: file, pool, buffer, pixels, wide, tall })
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        self.buffer.destroy();
        self.pool.destroy();
    }
}

/// The registry, which is asked nothing after the globals are bound.
///
/// `registry_queue_init` answers the first round of these itself and hands
/// back the list; what arrives here afterwards is a global appearing or going
/// while the keyboard runs. A second seat or a second output is not something
/// this device has.
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Board {
    fn event(
        _board: &mut Self,
        _registry: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _held: &GlobalListContents,
        _connection: &Connection,
        _hand: &QueueHandle<Self>,
    ) {
    }
}

/// The layer surface, which is the only object here that says anything the
/// keyboard has to act on.
impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for Board {
    fn event(
        board: &mut Self,
        layer: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _held: &(),
        _connection: &Connection,
        _hand: &QueueHandle<Self>,
    ) {
        match event {
            // "This is how big you are." It has to be acknowledged before
            // anything is attached, and the size in it is the answer to the
            // zero width asked for above.
            zwlr_layer_surface_v1::Event::Configure { serial, width, height } => {
                layer.ack_configure(serial);
                board.size = Some((width, height));
            },
            // "You are not on the screen any more." Said when the output goes
            // or the session ends; there is no arguing with it and no
            // reattaching to it.
            zwlr_layer_surface_v1::Event::Closed => {
                board.closed = true;
                board.up = None;
                board.frame = None;
                board.size = None;
            },
            _ => {},
        }
    }
}

/// The surface, whose only events are about which output it is on.
///
/// The scale is taken from there because it is what decides how many real
/// pixels a key is: the panel on this device is 2560 across and the desktop
/// is drawn at 1024, and a keyboard that ignored that would be drawn at two
/// fifths of the size and blurred back up.
impl Dispatch<wl_surface::WlSurface, ()> for Board {
    fn event(
        board: &mut Self,
        _surface: &wl_surface::WlSurface,
        event: wl_surface::Event,
        _held: &(),
        _connection: &Connection,
        _hand: &QueueHandle<Self>,
    ) {
        if let wl_surface::Event::PreferredBufferScale { factor } = event {
            board.scale = factor.max(1);
        }
    }
}

// Everything else says nothing this has to answer: the compositor and the
// shell are factories, the pool is a lease on the memory, and the formats
// wl_shm advertises are a list this does not read -- ARGB8888 is the one
// format every compositor supports and the only one asked for.
delegate_noop!(Board: ignore wl_compositor::WlCompositor);
delegate_noop!(Board: ignore wl_shm::WlShm);
delegate_noop!(Board: ignore wl_shm_pool::WlShmPool);
delegate_noop!(Board: ignore wl_buffer::WlBuffer);
delegate_noop!(Board: ignore zwlr_layer_shell_v1::ZwlrLayerShellV1);
delegate_noop!(Board: ignore zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1);
delegate_noop!(Board: ignore zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1);

/// The seat, which says what there is to touch the keyboard with.
///
/// Both are taken when both are offered. The device is a touchscreen and the
/// laptop this is written on is a mouse, and a keyboard that can only be used
/// on the machine it ships to is a keyboard nobody can try.
impl Dispatch<wl_seat::WlSeat, ()> for Board {
    fn event(
        _board: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _held: &(),
        _connection: &Connection,
        hand: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities { capabilities } = event {
            let has = match capabilities {
                wayland_client::WEnum::Value(has) => has,
                wayland_client::WEnum::Unknown(_) => return,
            };

            if has.contains(wl_seat::Capability::Touch) {
                seat.get_touch(hand, ());
            }

            if has.contains(wl_seat::Capability::Pointer) {
                seat.get_pointer(hand, ());
            }
        }
    }
}

/// A thumb on the glass.
///
/// One finger at a time. A second finger down while the first is still there
/// is ignored rather than tracked, because two fingers on a keyboard this size
/// is a palm, and the C keyboard learned the same thing.
impl Dispatch<wl_touch::WlTouch, ()> for Board {
    fn event(
        board: &mut Self,
        _touch: &wl_touch::WlTouch,
        event: wl_touch::Event,
        _held: &(),
        _connection: &Connection,
        _hand: &QueueHandle<Self>,
    ) {
        match event {
            wl_touch::Event::Down { x, y, .. } => board.pokes.push(Poke::Down { x, y }),
            wl_touch::Event::Motion { x, y, .. } => board.pokes.push(Poke::Moved { x, y }),
            wl_touch::Event::Up { .. } => board.pokes.push(Poke::Up),
            // The compositor took the touch away -- a gesture went to it
            // instead, or the screen turned. Whatever was under the finger is
            // not pressed, so this is a lift and not a press.
            wl_touch::Event::Cancel => board.pokes.push(Poke::Up),
            _ => {},
        }
    }
}

/// A mouse, which is how this is tried on a machine with no touchscreen.
///
/// The button says which button and not where, so where is whatever the last
/// motion said. Motion with nothing held is a cursor crossing the keyboard on
/// its way somewhere, and is not a finger.
impl Dispatch<wl_pointer::WlPointer, ()> for Board {
    fn event(
        board: &mut Self,
        _pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _held: &(),
        _connection: &Connection,
        _hand: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter { surface_x, surface_y, .. } => {
                board.pointer_at = (surface_x, surface_y);
            },
            wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
                board.pointer_at = (surface_x, surface_y);

                if board.pointer_down {
                    board.pokes.push(Poke::Moved { x: surface_x, y: surface_y });
                }
            },
            wl_pointer::Event::Button { state, .. } => {
                let down = matches!(state, wayland_client::WEnum::Value(wl_pointer::ButtonState::Pressed));
                board.pointer_down = down;
                let (x, y) = board.pointer_at;
                board.pokes.push(match down {
                    true => Poke::Down { x, y },
                    false => Poke::Up,
                });
            },
            // A cursor that left with the button down is a press that will
            // never be lifted here, so it is lifted here.
            wl_pointer::Event::Leave { .. } if board.pointer_down => {
                board.pointer_down = false;
                board.pokes.push(Poke::Up);
            },
            _ => {},
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The name on the surface is the name the rest of the desktop looks for.
    /// Nothing about this is checkable without a compositor except the one
    /// thing that has actually been wrong before, which is the word itself.
    #[test]
    fn the_keyboard_publishes_the_name_the_desktop_looks_for() {
        assert_eq!(NAMESPACE, console_controller_keyboard_name());
    }

    /// Read out of the controller's own constant, through the file rather than
    /// through a dependency: this crate is built on the device and the
    /// controller is not one of its dependencies.
    fn console_controller_keyboard_name() -> String {
        let at = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../console-controller/src/mode.rs");
        let held = std::fs::read_to_string(at).expect("the controller's modes");
        held.lines()
            .find_map(|line| {
                let rest = line.trim().strip_prefix("pub const KEYBOARD: &str =")?;
                let (_, quoted) = rest.split_once('"')?;
                let (name, _) = quoted.split_once('"')?;
                Some(name.to_string())
            })
            .expect("KEYBOARD in the controller's modes")
    }
}
