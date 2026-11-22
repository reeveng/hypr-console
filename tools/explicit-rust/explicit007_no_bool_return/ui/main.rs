// edition:2024
// compile-flags: --test
// UI test for EXPLICIT007 — bool return values are forbidden.

enum Never {}

type Answered<T> = Result<T, Never>;

#[derive(Debug)]
struct Unreadable;

// BAD EXPLICIT007
//~v EXPLICIT007_NO_BOOL_RETURN
fn saves() -> bool {
    true
}

// BAD -- a `Result` whose error cannot happen is still a `bool`.
//~v EXPLICIT007_NO_BOOL_RETURN
fn is_empty() -> Result<bool, Never> {
    Ok(true)
}

// BAD -- and so is the standard library's name for the same thing.
//~v EXPLICIT007_NO_BOOL_RETURN
fn is_full() -> Result<bool, std::convert::Infallible> {
    Ok(true)
}

// BAD -- an alias is a name for the `Result`, not a second kind of answer.
//~v EXPLICIT007_NO_BOOL_RETURN
fn is_open() -> Answered<bool> {
    Ok(true)
}

// BAD -- a trait written here is asked where it is written, body or none.
trait Asked {
    //~v EXPLICIT007_NO_BOOL_RETURN
    fn is_held(&self) -> bool;

    //~v EXPLICIT007_NO_BOOL_RETURN
    fn is_provided(&self) -> Result<bool, Never> {
        Ok(true)
    }

    //~v EXPLICIT007_NO_BOOL_RETURN
    async fn will_hold(&self) -> bool;
}

// GOOD -- the impl of that trait had no choice to make; the trait did.
struct Holder;
impl Asked for Holder {
    fn is_held(&self) -> bool {
        true
    }

    async fn will_hold(&self) -> bool {
        true
    }
}

// GOOD — an enum tells the caller what went wrong.
#[derive(Debug, PartialEq, Eq)]
enum Save { Outcome { kind: u8 }, }
fn saves_ok() -> Result<Save, std::io::Error> {
    Ok(Save::Outcome { kind: 0 })
}

// BAD -- the failure type leaves the success outcomes unnamed.
//~v EXPLICIT007_NO_BOOL_RETURN
fn reads_a_flag() -> Result<bool, Unreadable> {
    Err(Unreadable)
}

// BAD -- an optional Boolean still needs named outcomes.
//~v EXPLICIT007_NO_BOOL_RETURN
fn gets_a_flag() -> Answered<Option<bool>> {
    Ok(None)
}

// BAD -- a Boolean failure also needs named outcomes.
//~v EXPLICIT007_NO_BOOL_RETURN
fn boolean_failure() -> Result<Save, bool> {
    Err(false)
}

// BAD -- an async function returns a Boolean through its future.
//~v EXPLICIT007_NO_BOOL_RETURN
async fn ready_later() -> bool {
    true
}

// BAD -- the same rule applies to a declared future output.
//~v EXPLICIT007_NO_BOOL_RETURN
fn done_later() -> impl core::future::Future<Output = Option<bool>> {
    async { Some(true) }
}

#[test]
//~v EXPLICIT007_NO_BOOL_RETURN
fn truth() -> Result<(), bool> {
    Err(false)
}

// GOOD -- the trait fixed this signature, so the impl had no choice to make.
struct Pair(u8);
impl PartialEq for Pair {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

fn main() {
    let _ = (saves(), is_empty(), is_full(), is_open(), Holder.is_held(), Holder.is_provided());
    let _ = (saves_ok(), reads_a_flag(), Pair(0) == Pair(1));
}
