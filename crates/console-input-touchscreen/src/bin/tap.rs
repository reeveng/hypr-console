//! Put a finger down on the screen, at a place on the picture.  For the checks.
//! Everything else this desktop can be asked to do can be asked with a button
//! -- InputPlumber publishes the pad and a check presses it over D-Bus -- and a
//! touch is the one thing nothing could send, so it was the one thing no check
//! ever asked. That is not a small gap: a surface can be on the screen, at the
//! right size, on the right layer, drawing the right thing, and answer no
//! finger at all, and every question that is not a finger answers yes.  Which
//! is what happened to the bar. The home screen asked the compositor for the
//! keyboard exclusively, an exclusive layer is handed every touch on the screen
//! wherever it lands, and so every tap on the bar went to the home screen.
//! Nothing was broken about the bar and nothing about the bar could be asked to
//! find that out.  It makes a touchscreen of its own rather than driving the
//! real one, because the real one is glass and there is no one holding the
//! machine. The ranges are the panel's own, which is what the real digitizer
//! reports in, so the compositor turns what this says exactly as it turns what
//! that says -- `console_screen::Screen::on_the_panel` is the arithmetic, and
//! it is the same arithmetic either way round.  The device used to be made here
//! by hand: /dev/uinput opened as a file, the ioctl request numbers derived at
//! compile time, `input_event` packed into twenty-four bytes and written. All
//! of that was already in the tree. `console_input_gamepad::uinput` builds the
//! emulator's pad through `console-input-event-devices`, the daemon publishes
//! its wheel through it, and so does this. Two answers to how this machine
//! makes an input device is one more than it can keep true:
//! a kernel that changed what a multi-touch device has to declare would be
//! found by one of them and not the other. This is the one that was written
//! twice, so this is the one that stopped.
//!     console-tap ACROSS DOWN
//!
//! Where ACROSS and DOWN are a place on the picture, in the size the desktop
//! is laid out in -- the numbers `hyprctl layers` answers in, and the numbers
//! anything drawing a surface thinks in.

use std::process::ExitCode;

use console_input_event_devices::{
    AbsInfo, AbsoluteAxisCode, BusType, EventType, InputEvent, InputId, KeyCode, PropType, Setup,
    Unmade, VirtualDevice,
};

use console_core_geometry::{Point, Size};
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_screen::declared;

const DOWN_FOR: std::time::Duration = std::time::Duration::from_millis(120);

const NOTICED: std::time::Duration = std::time::Duration::from_millis(1200);

const BETWEEN: std::time::Duration = std::time::Duration::from_millis(30);

const SWIPE: &str = "--swipe";

const STEPS: i64 = 12;

const STEP: std::time::Duration = std::time::Duration::from_millis(16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sweeping {
    Yes,
    No,
}

fn main() -> ExitCode {
    match tapped() {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("console-tap: {fault}");

            ExitCode::FAILURE
        }
    }
}

#[derive(Debug)]
enum Untouched {
    NotTwoWords,
    NotASwipe,
    NotAPlace(String),
    Undeclared(console_screen::Undeclared),
    OffScreen(Point<u32>, Size<u32>),
    Unbuilt(Unmade),
    Unwritten(std::io::Error),
}

impl std::fmt::Display for Untouched {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Untouched::NotTwoWords => write!(to, "say where: console-tap ACROSS DOWN [ACROSS DOWN ...], or console-tap --swipe ACROSS DOWN ACROSS DOWN"),
            Untouched::NotASwipe => write!(to, "a swipe goes from one place to one other: console-tap --swipe ACROSS DOWN ACROSS DOWN"),
            Untouched::NotAPlace(said) => write!(to, "{said} is not a place on the screen"),
            Untouched::Undeclared(fault) => write!(to, "{fault}"),
            Untouched::OffScreen(at, room) => write!(
                to,
                "({}, {}) is not on a {}x{} screen",
                at.x, at.y, room.width, room.height
            ),
            Untouched::Unbuilt(fault) => write!(to, "the touchscreen would not be made: {fault}"),
            Untouched::Unwritten(fault) => write!(to, "writing the touch: {fault}"),
        }
    }
}

impl std::error::Error for Untouched {}

impl From<console_screen::Undeclared> for Untouched {
    fn from(fault: console_screen::Undeclared) -> Self {
        Untouched::Undeclared(fault)
    }
}

fn tapped() -> Result<(), Untouched> {
    let said: Vec<String> = std::env::args().skip(1).collect();
    let (sweeping, places) = match said.split_first() {
        Some((first, rest)) => match first.as_str() {
            SWIPE => (Sweeping::Yes, rest),
            _a_place => (Sweeping::No, said.as_slice()),
        },
        None => (Sweeping::No, said.as_slice()),
    };
    let screen = declared()?;
    let spots = placed(places, &screen)?;
    let mut finger = Touch::new(screen.mode)?;

    #[cfg_attr(
        dylint_lib = "explicit021_no_sleeping",
        allow(
            explicit021_no_sleeping,
            reason = "the touchscreen was created a moment ago and the compositor binds it on its own schedule; nothing on this side of the kernel can be asked whether it has, and a tap into a device no one is reading is lost"
        )
    )]
    std::thread::sleep(NOTICED);

    match sweeping {
        Sweeping::Yes => match spots.as_slice() {
            [from, to] => finger.swept(*from, *to)?,
            _not_two_places => return Err(Untouched::NotASwipe),
        },
        Sweeping::No => {
            for spot in spots {
                finger.at(spot)?;

                #[cfg_attr(
                    dylint_lib = "explicit021_no_sleeping",
                    allow(
                        explicit021_no_sleeping,
                        reason = "the gap between two taps from a thumb that is hurrying, which is the rate being asked for rather than a wait for anything"
                    )
                )]
                std::thread::sleep(BETWEEN);
            }
        }
    }

    #[cfg_attr(
        dylint_lib = "explicit021_no_sleeping",
        allow(
            explicit021_no_sleeping,
            reason = "the frames are written into a device this process owns, and it is destroyed the moment this returns; the gap is what lets the compositor read the last lift before the device goes"
        )
    )]
    std::thread::sleep(DOWN_FOR);

    Ok(())
}

fn placed(said: &[String], screen: &console_screen::Screen) -> Result<Vec<Point<u32>>, Untouched> {
    let pairs = said.chunks_exact(2);

    match (said.is_empty(), pairs.remainder().is_empty()) {
        (false, true) => {}
        (true, _) | (false, false) => return Err(Untouched::NotTwoWords),
    }

    let Ok(room) = screen.logical();
    let mut spots: Vec<Point<u32>> = Vec::new();

    for pair in pairs {
        let (across, down) = match pair {
            [across, down] => (across, down),
            _not_two_words => return Err(Untouched::NotTwoWords),
        };
        let across = number(across)?;
        let down = number(down)?;
        let at = Point { x: across, y: down };
        let off_screen = at.x > room.width || at.y > room.height;

        match off_screen {
            true => return Err(Untouched::OffScreen(at, room)),
            false => {}
        }

        let Ok(spot) = screen.on_the_panel(at);

        spots.push(spot);
    }

    Ok(spots)
}

fn number(said: &str) -> Result<u32, Untouched> {
    said.parse()
        .map_err(|_| Untouched::NotAPlace(said.to_string()))
}

const HELD: i32 = 1;
const LIFTED: i32 = -1;

const SLOTS: i32 = 9;

const IDS: i32 = 65535;

const TOUCHING: i32 = 1;
const GONE: i32 = 0;

struct Touch {
    device: VirtualDevice,
}

impl Touch {
    fn new(mode: Size<u32>) -> Result<Touch, Untouched> {
        let Ok(wide) = fitted(mode.width);
        let Ok(tall) = fitted(mode.height);
        let reaching = |maximum: i32| AbsInfo { maximum, ..AbsInfo::default() };
        let setup = Setup {
            name: "console-tap".to_string(),
            id: InputId { bus: BusType::BUS_USB, vendor: 1, product: 1, version: 1 },
            keys: vec![KeyCode::BTN_TOUCH],
            properties: vec![PropType::DIRECT],
            absolute_axes: vec![
                (AbsoluteAxisCode::ABS_X, reaching(wide)),
                (AbsoluteAxisCode::ABS_Y, reaching(tall)),
                (AbsoluteAxisCode::ABS_MT_POSITION_X, reaching(wide)),
                (AbsoluteAxisCode::ABS_MT_POSITION_Y, reaching(tall)),
                (AbsoluteAxisCode::ABS_MT_SLOT, reaching(SLOTS)),
                (AbsoluteAxisCode::ABS_MT_TRACKING_ID, reaching(IDS)),
            ],
            ..Setup::default()
        };
        let device = VirtualDevice::create(&setup).map_err(Untouched::Unbuilt)?;

        Ok(Touch { device })
    }

    fn at(&mut self, spot: Point<u32>) -> Result<(), Untouched> {
        let Ok(across) = fitted::<u32, i32>(spot.x);
        let Ok(down) = fitted::<u32, i32>(spot.y);

        let Ok(slot) = moved(AbsoluteAxisCode::ABS_MT_SLOT, 0);
        let Ok(held) = moved(AbsoluteAxisCode::ABS_MT_TRACKING_ID, HELD);
        let Ok(finger_across) = moved(AbsoluteAxisCode::ABS_MT_POSITION_X, across);
        let Ok(finger_down) = moved(AbsoluteAxisCode::ABS_MT_POSITION_Y, down);
        let Ok(pointer_across) = moved(AbsoluteAxisCode::ABS_X, across);
        let Ok(pointer_down) = moved(AbsoluteAxisCode::ABS_Y, down);
        let Ok(touching) = touched(TOUCHING);

        self.say(&[
            slot,
            held,
            finger_across,
            finger_down,
            pointer_across,
            pointer_down,
            touching,
        ])?;

        #[cfg_attr(
            dylint_lib = "explicit021_no_sleeping",
            allow(
                explicit021_no_sleeping,
                reason = "a tap is a finger down and a finger up with a person-shaped gap between them; lifted in the same frame it is not a tap anything downstream counts"
            )
        )]
        std::thread::sleep(DOWN_FOR);

        let Ok(lifted) = moved(AbsoluteAxisCode::ABS_MT_TRACKING_ID, LIFTED);
        let Ok(gone) = touched(GONE);

        self.say(&[lifted, gone])?;

        Ok(())
    }

    fn swept(&mut self, from: Point<u32>, to: Point<u32>) -> Result<(), Untouched> {
        let Ok(from_across) = fitted::<u32, i64>(from.x);
        let Ok(from_down) = fitted::<u32, i64>(from.y);
        let Ok(to_across) = fitted::<u32, i64>(to.x);
        let Ok(to_down) = fitted::<u32, i64>(to.y);
        let Ok(across) = fitted::<i64, i32>(from_across);
        let Ok(down) = fitted::<i64, i32>(from_down);

        let Ok(slot) = moved(AbsoluteAxisCode::ABS_MT_SLOT, 0);
        let Ok(held) = moved(AbsoluteAxisCode::ABS_MT_TRACKING_ID, HELD);
        let Ok(finger_across) = moved(AbsoluteAxisCode::ABS_MT_POSITION_X, across);
        let Ok(finger_down) = moved(AbsoluteAxisCode::ABS_MT_POSITION_Y, down);
        let Ok(pointer_across) = moved(AbsoluteAxisCode::ABS_X, across);
        let Ok(pointer_down) = moved(AbsoluteAxisCode::ABS_Y, down);
        let Ok(touching) = touched(TOUCHING);

        self.say(&[slot, held, finger_across, finger_down, pointer_across, pointer_down, touching])?;

        for step in 1..=STEPS {
            let along = |from: i64, to: i64| {
                from.saturating_add(to.saturating_sub(from).saturating_mul(step).saturating_div(STEPS))
            };
            let Ok(across) = fitted::<i64, i32>(along(from_across, to_across));
            let Ok(down) = fitted::<i64, i32>(along(from_down, to_down));

            #[cfg_attr(
                dylint_lib = "explicit021_no_sleeping",
                allow(
                    explicit021_no_sleeping,
                    reason = "a swipe is a finger moving at a finger's pace, a frame at a time; the gap is the pace being asked for rather than a wait for anything"
                )
            )]
            std::thread::sleep(STEP);

            let Ok(finger_across) = moved(AbsoluteAxisCode::ABS_MT_POSITION_X, across);
            let Ok(finger_down) = moved(AbsoluteAxisCode::ABS_MT_POSITION_Y, down);
            let Ok(pointer_across) = moved(AbsoluteAxisCode::ABS_X, across);
            let Ok(pointer_down) = moved(AbsoluteAxisCode::ABS_Y, down);

            self.say(&[finger_across, finger_down, pointer_across, pointer_down])?;
        }

        let Ok(lifted) = moved(AbsoluteAxisCode::ABS_MT_TRACKING_ID, LIFTED);
        let Ok(gone) = touched(GONE);

        self.say(&[lifted, gone])?;

        Ok(())
    }

    fn say(&mut self, frame: &[InputEvent]) -> Result<(), Untouched> {
        self.device.emit(frame).map_err(Untouched::Unwritten)
    }
}

fn moved(axis: AbsoluteAxisCode, to: i32) -> Result<InputEvent, Never> {
    Ok(InputEvent { kind: EventType::ABSOLUTE, code: axis.0, value: to })
}

fn touched(how: i32) -> Result<InputEvent, Never> {
    Ok(InputEvent { kind: EventType::KEY, code: KeyCode::BTN_TOUCH.0, value: how })
}
