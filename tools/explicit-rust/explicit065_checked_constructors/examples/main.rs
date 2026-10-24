#[derive(Default)]
struct Ringed {
    held: u8,
}

impl Ringed {
    fn try_new(held: u8) -> Result<Self, u8> {
        match held > 0 {
            true => Ok(Self { held }),
            false => Err(held),
        }
    }
}

struct Asked {
    pub held: u8,
}

impl Asked {
    fn new(held: u8) -> Result<Self, u8> {
        Ok(Self { held })
    }
}

struct Held {
    pub held: u8,
}

impl Held {
    fn new(held: u8) -> Self {
        Self { held }
    }
}

fn main() {
    let _ = (Ringed::try_new(1), Asked::new(1), Held::new(1));
}
