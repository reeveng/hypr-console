// run-rustfix
// edition:2024

#[derive(Clone, Copy)]
enum TreeSize {
    Large,
    Small,
}

struct Tree;

impl Tree {
    pub(crate) fn branch(&self, large: bool) -> u32 {
        match large {
            true => 128,
            false => 64,
        }
    }

    fn leaf(&self, large: bool) -> u32 {
        self.branch(large)
    }
}

struct OtherTree;

impl OtherTree {
    fn branch(&self) -> u32 {
        8
    }
}

fn main() {
    let tree = Tree;
    let depending_on_args = std::env::args().len() > 1;
    let _ = tree.leaf(depending_on_args);
    assert_eq!(Tree::branch(&tree, false), 64);
    assert_eq!(OtherTree.branch(), 8);
}
