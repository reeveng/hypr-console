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
//! real one, because the real one is glass and there is nobody holding the
//! machine. The ranges are the panel's own, which is what the real digitizer
//! reports in, so the compositor turns what this says exactly as it turns what
//! that says -- `console_screen::Screen::on_the_panel` is the arithmetic, and
//! it is the same arithmetic either way round.  The device used to be made here
//! by hand: /dev/uinput opened as a file, the ioctl request numbers derived at
//! compile time, `input_event` packed into twenty-four bytes and written. All
//! of that was already in the tree. `console_input_gamepad::uinput` builds the
//! emulator's pad through `evdev`, the daemon publishes its wheel through
//! `evdev`, and `evdev` is on the device because both of them are. Two answers
//! to how this machine makes an input device is one more than it can keep true:
//! a kernel that changed what a multi-touch device has to declare would be
//! found by one of them and not the other. This is the one that was written
//! twice, so this is the one that stopped.
//!     console-tap ACROSS DOWN
//!
//! Where ACROSS and DOWN are a place on the picture, in the size the desktop
//! is laid out in -- the numbers `hyprctl layers` answers in, and the numbers
//! anything drawing a surface thinks in.

use evdev::uinput::VirtualDevice;
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, EventType, InputEvent, InputId, KeyCode,
    PropType, UinputAbsSetup,
};

use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_screen::declared;

const DOWN_FOR: std::time::Duration = std::time::Duration::from_millis(120);

const NOTICED: std::time::Duration = std::time::Duration::from_millis(1200);

fn main() {
    match tapped() {
        Ok(()) => {}
        Err(fault) => {
            eprintln!("console-tap: {fault}");
            std::process::exit(1);
        }
    }
}

fn tapped() -> Result<(), String> {
    let said: Vec<String> = std::env::args().skip(1).collect();

    let (across, down) = match said.as_slice() {
        [across, down] => (across, down),
        _not_two_words => return Err("say where: console-tap ACROSS DOWN".to_string()),
    };

    let across = number(across)?;
    let down = number(down)?;
    let at = (across, down);
    let screen = declared()?;
    let Ok((wide, tall)) = screen.logical();
    let off_screen = at.0 > wide || at.1 > tall;

    match off_screen {
        true => {
            return Err(format!("({}, {}) is not on a {wide}x{tall} screen", at.0, at.1));
        }
        false => {}
    }

    let Ok((x, y)) = screen.on_the_panel(at);
    let mut finger = Finger::new(screen.mode)?;

    #[cfg_attr(
        dylint_lib = "explicit021_no_sleeping",
        allow(
            explicit021_no_sleeping,
            reason = "the touchscreen was created a moment ago and the compositor binds it on its own schedule; nothing on this side of the kernel can be asked whether it has, and a tap into a device nobody is reading is lost"
        )
    )]
    std::thread::sleep(NOTICED);
    finger.at(x, y)?;

    Ok(())
}

fn number(said: &str) -> Result<u32, String> {
    said.parse().map_err(|_| format!("{said} is not a place on the screen"))
}

const HELD: i32 = 1;
const LIFTED: i32 = -1;

const SLOTS: i32 = 9;

const IDS: i32 = 65535;

const TOUCHING: i32 = 1;
const GONE: i32 = 0;

struct Finger {
    device: VirtualDevice,
}

impl Finger {
    fn new(mode: (u32, u32)) -> Result<Finger, String> {
        let touch: AttributeSet<KeyCode> = [KeyCode::BTN_TOUCH].into_iter().collect();
        let direct: AttributeSet<PropType> = [PropType::DIRECT].into_iter().collect();

        let opened = VirtualDevice::builder()
            .map_err(|fault| format!("no way in to /dev/uinput: {fault}"))?;
        let named = opened.name("console-tap").input_id(InputId::new(BusType::BUS_USB, 1, 1, 1));
        let touching = named
            .with_keys(&touch)
            .map_err(|fault| format!("a device that can be touched: {fault}"))?;
        let mut built = touching
            .with_properties(&direct)
            .map_err(|fault| format!("a screen rather than a pad: {fault}"))?;

        let Ok(wide) = fitted(mode.0);
        let Ok(tall) = fitted(mode.1);

        for (axis, most) in [
            (AbsoluteAxisCode::ABS_X, wide),
            (AbsoluteAxisCode::ABS_Y, tall),
            (AbsoluteAxisCode::ABS_MT_POSITION_X, wide),
            (AbsoluteAxisCode::ABS_MT_POSITION_Y, tall),
            (AbsoluteAxisCode::ABS_MT_SLOT, SLOTS),
            (AbsoluteAxisCode::ABS_MT_TRACKING_ID, IDS),
        ] {
            let setup = UinputAbsSetup::new(axis, AbsInfo::new(0, 0, most, 0, 0, 0));
            let with_axis = built
                .with_absolute_axis(&setup)
                .map_err(|fault| format!("the range of an axis: {fault}"))?;

            built = with_axis;
        }

        let device =
            built.build().map_err(|fault| format!("the touchscreen would not be made: {fault}"))?;

        Ok(Finger { device })
    }

    fn at(&mut self, x: u32, y: u32) -> Result<(), String> {
        let Ok(across) = fitted::<u32, i32>(x);
        let Ok(down) = fitted::<u32, i32>(y);

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

        #[cfg_attr(
            dylint_lib = "explicit021_no_sleeping",
            allow(
                explicit021_no_sleeping,
                reason = "the frames are written into a device this process owns, and it is destroyed the moment this returns; the gap is what lets the compositor read the lift before the device goes"
            )
        )]
        std::thread::sleep(DOWN_FOR);

        Ok(())
    }

    fn say(&mut self, frame: &[InputEvent]) -> Result<(), String> {
        self.device.emit(frame).map_err(|fault| format!("writing the touch: {fault}"))
    }
}

fn moved(axis: AbsoluteAxisCode, to: i32) -> Result<InputEvent, Never> {
    Ok(InputEvent::new(EventType::ABSOLUTE.0, axis.0, to))
}

fn touched(how: i32) -> Result<InputEvent, Never> {
    Ok(InputEvent::new(EventType::KEY.0, KeyCode::BTN_TOUCH.0, how))
}
