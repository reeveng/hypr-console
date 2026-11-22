// edition:2024

#![allow(dead_code, unexpected_cfgs)]

mod enabled {
    pub enum Stage {
        Ready,
        Pending,
    }
}

mod alternate {
    pub enum Stage {
        Ready,
        Pending,
        Failed,
    }
}

mod selected {
    #[cfg(not(alternate))]
    pub use super::enabled::Stage;
    #[cfg(alternate)]
    pub use super::alternate::Stage;
}

use selected::Stage as Imported;

type StageAlias = Imported;

impl StageAlias {
    fn self_type(self) -> u8 {
        if let Self::Ready = self { 1 } else { 0 }
    }
}

fn qualified(value: selected::Stage) -> u8 {
    if let selected::Stage::Ready = value { 1 } else { 0 }
}

fn forwarded(value: Imported) -> u8 {
    if let Imported::Ready = value { 1 } else { 0 }
}

fn main() {}
