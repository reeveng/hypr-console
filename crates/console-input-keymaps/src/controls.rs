#[derive(Copy, Clone, Eq, PartialEq, Hash, Default)]
pub(crate) struct ControlsMask(pub u32);

bitmask!(ControlsMask);
