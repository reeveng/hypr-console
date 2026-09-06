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
//! `console_onscreen::is_open` asks the compositor for its list of layer surfaces
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
//!
//! ## A closed socket is not quiet
//!
//! [`Screen::wait_with`] asks the display descriptor about `POLLIN` and, for a
//! long time, about nothing else. A compositor that goes away does not leave
//! that poll waiting: it closes the socket, `POLLHUP` comes up on its own, and
//! a poll asked only about `POLLIN` returns at once with nothing to read and no
//! error to raise. `dispatch_pending` finds an empty queue and agrees, and the
//! next turn puts the same question to the same dead descriptor. What that
//! looks like is a keyboard at a whole core forever, drawing on a screen that
//! is not there -- thirty of them at once after an afternoon of nested
//! desktops, each outliving the compositor it was made for, because nothing in
//! the loop ever decided they had ended.
//!
//! So the hangup is asked about by name and it is [`Missing::Hung`], which
//! leaves the loop the way every other missing thing does. The read is no
//! longer discarded either: a socket that will not read is the same fault
//! arriving by another door, and throwing its error away is how it stayed
//! invisible for an afternoon.
//!
//! All of it except `WouldBlock`, which is not that fault and is not rare. A
//! descriptor can be ready to read and have nothing left on it by the time the
//! read happens -- the queue was drained by the dispatch a few lines down on
//! the turn before, and `POLLIN` says only that it was ready when `poll`
//! looked. The discarded read absorbed that for as long as it existed. Made
//! fatal along with everything else, it ended the on-screen keyboard about two
//! seconds after every time it was opened, on the one machine where that
//! keyboard is the only way to type. It is the ordinary answer to a question
//! asked slightly too late, and the loop goes round again.


use console_never::Never;
use console_number_conversion::fitted;
use std::os::fd::{AsFd, OwnedFd};

use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_pointer, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
    wl_touch,
};
use wayland_client::backend::WaylandError;
use wayland_client::{Connection, Dispatch, DispatchError, EventQueue, QueueHandle, delegate_noop};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1, zwp_virtual_keyboard_v1,
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::shared_memory::{Mapped, drawing_buffer};

pub const NAMESPACE: &str = "virtual-keyboard";

const DEEP: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Poke {
    Down { x: f64, y: f64 },
    Moved { x: f64, y: f64 },
    Up,
}

pub struct Screen {
    connection: Connection,
    queue: EventQueue<Board>,
    board: Board,
}

pub struct Board {
    compositor: wl_compositor::WlCompositor,
    shm: wl_shm::WlShm,
    shell: zwlr_layer_shell_v1::ZwlrLayerShellV1,
    pub seat: wl_seat::WlSeat,
    up: Option<Up>,
    frame: Option<Frame>,
    pub size: Option<(u32, u32)>,
    pub scale: i32,
    pub closed: bool,
    pub typing: Option<zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1>,
    pub pokes: Vec<Poke>,
    pointer_at: (f64, f64),
    pointer_down: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Showing {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gone {
    Yes,
    No,
}

struct Up {
    surface: wl_surface::WlSurface,
    layer: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
}

struct Frame {
    _file: OwnedFd,
    pool: wl_shm_pool::WlShmPool,
    buffer: wl_buffer::WlBuffer,
    pixels: Mapped,
    wide: u32,
    tall: u32,
}

#[derive(Debug)]
pub enum Missing {
    Compositor(wayland_client::ConnectError),
    Global(&'static str),
    Gone(wayland_client::DispatchError),
    Hung,
    Memory(std::io::Error),
}

impl Screen {
    pub fn connect() -> Result<Screen, Missing> {
        let connection = Connection::connect_to_env().map_err(Missing::Compositor)?;
        let (globals, queue) = registry_queue_init::<Board>(&connection)
            .map_err(|_| Missing::Global("the compositor's list of globals"))?;
        let hand = queue.handle();

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

    pub fn show(&mut self, tall: u32) -> Result<(), Missing> {
        match self.board.up.is_some() {
            true => return Ok(()),
            false => {},
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
        layer.set_size(0, tall);
        layer.set_anchor(
            zwlr_layer_surface_v1::Anchor::Bottom
                | zwlr_layer_surface_v1::Anchor::Left
                | zwlr_layer_surface_v1::Anchor::Right,
        );
        layer.set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);
        let Ok(zone) = fitted(tall);

        layer.set_exclusive_zone(zone);
        surface.commit();

        self.board.size = None;
        self.board.up = Some(Up { surface, layer });

        while self.board.size.is_none() && !self.board.closed {
            self.queue.blocking_dispatch(&mut self.board).map_err(Missing::Gone)?;
        }

        Ok(())
    }

    pub fn hide(&mut self) -> Result<(), Never> {
        let Some(up) = self.board.up.take() else { return Ok(()) };

        up.layer.destroy();
        up.surface.destroy();
        self.board.frame = None;
        self.board.size = None;
        let _ = self.connection.flush();

        Ok(())
    }

    pub fn showing(&self) -> Result<Showing, Never> {
        Ok(match self.board.up {
            Some(_) => Showing::Yes,
            None => Showing::No,
        })
    }

    pub fn size(&self) -> Result<Option<(u32, u32)>, Never> {
        Ok(self.board.size)
    }

    pub fn scale(&self) -> Result<i32, Never> {
        Ok(self.board.scale)
    }

    pub fn seat(&self) -> Result<&wl_seat::WlSeat, Never> {
        Ok(&self.board.seat)
    }

    pub fn typing(&self) -> Result<Option<&zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1>, Never> {
        Ok(self.board.typing.as_ref())
    }

    pub fn pokes(&mut self) -> Result<Vec<Poke>, Never> {
        Ok(std::mem::take(&mut self.board.pokes))
    }

    pub fn hand(&self) -> Result<QueueHandle<Board>, Never> {
        Ok(self.queue.handle())
    }

    pub fn closed(&self) -> Result<Gone, Never> {
        Ok(match self.board.closed {
            true => Gone::Yes,
            false => Gone::No,
        })
    }

    pub fn draw(&mut self, paint: impl FnOnce(&mut [u8], u32, u32, i32)) -> Result<(), Missing> {
        let Some((wide, tall)) = self.board.size else { return Ok(()) };

        let Some(up) = self.board.up.as_ref() else { return Ok(()) };

        let hand = self.queue.handle();
        let scale = self.board.scale;
        let Ok(many) = fitted::<i32, u32>(scale);

        let across = wide.saturating_mul(many);
        let down = tall.saturating_mul(many);

        let stale = self.board.frame.as_ref().is_none_or(|f| f.wide != across || f.tall != down);

        match stale {
            true => {
                let frame = Frame::new(&self.board.shm, &hand, across, down)?;

                self.board.frame = Some(frame);
            }
            false => {},
        }

        let Some(frame) = self.board.frame.as_mut() else { return Ok(()) };

        let Ok(pixels) = frame.pixels.pixels();

        paint(pixels, across, down, scale);

        up.surface.attach(Some(&frame.buffer), 0, 0);
        up.surface.set_buffer_scale(scale);
        let Ok(damaged_across) = fitted(across);
        let Ok(damaged_down) = fitted(down);

        up.surface.damage_buffer(0, 0, damaged_across, damaged_down);
        up.surface.commit();
        let _ = self.connection.flush();
        Ok(())
    }

    pub fn wait_with(
        &mut self,
        also: &[std::os::fd::RawFd],
        until: Option<std::time::Duration>,
    ) -> Result<u32, Missing> {
        use std::os::fd::AsRawFd;

        self.queue.dispatch_pending(&mut self.board).map_err(Missing::Gone)?;
        let _ = self.connection.flush();

        let Some(guard) = self.connection.prepare_read() else {
            self.queue.dispatch_pending(&mut self.board).map_err(Missing::Gone)?;
            return Ok(0);
        };

        let socket = self.connection.as_fd().as_raw_fd();
        let mut watch = vec![libc::pollfd { fd: socket, events: libc::POLLIN, revents: 0 }];
        watch.extend(
            also.iter().map(|fd| libc::pollfd { fd: *fd, events: libc::POLLIN, revents: 0 }),
        );
        let wait = match until {
            None => -1,
            Some(d) => i32::try_from(d.as_millis()).unwrap_or(i32::MAX).max(1),
        };
        let Ok(many) = fitted(watch.len());

        // SAFETY: descriptors this process owns, and a count that matches.
        let ready = unsafe { libc::poll(watch.as_mut_ptr(), many, wait) };

        match ready < 0 {
            true => {
                drop(guard);
                return Ok(0);
            }
            false => {},
        }

        let Some(answer) = watch.first() else {
            drop(guard);
            return Ok(0);
        };

        match answer.revents & libc::POLLIN != 0 {
            true => {
                let read = guard.read();

                match read {
                    Ok(_) => {},
                    Err(WaylandError::Io(why))
                        if why.kind() == std::io::ErrorKind::WouldBlock => {},
                    Err(why) => return Err(Missing::Gone(DispatchError::Backend(why))),
                }
            },
            false => drop(guard),
        }

        self.queue.dispatch_pending(&mut self.board).map_err(Missing::Gone)?;

        match answer.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
            true => return Err(Missing::Hung),
            false => {},
        }

        let mut spoke = 0u32;

        for (bit, polled) in watch.iter().skip(1).enumerate() {
            match polled.revents & libc::POLLIN != 0 {
                true => {
                    let Ok(bit) = fitted::<usize, u32>(bit);

                    spoke |= 1u32.wrapping_shl(bit);
                }
                false => {},
            }
        }

        Ok(spoke)
    }

    pub fn wait(&mut self) -> Result<(), Missing> {
        self.queue.blocking_dispatch(&mut self.board).map(|_| ()).map_err(Missing::Gone)
    }

    pub fn catch_up(&mut self) -> Result<(), Missing> {
        self.queue.roundtrip(&mut self.board).map(|_| ()).map_err(Missing::Gone)
    }
}

impl Frame {
    fn new(
        shm: &wl_shm::WlShm,
        hand: &QueueHandle<Board>,
        wide: u32,
        tall: u32,
    ) -> Result<Frame, Missing> {
        let stride = wide.saturating_mul(DEEP);
        let Ok(long) = fitted::<u32, usize>(stride.saturating_mul(tall));
        let file = drawing_buffer(long).map_err(Missing::Memory)?;
        let pixels = Mapped::of(&file, long).map_err(Missing::Memory)?;

        let Ok(room) = fitted(long);
        let Ok(across) = fitted(wide);
        let Ok(down) = fitted(tall);
        let Ok(along) = fitted(stride);

        let pool = shm.create_pool(file.as_fd(), room, hand, ());
        let buffer = pool.create_buffer(
            0,
            across,
            down,
            along,
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
            zwlr_layer_surface_v1::Event::Configure { serial, width, height } => {
                layer.ack_configure(serial);
                board.size = Some((width, height));
            },
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

impl Dispatch<wl_surface::WlSurface, ()> for Board {
    fn event(
        board: &mut Self,
        _surface: &wl_surface::WlSurface,
        event: wl_surface::Event,
        _held: &(),
        _connection: &Connection,
        _hand: &QueueHandle<Self>,
    ) {
        match event {
            wl_surface::Event::PreferredBufferScale { factor } => board.scale = factor.max(1),
            _ => {},
        }
    }
}

delegate_noop!(Board: ignore wl_compositor::WlCompositor);
delegate_noop!(Board: ignore wl_shm::WlShm);
delegate_noop!(Board: ignore wl_shm_pool::WlShmPool);
delegate_noop!(Board: ignore wl_buffer::WlBuffer);
delegate_noop!(Board: ignore zwlr_layer_shell_v1::ZwlrLayerShellV1);
delegate_noop!(Board: ignore zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1);
delegate_noop!(Board: ignore zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1);

impl Dispatch<wl_seat::WlSeat, ()> for Board {
    fn event(
        _board: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _held: &(),
        _connection: &Connection,
        hand: &QueueHandle<Self>,
    ) {
        match event {
            wl_seat::Event::Capabilities { capabilities } => {
                let has = match capabilities {
                    wayland_client::WEnum::Value(has) => has,
                    wayland_client::WEnum::Unknown(_) => return,
                };

                match has.contains(wl_seat::Capability::Touch) {
                    true => {
                        seat.get_touch(hand, ());
                    }
                    false => {
                        {};
                    }
                }

                match has.contains(wl_seat::Capability::Pointer) {
                    true => {
                        seat.get_pointer(hand, ());
                    }
                    false => {
                        {};
                    }
                }
            }
            _ => {},
        }
    }
}

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
            wl_touch::Event::Cancel => board.pokes.push(Poke::Up),
            _ => {},
        }
    }
}

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

                match board.pointer_down {
                    true => {
                        board.pokes.push(Poke::Moved { x: surface_x, y: surface_y });
                    }
                    false => {},
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

    #[test]
    fn the_keyboard_publishes_the_name_the_desktop_looks_for() {
        assert_eq!(NAMESPACE, the_desktops_keyboard_name());
    }

    fn the_desktops_keyboard_name() -> String {
        let at = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../console-onscreen/src/lib.rs");
        let held = std::fs::read_to_string(at).expect("what is on the screen");
        held.lines()
            .find_map(|line| {
                let rest = line.trim().strip_prefix("pub const KEYBOARD: &str =")?;
                let (_, quoted) = rest.split_once('"')?;
                let (name, _) = quoted.split_once('"')?;
                Some(name.to_string())
            })
            .expect("KEYBOARD in what is on the screen")
    }
}
