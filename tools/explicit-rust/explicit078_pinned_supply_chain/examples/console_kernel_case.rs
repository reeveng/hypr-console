#[derive(serde::Serialize)]
struct Held {
    state: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Compared {
    state: u8,
}

struct Said {
    state: u8,
}

impl core::fmt::Debug for Said {
    fn fmt(&self, asked: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(asked, "Said {{ state: {} }}", self.state)
    }
}

fn main() {
    let _ = (Held { state: 1 }, Said { state: 1 }, Compared { state: 1 });
}
