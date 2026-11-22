// run-rustfix
// edition:2024

use std::cell::Cell;
use std::convert::Infallible;

enum Never {}
type Done = Result<(), Never>;

fn send(counter: &Cell<u8>) -> Done {
    counter.set(counter.get() + 1);
    Ok(())
}

struct CountOnDrop<'a>(&'a Cell<u8>);

impl Drop for CountOnDrop<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

fn main() {
    let counter = Cell::new(0);
    send(&counter);
    Result::<(), Infallible>::Ok(());
    assert_eq!(counter.get(), 1);
    Result::<_, Infallible>::Ok(CountOnDrop(&counter));
    assert_eq!(counter.get(), 2);
}
