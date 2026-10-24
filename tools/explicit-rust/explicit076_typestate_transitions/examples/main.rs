struct Held {
    state: u8,
}

impl Held {
    fn taken(&mut self, to: u8) {
        self.state = to;
    }
}

fn set_from_outside(held: &mut Held, to: u8) {
    held.state = to;
}

fn asked(held: &mut Held, to: u8) {
    held.taken(to);
}

fn main() {
    let mut held = Held { state: 0 };

    set_from_outside(&mut held, 1);
    asked(&mut held, 2);

    let _ = held.state;
}
