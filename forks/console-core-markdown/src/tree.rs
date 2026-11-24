// Copyright 2018 Google LLC
//
// Use of this source code is governed by an MIT-style
// license that can be found in the LICENSE file or at
// https://opensource.org/licenses/MIT.

//! A Vec-based container for a tree structure.

use std::num::NonZeroUsize;
use std::ops::{Add, Sub};

use crate::parse::{Item, ItemBody};

#[derive(Debug, Eq, PartialEq, Copy, Clone, PartialOrd)]
pub(crate) struct TreeIndex(NonZeroUsize);

impl TreeIndex {
    #[must_use]
    fn new(i: usize) -> Self {
        TreeIndex(NonZeroUsize::new(i).unwrap())
    }

    #[must_use]
    pub fn get(self) -> usize {
        self.0.get()
    }
}

impl Add<usize> for TreeIndex {
    type Output = TreeIndex;

    fn add(self, rhs: usize) -> Self {
        let inner = self.0.get() + rhs;
        TreeIndex::new(inner)
    }
}

impl Sub<usize> for TreeIndex {
    type Output = TreeIndex;

    fn sub(self, rhs: usize) -> Self {
        let inner = self.0.get().checked_sub(rhs).unwrap();
        TreeIndex::new(inner)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Node<T> {
    pub child: Option<TreeIndex>,
    pub next: Option<TreeIndex>,
    pub item: T,
}

/// A tree abstraction, intended for fast building as a preorder traversal.
#[derive(Clone)]
pub(crate) struct Tree<T> {
    nodes: Vec<Node<T>>,
    spine: Vec<TreeIndex>,
    cur: Option<TreeIndex>,
}

impl<T: Default> Tree<T> {



    #[must_use]
    pub(crate) fn with_capacity(cap: usize) -> Tree<T> {
        let mut nodes = Vec::with_capacity(cap);
        nodes.push(Node {
            child: None,
            next: None,
            item: <T as Default>::default(),
        });
        Tree {
            nodes,
            spine: Vec::new(),
            cur: None,
        }
    }

    /// Returns the index of the element currently in focus.
    #[must_use]
    pub(crate) fn cur(&self) -> Option<TreeIndex> {
        self.cur
    }

    /// Append one item to the current position in the tree.
    pub(crate) fn append(&mut self, item: T) -> TreeIndex {
        let ix = self.create_node(item);
        let this = Some(ix);

        match self.cur {
            Some(ix_2) => {
                self[ix_2].next = this;
            },
            ::core::option::Option::None => match self.spine.last() {
                Some(&parent) => {
                    self[parent].child = this;
                },
                ::core::option::Option::None => {},
            },
        }

        self.cur = this;
        ix
    }

    /// Create an isolated node.
    pub(crate) fn create_node(&mut self, item: T) -> TreeIndex {
        let this = self.nodes.len();
        self.nodes.push(Node {
            child: None,
            next: None,
            item,
        });
        TreeIndex::new(this)
    }

    /// Push down one level, so that new items become children of the current node.
    /// The new focus index is returned.
    pub(crate) fn push(&mut self) -> TreeIndex {
        let cur_ix = self.cur.unwrap();
        self.spine.push(cur_ix);
        self.cur = self[cur_ix].child;
        cur_ix
    }

    /// Pop back up a level.
    pub(crate) fn pop(&mut self) -> Option<TreeIndex> {
        let checked_value = self.spine.pop()?;
        let ix = Some(checked_value);
        self.cur = ix;
        ix
    }

    /// Remove the last node, as `pop` but removing it.
    pub(crate) fn remove_node(&mut self) -> Option<TreeIndex> {
        let ix = self.spine.pop()?;
        self.cur = Some(ix);
        self.nodes.pop()?;
        self[ix].child = None;
        Some(ix)
    }

    /// Look at the parent node.
    #[must_use]
    pub(crate) fn peek_up(&self) -> Option<TreeIndex> {
        self.spine.last().copied()
    }

    /// Look at grandparent node.
    #[must_use]
    pub(crate) fn peek_grandparent(&self) -> Option<TreeIndex> {
        match {
            let _explicit_decision = ::core::primitive::bool::then_some(self.spine.len() >= 2, ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                Some(self.spine[self.spine.len() - 2])
            },
            ::core::option::Option::None => {
                None
            },
        }
    }

    /// Returns true when there are no nodes other than the root node
    /// in the tree, false otherwise.
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.nodes.len() <= 1
    }

    /// Returns the length of the spine.
    #[must_use]
    pub(crate) fn spine_len(&self) -> usize {
        self.spine.len()
    }

    /// Resets the focus to the first node added to the tree, if it exists.
    pub(crate) fn reset(&mut self) {
        self.cur = match {
            let _explicit_decision = ::core::primitive::bool::then_some(self.is_empty(), ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                None
            },
            ::core::option::Option::None => {
                Some(TreeIndex::new(1))
            },
        };
        self.spine.clear();
    }

    /// Walks the spine from a root node up to, but not including, the current node.
    #[must_use]
    pub(crate) fn walk_spine(&self) -> impl std::iter::DoubleEndedIterator<Item = &TreeIndex> {
        self.spine.iter()
    }

    /// Moves focus to the next sibling of the given node.
    pub(crate) fn next_sibling(&mut self, cur_ix: TreeIndex) -> Option<TreeIndex> {
        self.cur = self[cur_ix].next;
        self.cur
    }

    pub(crate) fn truncate_to_parent(&mut self, child_ix: TreeIndex) {
        let next = self[child_ix].next;
        self[child_ix].next = None;

        match self.cur {
            Some(cur) => {
                self[cur].next = next;
            },
            ::core::option::Option::None => match self.spine.last() {
                Some(&parent) => {
                    self[parent].child = next;
                },
                ::core::option::Option::None => {},
            },
        }

        match next {
            ::core::option::Option::Some(_) => {
                self.cur = next;
            },
            ::core::option::Option::None => {},
        }
    }
}

impl Tree<Item> {
    /// Truncates the preceding siblings to the given end position,
    /// and returns the new current node.
    pub(crate) fn truncate_siblings(&mut self, end_byte_ix: usize) {
        let parent_ix = self.peek_up().unwrap();
        let mut next_child_ix = self[parent_ix].child;
        let mut prev_child_ix = None;


        while let Some(child_ix) = next_child_ix {
            let child_end = self[child_ix].item.end;

            match {
                let _explicit_decision = ::core::primitive::bool::then_some(child_end < end_byte_ix, ());
                _explicit_decision
            } {
                ::core::option::Option::Some(()) => {

                    prev_child_ix = Some(child_ix);
                    next_child_ix = self[child_ix].next;
                    continue;
                },
                ::core::option::Option::None => match {
                    let _explicit_decision = ::core::primitive::bool::then_some(child_end == end_byte_ix, ());
                    _explicit_decision
                } {
                    ::core::option::Option::Some(()) => {

                        self[child_ix].next = None;

                        self.cur = Some(child_ix);
                    },
                    ::core::option::Option::None => match {
                        let _explicit_decision = ::core::primitive::bool::then_some(self[child_ix].item.start == end_byte_ix, ());
                        _explicit_decision
                    } {
                        ::core::option::Option::Some(()) => {

                            let is_previous_char_backslash_escape = match self[child_ix].item.body {
                                ItemBody::Text { backslash_escaped } => backslash_escaped,
                                _ => false,
                            };

                            match ::core::primitive::bool::then_some(is_previous_char_backslash_escape, ()) {
                                ::core::option::Option::Some(()) => {

                                    let last_byte_ix = end_byte_ix - 1;
                                    self[child_ix].item.start = last_byte_ix;
                                    self[child_ix].item.end = end_byte_ix;
                                    self.cur = Some(child_ix);
                                },
                                ::core::option::Option::None => match prev_child_ix {
                                    Some(prev_child_ix_2) => {


                                        self[prev_child_ix_2].next = None;
                                        self.cur = Some(prev_child_ix_2);
                                    },
                                    ::core::option::Option::None => {

                                        self[parent_ix].child = None;
                                        self.cur = None;
                                    },
                                },
                            }
                        },
                        ::core::option::Option::None => {
                            debug_assert!(self[child_ix].item.start < end_byte_ix);
                            debug_assert!(end_byte_ix < child_end);

                            self[child_ix].item.end = end_byte_ix;
                            self[child_ix].next = None;

                            self.cur = Some(child_ix);
                        },
                    },
                },
            }

            break;
        }
    }
}

impl<T> std::fmt::Debug for Tree<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn debug_tree<T>(
            tree: &Tree<T>,
            cur: TreeIndex,
            indent: usize,
            f: &mut std::fmt::Formatter<'_>,
        ) -> std::fmt::Result
        where
            T: std::fmt::Debug,
        {
            for _ in 0..indent {
                write!(f, "  ")?;
            }

            writeln!(f, "{:?}", &tree[cur].item)?;

            match tree[cur].child {
                Some(child_ix) => {
                    debug_tree(tree, child_ix, indent + 1, f)?;
                },
                ::core::option::Option::None => {},
            }

            match tree[cur].next {
                Some(next_ix) => {
                    debug_tree(tree, next_ix, indent, f)?;
                },
                ::core::option::Option::None => {},
            }

            Ok(())
        }

        match {
            let _explicit_decision = ::core::primitive::bool::then_some(self.nodes.len() > 1, ());
            _explicit_decision
        } {
            ::core::option::Option::Some(()) => {
                let cur = TreeIndex(NonZeroUsize::new(1).unwrap());
                debug_tree(self, cur, 0, f)
            },
            ::core::option::Option::None => {
                write!(f, "Empty tree")
            },
        }
    }
}

impl<T> std::ops::Index<TreeIndex> for Tree<T> {
    type Output = Node<T>;

    fn index(&self, ix: TreeIndex) -> &Self::Output {
        self.nodes.index(ix.get())
    }
}

impl<T> std::ops::IndexMut<TreeIndex> for Tree<T> {
    fn index_mut(&mut self, ix: TreeIndex) -> &mut Node<T> {
        self.nodes.index_mut(ix.get())
    }
}
