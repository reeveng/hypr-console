//! Helper algorithms re-exported for Dagre compatibility.

use super::Graph;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[must_use]
pub fn preorder<N, E, G>(g: &Graph<N, E, G>, roots: &[&str]) -> Vec<String>
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
{
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut out: Vec<String> = Vec::new();

    for r in roots {
        assert!(g.has_node(r), "preorder root is not in the graph: {r}");
        let mut stack = vec![r.to_string()];

        while let Some(v) = stack.pop() {
            match {
                ::core::primitive::bool::then_some(!visited.insert(v.clone()), ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            out.push(v.clone());
            let successors = g.successors(&v);

            for w in successors.into_iter().rev() {
                stack.push(w.to_string());
            }
        }
    }

    out
}

#[must_use]
pub fn postorder<N, E, G>(g: &Graph<N, E, G>, roots: &[&str]) -> Vec<String>
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
{
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut out: Vec<String> = Vec::new();

    for r in roots {
        assert!(g.has_node(r), "postorder root is not in the graph: {r}");
        let mut stack = vec![(r.to_string(), false)];

        while let Some((v, expanded)) = stack.pop() {
            match ::core::primitive::bool::then_some(expanded, ()) {
                ::core::option::Option::Some(()) => {
                    out.push(v);
                    continue;
                },
                ::core::option::Option::None => {},
            }

            match {
                ::core::primitive::bool::then_some(!visited.insert(v.clone()), ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            stack.push((v.clone(), true));
            let successors = g.successors(&v);

            for w in successors.into_iter().rev() {
                stack.push((w.to_string(), false));
            }
        }
    }

    out
}

#[must_use]
pub fn components<N, E, G>(g: &Graph<N, E, G>) -> Vec<Vec<String>>
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
{
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let ids = g.node_ids();
    let mut out: Vec<Vec<String>> = Vec::new();

    for start in ids {
        match {
            ::core::primitive::bool::then_some(!seen.insert(start.clone()), ())
        } {
            ::core::option::Option::Some(()) => {
                continue;
            },
            ::core::option::Option::None => {},
        }

        let mut comp: Vec<String> = Vec::new();
        let mut q: VecDeque<String> = VecDeque::new();
        q.push_back(start);

        while let Some(v) = q.pop_front() {
            comp.push(v.clone());

            for n in g.successors(&v) {
                match {
                    ::core::primitive::bool::then_some(seen.insert(n.to_string()), ())
                } {
                    ::core::option::Option::Some(()) => {
                        q.push_back(n.to_string());
                    },
                    ::core::option::Option::None => {},
                }
            }

            for n in g.predecessors(&v) {
                match {
                    ::core::primitive::bool::then_some(seen.insert(n.to_string()), ())
                } {
                    ::core::option::Option::Some(()) => {
                        q.push_back(n.to_string());
                    },
                    ::core::option::Option::None => {},
                }
            }
        }

        out.push(comp);
    }

    out
}

#[must_use]
pub fn find_cycles<N, E, G>(g: &Graph<N, E, G>) -> Vec<Vec<String>>
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
{
    // Strongly connected components (Tarjan). Report SCCs with size > 1, or self-loops.
    let node_ids = g.node_ids();

    struct TarjanFrame {
        v: String,
        successors: Vec<String>,
        next_successor: usize,
    }

    struct Tarjan<'a, N, E, G>
    where
        N: Default + 'static,
        E: Default + 'static,
        G: Default,
    {
        g: &'a Graph<N, E, G>,
        index: usize,
        stack: Vec<String>,
        on_stack: BTreeSet<String>,
        indices: BTreeMap<String, usize>,
        lowlink: BTreeMap<String, usize>,
        sccs: Vec<Vec<String>>,
    }

    impl<N, E, G> Tarjan<'_, N, E, G>
    where
        N: Default + 'static,
        E: Default + 'static,
        G: Default,
    {
        fn push_frame(&mut self, v: String, frames: &mut Vec<TarjanFrame>) {
            self.indices.insert(v.to_string(), self.index);
            self.lowlink.insert(v.to_string(), self.index);
            self.index += 1;
            self.stack.push(v.to_string());
            self.on_stack.insert(v.to_string());
            frames.push(TarjanFrame {
                successors: self
                    .g
                    .successors(&v)
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                v,
                next_successor: 0,
            });
        }

        fn strongconnect_iterative(&mut self, root: &str) {
            let mut frames: Vec<TarjanFrame> = Vec::new();
            self.push_frame(root.to_string(), &mut frames);

            while !frames.is_empty() {
                let next = {
                    let frame = match frames.last_mut() {
                        Some(frame) => frame,
                        None => break,
                    };

                    match {
                        ::core::primitive::bool::then_some(frame.next_successor < frame.successors.len(), ())
                    } {
                        ::core::option::Option::Some(()) => {
                            let w = frame.successors[frame.next_successor].clone();
                            frame.next_successor += 1;
                            Some(w)
                        },
                        ::core::option::Option::None => {
                            None
                        },
                    }
                };

                match next {
                    Some(w) => {
                        let v = match frames.last() {
                            Some(frame) => frame.v.clone(),
                            None => break,
                        };

                        match {
                            ::core::primitive::bool::then_some(!self.indices.contains_key(&w), ())
                        } {
                            ::core::option::Option::Some(()) => {
                                self.push_frame(w, &mut frames);
                                continue;
                            },
                            ::core::option::Option::None => {},
                        }

                        match {
                            ::core::primitive::bool::then_some(self.on_stack.contains(&w), ())
                        } {
                            ::core::option::Option::Some(()) => {
                                let Some(v_low) = self.lowlink.get(&v).copied() else {
                                    debug_assert!(false, "tarjan lowlink missing for v");
                                    continue;
                                };

                                let Some(w_idx) = self.indices.get(&w).copied() else {
                                    debug_assert!(false, "tarjan index missing for w");
                                    continue;
                                };

                                self.lowlink.insert(v, v_low.min(w_idx));
                            },
                            ::core::option::Option::None => {},
                        }

                        continue;
                    },
                    ::core::option::Option::None => {},
                }

                let Some(frame) = frames.pop() else {
                    break;
                };

                let v = frame.v;

                let Some(v_low) = self.lowlink.get(&v).copied() else {
                    debug_assert!(false, "tarjan lowlink missing for v");
                    continue;
                };

                let Some(v_idx) = self.indices.get(&v).copied() else {
                    debug_assert!(false, "tarjan index missing for v");
                    continue;
                };

                match {
                    ::core::primitive::bool::then_some(v_low == v_idx, ())
                } {
                    ::core::option::Option::Some(()) => {
                        let mut scc: Vec<String> = Vec::new();

                        loop {
                            let Some(w) = self.stack.pop() else {
                                debug_assert!(false, "tarjan stack underflow");
                                break;
                            };

                            self.on_stack.remove(&w);
                            scc.push(w.clone());

                            match {
                                ::core::primitive::bool::then_some(w == v, ())
                            } {
                                ::core::option::Option::Some(()) => {
                                    break;
                                },
                                ::core::option::Option::None => {},
                            }
                        }

                        self.sccs.push(scc);
                    },
                    ::core::option::Option::None => {},
                }

                match frames.last_mut() {
                    Some(parent) => {
                        let Some(parent_low) = self.lowlink.get(&parent.v).copied() else {
                            debug_assert!(false, "tarjan lowlink missing for parent");
                            continue;
                        };

                        self.lowlink.insert(parent.v.clone(), parent_low.min(v_low));
                    },
                    ::core::option::Option::None => {},
                }
            }
        }
    }

    let mut tarjan = Tarjan {
        g,
        index: 0,
        stack: Vec::new(),
        on_stack: BTreeSet::new(),
        indices: BTreeMap::new(),
        lowlink: BTreeMap::new(),
        sccs: Vec::new(),
    };

    for v in &node_ids {
        match {
            ::core::primitive::bool::then_some(!tarjan.indices.contains_key(v), ())
        } {
            ::core::option::Option::Some(()) => {
                tarjan.strongconnect_iterative(v);
            },
            ::core::option::Option::None => {},
        }
    }

    let mut cycles: Vec<Vec<String>> = Vec::new();
    let mut node_order: Option<BTreeMap<&str, usize>> = None;

    for mut scc in tarjan.sccs {
        match {
            ::core::primitive::bool::then_some(scc.len() > 1, ())
        } {
            ::core::option::Option::Some(()) => {
                // Share the original node order across SCCs; singleton components need no index.
                let order = node_order.get_or_insert_with(|| {
                    node_ids
                        .iter()
                        .enumerate()
                        .map(|(i, v)| (v.as_str(), i))
                        .collect()
                });
                scc.sort_by_key(|v| order.get(v.as_str()).copied().unwrap_or(usize::MAX));
                cycles.push(scc);
            },
            ::core::option::Option::None => {
                let v = &scc[0];

                match {
                    ::core::primitive::bool::then_some(g.has_edge(v, v, None) || !g.out_edges(v, Some(v)).is_empty(), ())
                } {
                    ::core::option::Option::Some(()) => {
                        cycles.push(vec![v.clone()]);
                    },
                    ::core::option::Option::None => {},
                }
            },
        }
    }

    cycles.sort_by(|a, b| a.first().cmp(&b.first()));
    cycles
}
