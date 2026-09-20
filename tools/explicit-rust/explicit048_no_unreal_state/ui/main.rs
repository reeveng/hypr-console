// UI test for EXPLICIT048 — a type spells no state it does not have.

struct Socket;

struct Fault;

// BAD EXPLICIT048 — a flag and two absences: sixteen spellings, four states.
//~v EXPLICIT048_NO_UNREAL_STATE
struct Connection {
    connected: bool,
    error: Option<Fault>,
    socket: Option<Socket>,
}

// GOOD — the same four states, named, and the compiler counts them.
enum Connected {
    Disconnected,
    Connected(Socket),
    Failed(Fault),
}

// BAD EXPLICIT048 — the same shape with the payload left out.
//~v EXPLICIT048_NO_UNREAL_STATE
struct Standing {
    open: bool,
    pressed: bool,
}

// GOOD — one flag, which is a flag.
struct Lit {
    on: bool,
    name: String,
}

// GOOD — two optional things, which is what two `Option`s usually are.
struct Song {
    title: Option<String>,
    artist: Option<String>,
}

// BAD EXPLICIT048 — three states spelled as two questions.
//~v EXPLICIT048_NO_UNREAL_STATE
fn looked_up() -> Option<Option<Socket>> {
    None
}

// BAD EXPLICIT048 — the same, with the second question spelled as a failure.
//~v EXPLICIT048_NO_UNREAL_STATE
fn tried() -> Option<Result<Socket, Fault>> {
    None
}

// GOOD — two questions with different answers: whether the call happened, and
// whether the thing is there. EXPLICIT002 puts every function through this.
fn opened() -> Result<Option<Socket>, Fault> {
    Ok(None)
}

// GOOD — one question, one answer.
fn asked() -> Option<Socket> {
    None
}

fn main() {
    let _ = (Connected::Disconnected, looked_up(), tried(), opened(), asked());
}
