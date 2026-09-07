//! Put the pointer somewhere on the screen, at a place on the picture.
//!
//!     console-point [--in NAMESPACE] ACROSS DOWN [--click] [--scroll NOTCHES]
//!  Where ACROSS and DOWN are a place on the picture, in the size the desktop
//! is laid out in -- the numbers `hyprctl layers` answers in and the numbers
//! anything drawing a surface thinks in. How big that is, is asked of the
//! compositor rather than read out of the tree: the density is a setting
//! somebody can change while the desktop is running, and a place measured
//! against the size the screen used to be lands somewhere else entirely.
//! `--in` says the place is inside a surface instead, measured from its corner:
//! `--in settings-panel 40 60` is forty across and sixty down from wherever the
//! compositor has put that panel. The corner is asked for here, in the session
//! being pointed at, at the moment of the press -- which is the whole reason it
//! is a word on this command line rather than arithmetic in a check. A check
//! that measured the corner first would be pointing at where the panel was, and
//! a check that guessed it would be pressing a place nobody drew. Both of those
//! go green just as readily as the right answer does.  The two questions are
//! asked of the same walk `console_onscreen` already makes for "is this up", so
//! a surface this can be measured against is exactly a surface the rest of the
//! desktop agrees is on the screen.  It asks the compositor for a pointer of
//! its own rather than making one out of uinput, and `console_input_pointer`
//! says why. What that buys is the nested desktop: the emulator is a compositor
//! inside a window on this machine, a uinput mouse would move the laptop's
//! pointer and never the one in the picture, and a pointer asked for over the
//! socket is inside the picture where the checks are looking. So the same
//! command presses the device and presses the emulator, and a check that could
//! only be written for one of them can now be written for both.  The pointer
//! stays where it is put. The virtual pointer goes when this program does, and
//! the compositor keeps the cursor where the last motion left it, so a
//! screenshot taken seconds later still has the pointer standing on whatever
//! this stood it on.

use std::time::{Duration, Instant};

use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_input_pointer::{
    Does, Measured, Where, approach, asked, from_the_corner, on_the_screen,
};
use wayland_client::protocol::wl_pointer::{Axis, AxisSource, ButtonState};
use wayland_client::protocol::{wl_output, wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::xdg_output::zv1::client::{
    zxdg_output_manager_v1::ZxdgOutputManagerV1, zxdg_output_v1, zxdg_output_v1::ZxdgOutputV1,
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
    zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
};

const BTN_LEFT: u32 = 0x110;

const NOTCH: f64 = 15.0;

const STEP: Duration = Duration::from_millis(60);

const PRESSED: Duration = Duration::from_millis(80);

const HELD: Duration = Duration::from_millis(400);

fn main() {
    match pointed() {
        Ok(()) => {},
        Err(fault) => {
            eprintln!("console-point: {fault}");
            std::process::exit(1);
        },
    }
}

fn pointed() -> Result<(), String> {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let asked = asked(&words)?;
    let at = match &asked.measured {
        Measured::FromTheScreen => asked.at,
        Measured::FromTheCorner(namespace) => in_the_surface(asked.at, namespace)?,
    };

    let pointer = Pointer::new()?;
    let room = pointer.room;
    let Ok(standing) = on_the_screen(at, room);

    match standing {
        Where::OnTheScreen => {},
        Where::OffIt => {
            return Err(format!("({}, {}) is not on a {}x{} screen", at.0, at.1, room.0, room.1));
        },
    }

    let Ok(from) = approach(at, room);

    pointer.to(from)?;
    #[cfg_attr(
        dylint_lib = "explicit021_no_sleeping",
        allow(
            explicit021_no_sleeping,
            reason = "a warp sends no motion: the pointer is put down beside the target and moved onto it a frame later, and the gap between the two is what makes it a movement the compositor reports"
        )
    )]
    std::thread::sleep(STEP);
    pointer.to(at)?;

    match asked.does {
        Does::Nothing => {},
        Does::Click => pointer.click()?,
        Does::Scroll(notches) => pointer.scroll(notches)?,
    }

    #[cfg_attr(
        dylint_lib = "explicit021_no_sleeping",
        allow(
            explicit021_no_sleeping,
            reason = "the virtual device is destroyed when this process ends, and an event written into a device that is gone before the compositor read it never happened"
        )
    )]
    std::thread::sleep(HELD);

    Ok(())
}

fn in_the_surface(at: (u32, u32), namespace: &str) -> Result<(u32, u32), String> {
    let screens = console_onscreen::screens()?;
    let Ok(drawn) = console_onscreen::standing(&screens, namespace);

    let standing = drawn.ok_or(format!(
        "nothing called {namespace} is on the screen, so there is no corner to measure from"
    ))?;

    let Ok(wide) = fitted::<i64, u32>(standing.wide);
    let Ok(tall) = fitted::<i64, u32>(standing.tall);
    let Ok(inside) = on_the_screen(at, (wide, tall));

    match inside {
        Where::OnTheScreen => {},
        Where::OffIt => {
            return Err(format!(
                "({}, {}) is past the {wide}x{tall} of {namespace}",
                at.0, at.1
            ));
        },
    }

    from_the_corner(at, (standing.across, standing.down))
}

#[derive(Default)]
struct Found {
    manager: Option<ZwlrVirtualPointerManagerV1>,
    sizes: Option<ZxdgOutputManagerV1>,
    seat: Option<wl_seat::WlSeat>,
    screen: Option<wl_output::WlOutput>,
    room: Option<(u32, u32)>,
}

struct Pointer {
    connection: Connection,
    said: ZwlrVirtualPointerV1,
    room: (u32, u32),
    since: Instant,
}

impl Pointer {
    fn new() -> Result<Pointer, String> {
        let connection = Connection::connect_to_env()
            .map_err(|fault| format!("no compositor to point at: {fault}"))?;
        let mut queue = connection.new_event_queue();
        let handle = queue.handle();
        let _registry = connection.display().get_registry(&handle, ());

        let mut found = Found::default();
        queue
            .roundtrip(&mut found)
            .map_err(|fault| format!("the compositor said nothing about itself: {fault}"))?;

        let asking = (found.sizes.as_ref(), found.screen.as_ref());

        match asking {
            (Some(sizes), Some(screen)) => {
                let _ = sizes.get_xdg_output(screen, &handle, ());
            },
            (Some(_), None) | (None, _) => {},
        }

        let asked = queue.roundtrip(&mut found);

        match asked {
            Ok(_answered) => {},
            Err(fault) => eprintln!("console-point: the screen said nothing of its size: {fault}"),
        }

        let room = match found.room {
            Some(room) => room,
            None => {
                let screen = console_screen::declared()?;
                let Ok(logical) = screen.logical();

                logical
            },
        };

        let manager = found.manager.ok_or(
            "this compositor publishes no zwlr_virtual_pointer_manager_v1, \
             so nothing but a hand can move its pointer",
        )?;
        let said = manager.create_virtual_pointer_with_output(
            found.seat.as_ref(),
            found.screen.as_ref(),
            &handle,
            (),
        );

        connection
            .flush()
            .map_err(|fault| format!("the pointer could not be asked for: {fault}"))?;

        Ok(Pointer { connection, said, room, since: Instant::now() })
    }

    fn now(&self) -> Result<u32, Never> {
        fitted(self.since.elapsed().as_millis())
    }

    fn flush(&self) -> Result<(), String> {
        self.connection.flush().map_err(|fault| format!("writing to the compositor: {fault}"))
    }

    fn to(&self, at: (u32, u32)) -> Result<(), String> {
        let Ok(now) = self.now();

        self.said.motion_absolute(now, at.0, at.1, self.room.0, self.room.1);
        self.said.frame();

        self.flush()
    }

    fn click(&self) -> Result<(), String> {
        let Ok(pressed) = self.now();

        self.said.button(pressed, BTN_LEFT, ButtonState::Pressed);
        self.said.frame();
        self.flush()?;

        #[cfg_attr(
            dylint_lib = "explicit021_no_sleeping",
            allow(
                explicit021_no_sleeping,
                reason = "a click is a press and a release with a person-shaped gap between them; released in the same frame it is a press nothing downstream counts"
            )
        )]
        std::thread::sleep(PRESSED);

        let Ok(released) = self.now();

        self.said.button(released, BTN_LEFT, ButtonState::Released);
        self.said.frame();

        self.flush()
    }

    fn scroll(&self, notches: i32) -> Result<(), String> {
        let far = f64::from(notches) * NOTCH;
        let Ok(now) = self.now();

        self.said.axis_source(AxisSource::Wheel);
        self.said.axis_discrete(now, Axis::VerticalScroll, far, notches);
        self.said.frame();

        self.flush()
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Found {
    fn event(
        found: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global { name, interface, version } = event else {
            return;
        };

        match interface.as_str() {
            "zwlr_virtual_pointer_manager_v1" => {
                found.manager = Some(registry.bind(name, version.min(2), handle, ()));
            },
            "zxdg_output_manager_v1" => {
                found.sizes = Some(registry.bind(name, version.min(3), handle, ()));
            },
            "wl_seat" => found.seat = Some(registry.bind(name, version.min(5), handle, ())),
            "wl_output" if found.screen.is_none() => {
                found.screen = Some(registry.bind(name, version.min(3), handle, ()));
            },
            _ => {},
        }
    }
}

impl Dispatch<ZxdgOutputV1, ()> for Found {
    fn event(
        found: &mut Self,
        _: &ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let zxdg_output_v1::Event::LogicalSize { width, height } = event else {
            return;
        };

        let Ok(wide) = fitted(width);
        let Ok(tall) = fitted(height);

        found.room = Some((wide, tall));
    }
}

macro_rules! says_nothing {
    ($($what:ty),*) => {
        $(impl Dispatch<$what, ()> for Found {
            fn event(
                _: &mut Self,
                _: &$what,
                _: <$what as wayland_client::Proxy>::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
            }
        })*
    };
}

says_nothing!(
    ZwlrVirtualPointerManagerV1,
    ZwlrVirtualPointerV1,
    ZxdgOutputManagerV1,
    wl_seat::WlSeat,
    wl_output::WlOutput
);
