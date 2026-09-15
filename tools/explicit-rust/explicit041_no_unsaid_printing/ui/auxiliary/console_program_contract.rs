// The contract, as much of it as this test needs: a program hands back a list
// of what it wants done, and printing is on that list.

pub enum Doing {
    Print(String),
    Stop,
}

// The event vocabulary, which is a different thing: a crate that wants to be
// woken when something changes names this and hands nobody a `Doing`.
pub enum Topic {
    Player,
}
