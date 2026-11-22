// edition:2024

enum Presence {
    Present,
    Missing,
}

impl From<bool> for Presence {
    fn from(present: bool) -> Self {
        match present {
            true => Self::Present,
            false => Self::Missing,
        }
    }
}

impl Presence {
    fn ordinary_decision(present: bool) -> Self {
        //~v EXPLICIT019_NO_IF
        match present {
            true => Self::Present,
            false => Self::Missing,
        }
    }
}

enum Destination {
    Stay,
    Leave,
}

impl From<bool> for Destination {
    fn from(leave: bool) -> Self {
        //~v EXPLICIT019_NO_IF
        match leave {
            true => {
                eprintln!("leaving");
                Self::Leave
            }
            false => Self::Stay,
        }
    }
}

fn main() {}
