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
//! The layer shell has four of them and this desktop uses two. A card and a
//! panel are `Overlay`, which is over everything including a game somebody has
//! put full screen. The bar is `Top`, which is over the windows and under the
//! overlay -- so the keyboard comes up over it and a panel covers it, which is
//! what happened when the bar was waybar's and is what anybody holding the
//! device already expects. [`Under`] is that choice said as what may cover this
//! surface rather than as a number in somebody else's protocol.
//!
//! ## A closed socket is not quiet
//!
//! `poll` asked only about `POLLIN` returns immediately and forever on a
//! descriptor whose far end has gone, because the hangup is on `POLLHUP` and
//! nothing reads it. That is a program at a whole core drawing on a screen that
//! is not there. It is asked about by name here.

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use std::io;
use std::os::fd::{AsFd, AsRawFd, RawFd};
use std::time::Duration;

use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_pointer, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
    wl_touch,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, delegate_noop};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1, wp_fractional_scale_v1,
};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::memory::Shared;
use crate::scale::Scale;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Top,
    TopRight,
    Bottom,
    Whole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Under {
    Anything,
    Nothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyboard {
    Takes,
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
pub enum Poke {
    Down { at: (f64, f64) },
    Moved { at: (f64, f64) },
    Up,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gone {
    Yes,
    No,
}

#[derive(Debug)]
pub enum Missing {
    Compositor(wayland_client::ConnectError),
    Global(&'static str),
    Went(wayland_client::DispatchError),
    Hung,
    Memory(io::Error),
}

impl std::fmt::Display for Missing {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Missing::Compositor(_) => write!(to, "no compositor answered on WAYLAND_DISPLAY"),
            Missing::Global(what) => write!(to, "the compositor has no {what}"),
            Missing::Went(why) => write!(to, "the compositor went away: {why}"),
            Missing::Hung => write!(to, "the compositor closed the connection"),
            Missing::Memory(why) => write!(to, "no memory for a frame: {why}"),
        }
    }
}

impl std::error::Error for Missing {}

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
}

impl Frame {
    fn new(
        shm: &wl_shm::WlShm,
        hand: &QueueHandle<Bound>,
        size: Size<u32>,
    ) -> Result<Frame, Missing> {
        let stride = size.wide.saturating_mul(4);
        let long = stride.saturating_mul(size.tall);
        let Ok(bytes) = fitted::<u32, usize>(long.max(4));
        let shared = Shared::of(bytes).map_err(Missing::Memory)?;
        let Ok(held) = shared.held();
        let Ok(many) = fitted::<usize, i32>(bytes);
        let pool = shm.create_pool(held.as_fd(), many, hand, ());
        let Ok(wide) = fitted::<u32, i32>(size.wide);
        let Ok(tall) = fitted::<u32, i32>(size.tall);
        let Ok(across) = fitted::<u32, i32>(stride);
        let buffer =
            pool.create_buffer(0, wide, tall, across, wl_shm::Format::Argb8888, hand, ());

        Ok(Frame { _shared: shared, pool, buffer, size })
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
    up: Option<Up>,
    frame: Option<Frame>,
    logical: Option<Size<u32>>,
    asked: Option<Size<u32>>,
    anchor: Anchor,
    room: Room,
    scale: Scale,
    closed: Gone,
    pokes: Vec<Poke>,
    pointer_at: (f64, f64),
}

pub struct Surface {
    connection: Connection,
    queue: EventQueue<Bound>,
    bound: Bound,
}

impl Surface {
    pub fn connect() -> Result<Surface, Missing> {
        let connection = Connection::connect_to_env().map_err(Missing::Compositor)?;
        let (globals, queue) = registry_queue_init::<Bound>(&connection)
            .map_err(|_| Missing::Global("a list of globals"))?;
        let hand = queue.handle();

        let compositor = globals
            .bind::<wl_compositor::WlCompositor, _, _>(&hand, 1..=6, ())
            .map_err(|_| Missing::Global("wl_compositor"))?;
        let shm = globals
            .bind::<wl_shm::WlShm, _, _>(&hand, 1..=1, ())
            .map_err(|_| Missing::Global("wl_shm"))?;
        let shell = globals
            .bind::<zwlr_layer_shell_v1::ZwlrLayerShellV1, _, _>(&hand, 1..=4, ())
            .map_err(|_| Missing::Global("zwlr_layer_shell_v1, which is what makes a surface ours"))?;
        let viewporter = globals
            .bind::<wp_viewporter::WpViewporter, _, _>(&hand, 1..=1, ())
            .map_err(|_| Missing::Global("wp_viewporter, which is the whole of drawing at 2.5"))?;
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
        let _ = globals.bind::<wl_seat::WlSeat, _, _>(&hand, 1..=7, ());

        let bound = Bound {
            compositor,
            shm,
            shell,
            viewporter,
            fractions,
            up: None,
            frame: None,
            logical: None,
            asked: None,
            anchor: Anchor::Whole,
            room: Room::Over,
            scale: Scale::ONE,
            closed: Gone::No,
            pokes: Vec::new(),
            pointer_at: (0.0, 0.0),
        };

        Ok(Surface { connection, queue, bound })
    }

    pub fn show(&mut self, wanted: &Wanted) -> Result<(), Missing> {
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
        layer.set_size(wanted.size.wide, wanted.size.tall);
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
            Keyboard::Declines => zwlr_layer_surface_v1::KeyboardInteractivity::None,
        });
        surface.commit();

        self.bound.logical = None;
        self.bound.asked = Some(wanted.size);
        self.bound.anchor = wanted.anchor;
        self.bound.room = wanted.room;
        self.bound.up = Some(Up { surface, layer, viewport, fraction });

        while self.bound.logical.is_none() && self.bound.closed == Gone::No {
            self.queue.blocking_dispatch(&mut self.bound).map_err(Missing::Went)?;
        }

        Ok(())
    }

    pub fn resize(&mut self, size: Size<u32>) -> Result<(), Never> {
        let up = match self.bound.up.as_ref() {
            Some(up) => up,
            None => return Ok(()),
        };

        match self.bound.asked {
            Some(asked) if asked == size => return Ok(()),
            Some(_) | None => {},
        }

        up.layer.set_size(size.wide, size.tall);
        up.surface.commit();
        let _ = self.connection.flush();

        self.bound.asked = Some(size);

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
        self.bound.frame = None;
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

    pub fn closed(&self) -> Result<Gone, Never> {
        Ok(self.bound.closed)
    }

    pub fn pokes(&mut self) -> Result<Vec<Poke>, Never> {
        Ok(std::mem::take(&mut self.bound.pokes))
    }

    pub fn draw(
        &mut self,
        paint: impl FnOnce(&mut [u8], Size<u32>, Scale) -> Result<(), Never>,
    ) -> Result<(), Missing> {
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

        let stale = self.bound.frame.as_ref().is_none_or(|frame| frame.size != device);

        match stale {
            true => {
                let frame = Frame::new(&self.bound.shm, &hand, device)?;

                self.bound.frame = Some(frame);
            }
            false => {},
        }

        let frame = match self.bound.frame.as_mut() {
            Some(frame) => frame,
            None => return Ok(()),
        };
        let Ok(pixels) = frame._shared.pixels();

        let Ok(()) = paint(pixels, device, scale);

        let Ok(wide) = fitted::<u32, i32>(device.wide);
        let Ok(tall) = fitted::<u32, i32>(device.tall);
        let Ok(across) = fitted::<u32, i32>(logical.wide);
        let Ok(down) = fitted::<u32, i32>(logical.tall);

        up.surface.attach(Some(&frame.buffer), 0, 0);
        up.surface.set_buffer_scale(1);
        up.viewport.set_destination(across, down);
        up.surface.damage_buffer(0, 0, wide, tall);
        up.surface.commit();
        let _ = self.connection.flush();

        Ok(())
    }

    pub fn wait(&mut self, also: &[RawFd], until: Option<Duration>) -> Result<(), Missing> {
        self.queue.dispatch_pending(&mut self.bound).map_err(Missing::Went)?;
        let _ = self.connection.flush();

        let guard = match self.connection.prepare_read() {
            Some(guard) => guard,
            None => {
                self.queue.dispatch_pending(&mut self.bound).map_err(Missing::Went)?;

                return Ok(());
            }
        };

        let socket = self.connection.as_fd().as_raw_fd();
        let mut watch = vec![libc::pollfd { fd: socket, events: libc::POLLIN, revents: 0 }];

        watch.extend(also.iter().map(|fd| libc::pollfd {
            fd: *fd,
            events: libc::POLLIN,
            revents: 0,
        }));

        let wait = match until {
            None => -1,
            Some(long) => {
                let Ok(many) = fitted::<u128, i32>(long.as_millis());

                many.max(1)
            }
        };
        let Ok(many) = fitted::<usize, libc::nfds_t>(watch.len());

        // SAFETY: descriptors this process owns, and a count that matches.
        let ready = unsafe { libc::poll(watch.as_mut_ptr(), many, wait) };

        match ready < 0 {
            true => {
                drop(guard);

                return Ok(());
            }
            false => {},
        }

        let hung = watch
            .first()
            .is_some_and(|first| first.revents & (libc::POLLHUP | libc::POLLERR) != 0);

        match hung {
            true => {
                drop(guard);
                self.bound.closed = Gone::Yes;

                return Err(Missing::Hung);
            }
            false => {},
        }

        let readable = watch.first().is_some_and(|first| first.revents & libc::POLLIN != 0);

        match readable {
            true => match guard.read() {
                Ok(_) => {}
                Err(wayland_client::backend::WaylandError::Io(why))
                    if why.kind() == io::ErrorKind::WouldBlock => {}
                Err(_) => {
                    self.bound.closed = Gone::Yes;

                    return Err(Missing::Hung);
                }
            },
            false => drop(guard),
        }

        self.queue.dispatch_pending(&mut self.bound).map_err(Missing::Went)?;

        Ok(())
    }
}

fn reserved(room: Room, anchor: Anchor, logical: Size<u32>) -> Result<i32, Never> {
    let along = match anchor {
        Anchor::Top | Anchor::Bottom => logical.tall,
        Anchor::TopRight | Anchor::Whole => logical.wide,
    };
    let Ok(many) = fitted::<u32, i32>(along);

    Ok(match room {
        Room::Reserves => many,
        Room::Over => -1,
    })
}

fn among(under: Under) -> Result<zwlr_layer_shell_v1::Layer, Never> {
    Ok(match under {
        Under::Anything => zwlr_layer_shell_v1::Layer::Top,
        Under::Nothing => zwlr_layer_shell_v1::Layer::Overlay,
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
    use super::*;

    #[test]
    fn a_bar_along_the_top_reserves_how_deep_it_is_and_not_how_wide() {
        let Ok(zone) = reserved(Room::Reserves, Anchor::Top, Size { wide: 1024, tall: 38 });

        assert_eq!(zone, 38);
    }

    #[test]
    fn a_surface_that_takes_no_room_reserves_nothing_whichever_edge_it_is_on() {
        for anchor in [Anchor::Top, Anchor::TopRight, Anchor::Bottom, Anchor::Whole] {
            let Ok(zone) = reserved(Room::Over, anchor, Size { wide: 1024, tall: 38 });

            assert_eq!(zone, -1, "{anchor:?}");
        }
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
        let Ok(card) = among(Under::Nothing);
        let Ok(bar) = among(Under::Anything);

        assert_eq!(card, zwlr_layer_shell_v1::Layer::Overlay);
        assert_eq!(bar, zwlr_layer_shell_v1::Layer::Top);
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

                let logical = Size { wide: width.max(1), tall: height.max(1) };
                let Ok(zone) = reserved(bound.room, bound.anchor, logical);

                layer.set_exclusive_zone(zone);

                bound.logical = Some(logical);
            }
            zwlr_layer_surface_v1::Event::Closed => bound.closed = Gone::Yes,
            _ => {}
        }
    }
}

impl Dispatch<wp_fractional_scale_v1::WpFractionalScaleV1, ()> for Bound {
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
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for Bound {
    fn event(
        bound: &mut Self,
        _pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _held: &(),
        _connection: &Connection,
        _hand: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter { surface_x, surface_y, .. }
            | wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
                bound.pointer_at = (surface_x, surface_y);

                bound.pokes.push(Poke::Moved { at: bound.pointer_at });
            }
            wl_pointer::Event::Button { state, .. } => match state {
                wayland_client::WEnum::Value(wl_pointer::ButtonState::Pressed) => {
                    bound.pokes.push(Poke::Down { at: bound.pointer_at });
                }
                wayland_client::WEnum::Value(wl_pointer::ButtonState::Released)
                | wayland_client::WEnum::Unknown(_)
                | wayland_client::WEnum::Value(_) => bound.pokes.push(Poke::Up),
            },
            _ => {}
        }
    }
}

impl Dispatch<wl_touch::WlTouch, ()> for Bound {
    fn event(
        bound: &mut Self,
        _touch: &wl_touch::WlTouch,
        event: wl_touch::Event,
        _held: &(),
        _connection: &Connection,
        _hand: &QueueHandle<Self>,
    ) {
        match event {
            wl_touch::Event::Down { x, y, .. } => bound.pokes.push(Poke::Down { at: (x, y) }),
            wl_touch::Event::Motion { x, y, .. } => bound.pokes.push(Poke::Moved { at: (x, y) }),
            wl_touch::Event::Up { .. } => bound.pokes.push(Poke::Up),
            _ => {}
        }
    }
}

delegate_noop!(Bound: ignore wl_compositor::WlCompositor);
delegate_noop!(Bound: ignore wl_surface::WlSurface);
delegate_noop!(Bound: ignore wl_shm::WlShm);
delegate_noop!(Bound: ignore wl_shm_pool::WlShmPool);
delegate_noop!(Bound: ignore wl_buffer::WlBuffer);
delegate_noop!(Bound: ignore zwlr_layer_shell_v1::ZwlrLayerShellV1);
delegate_noop!(Bound: ignore wp_viewporter::WpViewporter);
delegate_noop!(Bound: ignore wp_viewport::WpViewport);
delegate_noop!(Bound: ignore wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1);
