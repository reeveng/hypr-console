//! The kernel's numbers for what an event is about, with the kernel's names.
//!
//! Every code is the number `input-event-codes.h` gives it, in a type of its
//! own so a key cannot be handed where an axis was asked for. The names are
//! the header's and nobody else's, because a binding, a capture and a log line
//! all have to be read by somebody holding the header rather than this tree,
//! and a name this tree invented for `KEY_VOLUMEUP` is one they would have to
//! translate. A number the header has no name for is still a code; it prints
//! as the number.

use std::fmt;

use console_core_never::Never;

pub trait Code: Copy + PartialEq + 'static {
    const KIND: &'static str;

    const NAMED: &'static [(&'static str, Self)];

    fn number(self) -> Result<u16, Never>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoSuchName {
    pub kind: &'static str,
    pub word: String,
}

impl fmt::Display for NoSuchName {
    fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(to, "{} is not the name of any {} the kernel has", self.word, self.kind)
    }
}

impl std::error::Error for NoSuchName {}

pub fn name<C: Code>(code: C) -> Result<Option<&'static str>, Never> {
    Ok(C::NAMED.iter().find(|(_, named)| *named == code).map(|(name, _)| *name))
}

pub fn named<C: Code>(word: &str) -> Result<C, NoSuchName> {
    match C::NAMED.iter().find(|(name, _)| *name == word) {
        Some((_, code)) => Ok(*code),
        None => Err(NoSuchName { kind: C::KIND, word: word.to_string() }),
    }
}

pub fn written<C: Code>(code: C, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    let Ok(said) = name(code);
    let Ok(number) = code.number();

    match said {
        Some(said) => to.pad(said),
        None => write!(to, "unknown {} {number}", C::KIND),
    }
}

macro_rules! codes {
    ($kind:ident, $said:literal, $($name:ident = $value:literal,)*) => {
        #[repr(transparent)]
        #[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $kind(pub u16);

        impl $kind {
            $(pub const $name: $kind = $kind($value);)*
        }

        impl $crate::codes::Code for $kind {
            const KIND: &'static str = $said;

            const NAMED: &'static [(&'static str, $kind)] = &[$((stringify!($name), $kind($value)),)*];

            fn number(self) -> Result<u16, console_core_never::Never> {
                Ok(self.0)
            }
        }

        impl std::fmt::Debug for $kind {
            fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                $crate::codes::written(*self, to)
            }
        }

        impl std::str::FromStr for $kind {
            type Err = $crate::codes::NoSuchName;

            fn from_str(word: &str) -> Result<$kind, $crate::codes::NoSuchName> {
                $crate::codes::named(word)
            }
        }
    };
}

pub(crate) use codes;

codes!(
    EventType,
    "event type",
    SYNCHRONIZATION = 0x00,
    KEY = 0x01,
    RELATIVE = 0x02,
    ABSOLUTE = 0x03,
    MISC = 0x04,
    SWITCH = 0x05,
    LED = 0x11,
    SOUND = 0x12,
    REPEAT = 0x14,
    FORCEFEEDBACK = 0x15,
    POWER = 0x16,
    FORCEFEEDBACKSTATUS = 0x17,
    UINPUT = 0x0101,
);

codes!(
    SynchronizationCode,
    "synchronization",
    SYN_REPORT = 0,
    SYN_CONFIG = 1,
    SYN_MT_REPORT = 2,
    SYN_DROPPED = 3,
);

codes!(
    PropType,
    "property",
    POINTER = 0x00,
    DIRECT = 0x01,
    BUTTONPAD = 0x02,
    SEMI_MT = 0x03,
    TOPBUTTONPAD = 0x04,
    POINTING_STICK = 0x05,
    ACCELEROMETER = 0x06,
);

codes!(
    RelativeAxisCode,
    "relative axis",
    REL_X = 0x00,
    REL_Y = 0x01,
    REL_Z = 0x02,
    REL_RX = 0x03,
    REL_RY = 0x04,
    REL_RZ = 0x05,
    REL_HWHEEL = 0x06,
    REL_DIAL = 0x07,
    REL_WHEEL = 0x08,
    REL_MISC = 0x09,
    REL_RESERVED = 0x0a,
    REL_WHEEL_HI_RES = 0x0b,
    REL_HWHEEL_HI_RES = 0x0c,
);

codes!(
    AbsoluteAxisCode,
    "absolute axis",
    ABS_X = 0x00,
    ABS_Y = 0x01,
    ABS_Z = 0x02,
    ABS_RX = 0x03,
    ABS_RY = 0x04,
    ABS_RZ = 0x05,
    ABS_THROTTLE = 0x06,
    ABS_RUDDER = 0x07,
    ABS_WHEEL = 0x08,
    ABS_GAS = 0x09,
    ABS_BRAKE = 0x0a,
    ABS_HAT0X = 0x10,
    ABS_HAT0Y = 0x11,
    ABS_HAT1X = 0x12,
    ABS_HAT1Y = 0x13,
    ABS_HAT2X = 0x14,
    ABS_HAT2Y = 0x15,
    ABS_HAT3X = 0x16,
    ABS_HAT3Y = 0x17,
    ABS_PRESSURE = 0x18,
    ABS_DISTANCE = 0x19,
    ABS_TILT_X = 0x1a,
    ABS_TILT_Y = 0x1b,
    ABS_TOOL_WIDTH = 0x1c,
    ABS_VOLUME = 0x20,
    ABS_MISC = 0x28,
    ABS_MT_SLOT = 0x2f,
    ABS_MT_TOUCH_MAJOR = 0x30,
    ABS_MT_TOUCH_MINOR = 0x31,
    ABS_MT_WIDTH_MAJOR = 0x32,
    ABS_MT_WIDTH_MINOR = 0x33,
    ABS_MT_ORIENTATION = 0x34,
    ABS_MT_POSITION_X = 0x35,
    ABS_MT_POSITION_Y = 0x36,
    ABS_MT_TOOL_TYPE = 0x37,
    ABS_MT_BLOB_ID = 0x38,
    ABS_MT_TRACKING_ID = 0x39,
    ABS_MT_PRESSURE = 0x3a,
    ABS_MT_DISTANCE = 0x3b,
    ABS_MT_TOOL_X = 0x3c,
    ABS_MT_TOOL_Y = 0x3d,
);

codes!(
    MiscCode,
    "miscellaneous code",
    MSC_SERIAL = 0x00,
    MSC_PULSELED = 0x01,
    MSC_GESTURE = 0x02,
    MSC_RAW = 0x03,
    MSC_SCAN = 0x04,
    MSC_TIMESTAMP = 0x05,
);

codes!(
    ForceFeedbackCode,
    "force feedback effect",
    FF_RUMBLE = 0x50,
    FF_PERIODIC = 0x51,
    FF_CONSTANT = 0x52,
    FF_SPRING = 0x53,
    FF_FRICTION = 0x54,
    FF_DAMPER = 0x55,
    FF_INERTIA = 0x56,
    FF_RAMP = 0x57,
    FF_SQUARE = 0x58,
    FF_TRIANGLE = 0x59,
    FF_SINE = 0x5a,
    FF_SAW_UP = 0x5b,
    FF_SAW_DOWN = 0x5c,
    FF_CUSTOM = 0x5d,
    FF_GAIN = 0x60,
    FF_AUTOCENTER = 0x61,
);

codes!(
    BusType,
    "bus",
    BUS_PCI = 0x01,
    BUS_ISAPNP = 0x02,
    BUS_USB = 0x03,
    BUS_HIL = 0x04,
    BUS_BLUETOOTH = 0x05,
    BUS_VIRTUAL = 0x06,
    BUS_ISA = 0x10,
    BUS_I8042 = 0x11,
    BUS_XTKBD = 0x12,
    BUS_RS232 = 0x13,
    BUS_GAMEPORT = 0x14,
    BUS_PARPORT = 0x15,
    BUS_AMIGA = 0x16,
    BUS_ADB = 0x17,
    BUS_I2C = 0x18,
    BUS_HOST = 0x19,
    BUS_GSC = 0x1A,
    BUS_ATARI = 0x1B,
    BUS_SPI = 0x1C,
    BUS_RMI = 0x1D,
    BUS_CEC = 0x1E,
    BUS_INTEL_ISHTP = 0x1F,
);

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn a_code_prints_as_the_name_the_header_gives_it() {
        assert_eq!(format!("{:?}", AbsoluteAxisCode::ABS_HAT0X), "ABS_HAT0X");
        assert_eq!(format!("{:?}", EventType::KEY), "KEY");
    }

    #[test]
    fn a_code_the_header_does_not_name_prints_as_its_number() {
        assert_eq!(format!("{:?}", RelativeAxisCode(15)), "unknown relative axis 15");
    }

    #[test]
    fn a_name_reads_back_as_its_code() {
        assert_eq!(AbsoluteAxisCode::from_str("ABS_MT_SLOT"), Ok(AbsoluteAxisCode::ABS_MT_SLOT));
        assert!(RelativeAxisCode::from_str("REL_NOTHING").is_err());
    }
}
