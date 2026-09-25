//! Put the pointer somewhere on the screen, at a place on the picture.
//!
//!     console-point [--in NAMESPACE] ACROSS DOWN [--click] [--scroll NOTCHES] [--drag ACROSS DOWN ...]
//!  Where ACROSS and DOWN are a place on the picture, in the size the desktop
//! is laid out in -- the numbers `hyprctl layers` answers in and the numbers
//! anything drawing a surface thinks in. How big that is, is asked of the
//! compositor rather than read out of the tree: the density is a setting
//! someone can change while the desktop is running, and a place measured
//! against the size the screen used to be lands somewhere else entirely.
//! `--in` says the place is inside a surface instead, measured from its corner:
//! `--in settings-panel 40 60` is forty across and sixty down from wherever the
//! compositor has put that panel. The corner is asked for here, in the session
//! being pointed at, at the moment of the press -- which is the whole reason it
//! is a word on this command line rather than arithmetic in a check. A check
//! that measured the corner first would be pointing at where the panel was, and
//! a check that guessed it would be pressing a place no one drew. Both of those
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

use std::process::ExitCode;
use std::time::{Duration, Instant};

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_input_pointer::{
    PointerAction, Measured, Unsaid, Where, approach, asked, from_the_corner, on_the_screen,
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

fn main() -> ExitCode {
    match pointed() {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("console-point: {fault}");

            ExitCode::FAILURE
        },
    }
}

#[derive(Debug)]
enum Unpointed {
    Unsaid(Unsaid),
    Undeclared(console_screen::Undeclared),
    Onscreen(console_onscreen::Error),
    NotUp(String),
    PastTheSurface(Point<u32>, Size<u32>, String),
    OffTheScreen(Point<u32>, Size<u32>),
    NoCompositor(wayland_client::ConnectError),
    Unsized(wayland_client::DispatchError),
    NoManager,
    Unasked(wayland_client::backend::WaylandError),
    Unwritten(wayland_client::backend::WaylandError),
}

impl std::fmt::Display for Unpointed {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unpointed::Unsaid(fault) => write!(to, "{fault}"),
            Unpointed::Undeclared(fault) => write!(to, "{fault}"),
            Unpointed::Onscreen(fault) => write!(to, "{fault}"),
            Unpointed::NotUp(namespace) => write!(
                to,
                "nothing called {namespace} is on the screen, so there is no corner to measure from"
            ),
            Unpointed::PastTheSurface(at, room, namespace) => write!(
                to,
                "({}, {}) is past the {}x{} of {namespace}",
                at.x, at.y, room.width, room.height
            ),
            Unpointed::OffTheScreen(at, room) => write!(
                to,
                "({}, {}) is not on a {}x{} screen",
                at.x, at.y, room.width, room.height
            ),
            Unpointed::NoCompositor(fault) => write!(to, "no compositor to point at: {fault}"),
            Unpointed::Unsized(fault) => {
                write!(to, "the compositor said nothing about itself: {fault}")
            }
            Unpointed::NoManager => write!(
                to,
                "this compositor publishes no zwlr_virtual_pointer_manager_v1, \
                 so nothing but a hand can move its pointer"
            ),
            Unpointed::Unasked(fault) => write!(to, "the pointer could not be asked for: {fault}"),
            Unpointed::Unwritten(fault) => write!(to, "writing to the compositor: {fault}"),
        }
    }
}

impl std::error::Error for Unpointed {}

impl From<Unsaid> for Unpointed {
    fn from(fault: Unsaid) -> Self {
        Unpointed::Unsaid(fault)
    }
}

impl From<console_screen::Undeclared> for Unpointed {
    fn from(fault: console_screen::Undeclared) -> Self {
        Unpointed::Undeclared(fault)
    }
}

impl From<console_onscreen::Error> for Unpointed {
    fn from(fault: console_onscreen::Error) -> Self {
        Unpointed::Onscreen(fault)
    }
}

fn pointed() -> Result<(), Unpointed> {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let asked = asked(&words)?;
    let at = placed(asked.at, &asked.measured)?;

    let pointer = Pointer::new()?;
    let room = pointer.room;
    let Ok(standing) = on_the_screen(at, room);

    match standing {
        Where::OnTheScreen => {},
        Where::OffIt => {
            return Err(Unpointed::OffTheScreen(at, room));
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
        PointerAction::None => {},
        PointerAction::Click => pointer.click()?,
        PointerAction::Scroll(notches) => pointer.scroll(notches)?,
        PointerAction::Drag(through) => {
            let mut places = Vec::new();

            for place in through {
                let place = placed(place, &asked.measured)?;

                places.push(place);
            }

            pointer.drag(&places)?;
        },
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

fn placed(at: Point<u32>, measured: &Measured) -> Result<Point<u32>, Unpointed> {
    match measured {
        Measured::FromTheScreen => Ok(at),
        Measured::FromTheCorner(namespace) => in_the_surface(at, namespace),
    }
}

fn in_the_surface(at: Point<u32>, namespace: &str) -> Result<Point<u32>, Unpointed> {
    let screens = console_onscreen::screens()?;
    let Ok(drawn) = console_onscreen::standing(&screens, namespace);

    let standing = drawn.ok_or_else(|| Unpointed::NotUp(namespace.to_string()))?;

    let Ok(wide) = fitted::<i64, u32>(standing.width);
    let Ok(tall) = fitted::<i64, u32>(standing.height);
    let Ok(inside) = on_the_screen(at, Size { width: wide, height: tall });

    match inside {
        Where::OnTheScreen => {},
        Where::OffIt => {
            return Err(Unpointed::PastTheSurface(
                at,
                Size { width: wide, height: tall },
                namespace.to_string(),
            ));
        },
    }

    from_the_corner(at, Point { x: standing.x, y: standing.y })
        .map_err(Unpointed::Unsaid)
}

#[derive(Default)]
struct Found {
    manager: Option<ZwlrVirtualPointerManagerV1>,
    sizes: Option<ZxdgOutputManagerV1>,
    seat: Option<wl_seat::WlSeat>,
    screen: Option<wl_output::WlOutput>,
    room: Option<Size<u32>>,
}

struct Pointer {
    connection: Connection,
    said: ZwlrVirtualPointerV1,
    room: Size<u32>,
    since: Instant,
}

impl Pointer {
    fn new() -> Result<Pointer, Unpointed> {
        let connection = Connection::connect_to_env().map_err(Unpointed::NoCompositor)?;
        let mut queue = connection.new_event_queue();
        let handle = queue.handle();
        let _registry = connection.display().get_registry(&handle, ());

        let mut found = Found::default();
        queue.roundtrip(&mut found).map_err(Unpointed::Unsized)?;

        let asking = (found.sizes.as_ref(), found.screen.as_ref());

        match asking {
            (Some(sizes), Some(screen)) => {
                let _ = sizes.get_xdg_output(screen, &handle, ());
            },
            (Some(_), None) | (None, _) => {},
        }

        match queue.roundtrip(&mut found) {
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

        let manager = found.manager.ok_or(Unpointed::NoManager)?;
        let said = manager.create_virtual_pointer_with_output(
            found.seat.as_ref(),
            found.screen.as_ref(),
            &handle,
            (),
        );

        connection.flush().map_err(Unpointed::Unasked)?;

        Ok(Pointer { connection, said, room, since: Instant::now() })
    }

    fn now(&self) -> Result<u32, Never> {
        fitted(self.since.elapsed().as_millis())
    }

    fn flush(&self) -> Result<(), Unpointed> {
        self.connection.flush().map_err(Unpointed::Unwritten)
    }

    fn to(&self, at: Point<u32>) -> Result<(), Unpointed> {
        let Ok(now) = self.now();

        self.said.motion_absolute(now, at.x, at.y, self.room.width, self.room.height);
        self.said.frame();

        self.flush()
    }

    fn click(&self) -> Result<(), Unpointed> {
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

    fn button(&self, state: ButtonState) -> Result<(), Unpointed> {
        let Ok(now) = self.now();

        self.said.button(now, BTN_LEFT, state);
        self.said.frame();

        self.flush()
    }

    fn drag(&self, through: &[Point<u32>]) -> Result<(), Unpointed> {
        self.button(ButtonState::Pressed)?;

        for place in through {
            #[cfg_attr(
                dylint_lib = "explicit021_no_sleeping",
                allow(
                    explicit021_no_sleeping,
                    reason = "a line is drawn at a hand's pace: every place is a motion of its own, in a frame of its own, and motions written in one frame arrive as one jump"
                )
            )]
            std::thread::sleep(STEP);

            self.to(*place)?;
        }

        #[cfg_attr(
            dylint_lib = "explicit021_no_sleeping",
            allow(
                explicit021_no_sleeping,
                reason = "the last place is reached before the button lets go, and released in the same frame as the motion the lift would land before the place did"
            )
        )]
        std::thread::sleep(PRESSED);

        self.button(ButtonState::Released)
    }

    fn scroll(&self, notches: i32) -> Result<(), Unpointed> {
        let far = f64::from(notches) * NOTCH;
        let Ok(now) = self.now();

        self.said.axis_source(AxisSource::Wheel);
        self.said.axis_discrete(now, Axis::VerticalScroll, far, notches);
        self.said.frame();

        self.flush()
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Found {
    #[cfg_attr(
        dylint_lib = "explicit016_no_wildcard_arm",
        allow(
            explicit016_no_wildcard_arm,
            reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
        )
    )]
    fn event(
        found: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        let (name, interface, version) = match event {
            wl_registry::Event::Global { name, interface, version } => (name, interface, version),
            _the_registry_said_something_else => return,
        };

        match interface.as_str() {
            "zwlr_virtual_pointer_manager_v1" => {
                found.manager = Some(registry.bind(name, version.min(2), handle, ()));
            },
            "zxdg_output_manager_v1" => {
                found.sizes = Some(registry.bind(name, version.min(3), handle, ()));
            },
            "wl_seat" => found.seat = Some(registry.bind(name, version.min(5), handle, ())),
            "wl_output" => match found.screen.is_none() {
                true => {
                    found.screen = Some(registry.bind(name, version.min(3), handle, ()));
                }
                false => {},
            },
            _ => {},
        }
    }
}

impl Dispatch<ZxdgOutputV1, ()> for Found {
    #[cfg_attr(
        dylint_lib = "explicit016_no_wildcard_arm",
        allow(
            explicit016_no_wildcard_arm,
            reason = "a Wayland protocol enum is somebody else's and is marked non_exhaustive, so the compiler demands an arm for the events this version of the protocol has not heard of; what this desktop does about one is nothing"
        )
    )]
    fn event(
        found: &mut Self,
        _: &ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let (width, height) = match event {
            zxdg_output_v1::Event::LogicalSize { width, height } => (width, height),
            _the_output_said_something_else => return,
        };

        let Ok(wide) = fitted(width);
        let Ok(tall) = fitted(height);

        found.room = Some(Size { width: wide, height: tall });
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
