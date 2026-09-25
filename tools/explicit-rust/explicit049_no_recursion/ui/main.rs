// UI test for EXPLICIT049 — a function does not reach itself; the depth is a
// loop's to count.

use std::fmt;

struct Folder {
    name: String,
    within: Vec<Folder>,
}

// BAD EXPLICIT049 — the walk goes as deep as the tree it is handed.
fn count(folder: &Folder) -> u32 {
    let mut found = 1;

    for inner in &folder.within {
        //~v EXPLICIT049_NO_RECURSION
        found += count(inner);
    }

    found
}

// BAD EXPLICIT049 — two functions, and neither body says the pair recurses.
fn open_one(folder: &Folder) -> u32 {
    //~v EXPLICIT049_NO_RECURSION
    open_all(&folder.within)
}

fn open_all(folders: &[Folder]) -> u32 {
    //~v EXPLICIT049_NO_RECURSION
    folders.iter().map(open_one).sum()
}

// BAD EXPLICIT049 — a method reaching itself through a closure is the same
// walk.
impl Folder {
    fn deepest(&self) -> u32 {
        //~v EXPLICIT049_NO_RECURSION
        self.within.iter().map(|inner| inner.deepest()).max().unwrap_or(0) + 1
    }
}

// BAD EXPLICIT049 — a trait method is followed to the impl the types choose.
impl fmt::Display for Folder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;

        for inner in &self.within {
            //~v EXPLICIT049_NO_RECURSION
            fmt::Display::fmt(inner, f)?;
        }

        Ok(())
    }
}

// GOOD — a stack of its own, walked until it is empty.
fn counted(folder: &Folder) -> u32 {
    let mut found = 0;
    let mut waiting = vec![folder];

    while let Some(here) = waiting.pop() {
        found += 1;
        waiting.extend(here.within.iter());
    }

    found
}

// GOOD — one function calling another that does not come back.
fn named(folder: &Folder) -> usize {
    folder.name.len()
}

fn described(folder: &Folder) -> usize {
    named(folder) + counted(folder) as usize
}

// GOOD — a callback that registers the same function again runs later, from
// the loop that holds it, and not on this stack.
fn on_press(waiting: &mut Vec<Box<dyn FnOnce(&mut Vec<u32>)>>) {
    waiting.push(Box::new(move |pressed| {
        pressed.push(1);
        let _ = on_press;
    }));
}

fn main() {
    let root = Folder { name: String::new(), within: Vec::new() };
    let _ = (count(&root), open_one(&root), root.deepest(), described(&root), root.to_string());
}
