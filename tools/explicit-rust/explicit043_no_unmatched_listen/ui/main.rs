// UI test for EXPLICIT043 — a topic listened to is a topic the program stops listening to. The
// question is asked of the crate rather than of the line, so what this file
// says once about `Subscriber` is what settles both of its calls.

pub struct Subscription(pub &'static str);

pub enum Effect {
    Subscribe(Subscription),
    Unsubscribe(Subscription),
    Print(String),
}

pub struct Subscriber;

impl Subscriber {
    pub fn subscribe(&self, _topic: &str) {}

    pub fn unsubscribe(&self, _topic: &str) {}
}

// BAD EXPLICIT043 — this crate says `Subscribe` and never says `Unsubscribe`, so
// everything it listens to it listens to for as long as it runs.
fn starts_caring() -> Effect {
    //~v EXPLICIT043_NO_UNMATCHED_LISTEN
    Effect::Subscribe(Subscription("sound"))
}

// GOOD — the pair is written, in the two places it belongs.
fn starts(words: &Subscriber) {
    words.subscribe("sound");
}

fn stops(words: &Subscriber) {
    words.unsubscribe("sound");
}

// GOOD — handling an effect someone else made is a pattern, not a listen.
fn what_it_means(effect: &Effect) -> &'static str {
    match effect {
        Effect::Subscribe(_) => "started",
        Effect::Unsubscribe(_) => "stopped",
        Effect::Print(_) => "said",
    }
}

fn main() {
    let _ = starts_caring();
    let _ = what_it_means(&Effect::Print(String::new()));
}
