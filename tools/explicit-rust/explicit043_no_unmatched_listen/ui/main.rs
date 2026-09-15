// UI test for EXPLICIT043 — a topic listened to is a topic deafened. The
// question is asked of the crate rather than of the line, so what this file
// says once about `Listening` is what settles both of its calls.

pub struct Wants(pub &'static str);

pub enum Doing {
    Listen(Wants),
    Deafen(Wants),
    Print(String),
}

pub struct Listening;

impl Listening {
    pub fn also(&self, _topic: &str) {}

    pub fn not(&self, _topic: &str) {}
}

// BAD EXPLICIT043 — this crate says `Listen` and never says `Deafen`, so
// everything it listens to it listens to for as long as it runs.
fn starts_caring() -> Doing {
    //~v EXPLICIT043_NO_UNMATCHED_LISTEN
    Doing::Listen(Wants("sound"))
}

// GOOD — the pair is written, in the two places it belongs.
fn starts(words: &Listening) {
    words.also("sound");
}

fn stops(words: &Listening) {
    words.not("sound");
}

// GOOD — handling a doing somebody else made is a pattern, not a listen.
fn what_it_means(doing: &Doing) -> &'static str {
    match doing {
        Doing::Listen(_) => "started",
        Doing::Deafen(_) => "stopped",
        Doing::Print(_) => "said",
    }
}

fn main() {
    let _ = starts_caring();
    let _ = what_it_means(&Doing::Print(String::new()));
}
