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

#[allow(dead_code)]
struct Converted(u8);

impl TryFrom<u8> for Converted {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, ()> {
        if value == 0 { Err(()) } else { Ok(Self(value)) }
    }
}

impl AsMut<u8> for Converted {
    fn as_mut(&mut self) -> &mut u8 {
        &mut self.0
    }
}

#[allow(dead_code)]
struct Parsed(u8);

impl std::str::FromStr for Parsed {
    type Err = std::num::ParseIntError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        text.parse::<u8>().map(Self)
    }
}

impl std::ops::Deref for Parsed {
    type Target = u8;

    fn deref(&self) -> &u8 {
        &self.0
    }
}

impl std::ops::DerefMut for Parsed {
    fn deref_mut(&mut self) -> &mut u8 {
        &mut self.0
    }
}

#[allow(dead_code)]
struct Unchecked(u8);

impl AsMut<u8> for Unchecked {
    fn as_mut(&mut self) -> &mut u8 {
        &mut self.0
    }
}

impl Unchecked {
    fn new(held: u8) -> Result<Self, std::convert::Infallible> {
        Ok(Self(held))
    }
}

fn main() {
    let _ = (Ringed::try_new(1), Asked::new(1), Held::new(1), Unchecked::new(1));
}

#[cfg_attr(
    dylint_lib = "explicit065_checked_constructors",
    expect(
        explicit065_checked_constructors,
        reason = "conversion checks the source width and every stored byte is valid"
    )
)]
#[derive(Default)]
pub struct Representation(pub u8);

impl TryFrom<u16> for Representation {
    type Error = std::num::TryFromIntError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        u8::try_from(value).map(Self)
    }
}
