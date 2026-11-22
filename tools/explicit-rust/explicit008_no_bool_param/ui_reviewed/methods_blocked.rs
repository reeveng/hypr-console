// edition:2024

#[derive(Clone, Copy)]
enum TreeSize {
    Large,
    Small,
}

struct Tree;

impl Tree {
    fn missing_consumer(&self, large: bool) -> u32 {
        match large {
            true => 128,
            false => 64,
        }
    }

    fn as_callback(&self, large: bool) -> u32 {
        match large {
            true => 128,
            false => 64,
        }
    }
}

#[cfg(any())]
fn hidden(tree: &Tree) -> u32 {
    tree.missing_consumer(true)
}

fn main() {
    let callback: fn(&Tree, bool) -> u32 = Tree::as_callback;
    assert_eq!(callback(&Tree, true), 128);
    assert_eq!(Tree.missing_consumer(false), 64);
}
