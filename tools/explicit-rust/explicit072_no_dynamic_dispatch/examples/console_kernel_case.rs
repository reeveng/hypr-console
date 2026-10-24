trait Drawn {
    fn draw(&self) -> u8;
}

struct Carried {
    what: Box<dyn Drawn>,
    held: fn() -> u8,
}

#[allow(dead_code)]
fn drawn(what: &dyn Drawn, held: fn() -> u8) -> u8 {
    what.draw().saturating_add(held())
}

#[allow(dead_code)]
fn counted(held: u8) -> u8 {
    held
}

struct Kept;

impl Kept {
    fn handed(&self, what: &dyn Drawn) -> u8 {
        what.draw()
    }

    fn table(&self, held: [fn() -> u8; 2]) -> u8 {
        held.len().try_into().unwrap_or(0)
    }
}

trait Asked {
    fn asked(&self, what: Option<Box<dyn Drawn>>) -> u8;
}

impl Asked for Kept {
    fn asked(&self, what: Option<Box<dyn Drawn>>) -> u8 {
        what.map_or(0, |held| held.draw())
    }
}

impl Drawn for Kept {
    fn draw(&self) -> u8 {
        1
    }
}

fn main() {
    let _ = (Kept.handed(&Kept), Kept.table([|| 1, || 2]), Kept.asked(None));
}
