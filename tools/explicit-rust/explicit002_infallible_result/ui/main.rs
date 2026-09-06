// UI test for EXPLICIT002 -- registered `Warn`, so these are warnings rather
// than errors, which is the warned tier printing its remaining distance.

fn plain() -> i32 {
    0
}

fn nothing() {}

fn already_says_so() -> Result<i32, ()> {
    Ok(0)
}

fn never_answers() -> ! {
    loop {}
}

// Not asked: an alias is a name for a `Result`, and this one already says how
// it fails.
type Done = Result<(), String>;

fn done() -> Done {
    Ok(())
}

// Not asked: the signature belongs to the trait.
struct Counted;

impl Default for Counted {
    fn default() -> Self {
        Counted
    }
}

impl Drop for Counted {
    fn drop(&mut self) {}
}

// Not asked: the signature belongs to the ABI.
extern "C" fn handled(_signal: i32) {}

// Not asked: the entry point has no caller to keep the same shape for.
fn main() {
    let _ = plain();
    let _ = already_says_so();
    let _ = done();
    let _ = Counted::default();
    let _ = handled;
    nothing();
}
