//! Greedy feedback arc set (FAS) selection.
//!
//! Ported from Dagre's `greedy-fas.js`. This is used by `acyclic` when `acyclicer=greedy`.

use crate::graphlib::{EdgeKey, Graph};
use console_core_perfect_hash::collections::{FixedMap as HashMap, FixedSet as HashSet};
use std::collections::{BTreeMap, VecDeque, hash_map::Entry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BucketKind {
    Sink,
    Source,
    Score(i128),
}

#[derive(Debug, Default)]
struct SparseBuckets {
    sink: VecDeque<(String, u128)>,
    source: VecDeque<(String, u128)>,
    scores: BTreeMap<i128, VecDeque<(String, u128)>>,
    current: HashMap<String, (BucketKind, u128)>,
    next_generation: u128,
}

impl SparseBuckets {
    fn assign(&mut self, v: &str, in_w: &HashMap<String, i128>, out_w: &HashMap<String, i128>) {
        let in_v = match in_w.get(v).copied() {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0,
        };
        let out_v = match out_w.get(v).copied() {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0,
        };
        let kind = match out_v {
            0 => {
                BucketKind::Sink
            },
            _ => match in_v {
                0 => {
                    BucketKind::Source
                },
                _ => {
                    BucketKind::Score(out_v.saturating_sub(in_v))
                },
            },
        };
        let generation = self.next_generation;
        self.next_generation = self.next_generation.saturating_add(1);
        let entry = (v.to_string(), generation);

        match kind {
            BucketKind::Sink => self.sink.push_front(entry),
            BucketKind::Source => self.source.push_front(entry),
            BucketKind::Score(score) => self.scores.entry(score).or_default().push_front(entry),
        }

        self.current.insert(v.to_string(), (kind, generation));
    }

    fn pop_sink(&mut self, alive: &HashSet<String>) -> Option<String> {
        pop_valid_bucket(&mut self.sink, BucketKind::Sink, &self.current, alive)
    }

    fn pop_source(&mut self, alive: &HashSet<String>) -> Option<String> {
        pop_valid_bucket(&mut self.source, BucketKind::Source, &self.current, alive)
    }

    fn pop_highest_score(&mut self, alive: &HashSet<String>) -> Option<String> {
        loop {
            let score = self.scores.keys().next_back().copied()?;
            let popped = self.scores.get_mut(&score).and_then(|queue| {
                pop_valid_bucket(queue, BucketKind::Score(score), &self.current, alive)
            });
            let empty = self.scores.get(&score).is_none_or(VecDeque::is_empty);

            match ::core::primitive::bool::then_some(empty, ()) {
                ::core::option::Option::Some(()) => {
                    self.scores.remove(&score);
                },
                ::core::option::Option::None => {},
            }

            match popped {
                ::core::option::Option::Some(_) => {
                    return popped;
                },
                ::core::option::Option::None => {},
            }
        }
    }

    fn remove(&mut self, v: &str) {
        self.current.remove(v);
    }
}

fn pop_valid_bucket(
    bucket: &mut VecDeque<(String, u128)>,
    kind: BucketKind,
    current: &HashMap<String, (BucketKind, u128)>,
    alive: &HashSet<String>,
) -> Option<String> {
    while let Some((v, generation)) = bucket.pop_back() {
        match {
            ::core::primitive::bool::then_some(alive.contains(&v) && current.get(&v) == Some(&(kind, generation)), ())
        } {
            ::core::option::Option::Some(()) => {
                return Some(v);
            },
            ::core::option::Option::None => {},
        }
    }

    None
}

#[must_use]
pub fn greedy_fas<N, E, G>(g: &Graph<N, E, G>) -> Vec<EdgeKey>
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
{
    greedy_fas_with_weight(g, |_| 1)
}

#[must_use]
pub fn greedy_fas_with_weight<N, E, G>(
    g: &Graph<N, E, G>,
    weight_fn: impl Fn(&E) -> i64,
) -> Vec<EdgeKey>
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
{
    match {
        ::core::primitive::bool::then_some(g.node_count() <= 1, ())
    } {
        ::core::option::Option::Some(()) => {
            return Vec::new();
        },
        ::core::option::Option::None => {},
    }

    // Aggregate multi-edges into a simple graph with summed weights.
    //
    // Note: Upstream Dagre preserves Graphlib's JavaScript property-enumeration order for
    // `g.nodes()` / `g.edges()` and the derived `inEdges(v)` / `outEdges(v)` traversals.
    // GreedyFAS is sensitive to that order because it uses stable queues
    // (`List.enqueue` + `List.dequeue`).
    //
    // For parity, keep node initialization in `g.node_ids()` order and keep the aggregated
    // adjacency order based on the *first occurrence* of each `(v, w)` in `g.edges()`.
    let node_ids = g.node_ids();
    let mut in_w: HashMap<String, i128> = HashMap::default();
    let mut out_w: HashMap<String, i128> = HashMap::default();

    for v in &node_ids {
        in_w.insert(v.clone(), 0);
        out_w.insert(v.clone(), 0);
    }

    let mut edge_w: HashMap<(String, String), i128> = HashMap::default();
    let mut edge_order: Vec<(String, String)> = Vec::new();

    for e in g.edges() {
        let w = i128::from(match g.edge_by_key(e).map(&weight_fn) {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 1,
        });
        let key = (e.v.clone(), e.w.clone());

        match edge_w.entry(key.clone()) {
            Entry::Vacant(v) => {
                v.insert(w);
                edge_order.push(key);
            }
            Entry::Occupied(mut o) => {
                let combined = o.get().saturating_add(w);
                *o.get_mut() = combined;
            }
        }

        let o = out_w.entry(e.v.clone()).or_insert(0);
        *o = (*o).saturating_add(w);
        let i = in_w.entry(e.w.clone()).or_insert(0);
        *i = (*i).saturating_add(w);
    }

    let mut buckets = SparseBuckets::default();

    for v in &node_ids {
        buckets.assign(v, &in_w, &out_w);
    }

    // Build adjacency for the aggregated graph (for efficient updates).
    let mut in_edges: HashMap<String, Vec<(String, i128)>> = HashMap::default();
    let mut out_edges: HashMap<String, Vec<(String, i128)>> = HashMap::default();

    for (v, w) in &edge_order {
        let wgt = match edge_w.get(&(v.clone(), w.clone())).copied() {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0,
        };
        out_edges
            .entry(v.clone())
            .or_default()
            .push((w.clone(), wgt));
        in_edges
            .entry(w.clone())
            .or_default()
            .push((v.clone(), wgt));
    }

    let mut alive: HashSet<String> = node_ids.iter().cloned().collect();
    let mut results: Vec<(String, String)> = Vec::new();

    struct Work<'a> {
        alive: &'a mut HashSet<String>,
        buckets: &'a mut SparseBuckets,
        in_w: &'a mut HashMap<String, i128>,
        out_w: &'a mut HashMap<String, i128>,
        in_edges: &'a HashMap<String, Vec<(String, i128)>>,
        out_edges: &'a HashMap<String, Vec<(String, i128)>>,
    }

    impl Work<'_> {
        fn remove_node(&mut self, v: &str) {
            self.remove_node_inner(v, None);
        }

        fn remove_node_collect_predecessors(&mut self, v: &str, preds: &mut Vec<(String, String)>) {
            self.remove_node_inner(v, Some(preds));
        }

        fn remove_node_inner(
            &mut self,
            v: &str,
            collect_predecessors: Option<&mut Vec<(String, String)>>,
        ) {
            match {
                ::core::primitive::bool::then_some(!self.alive.remove(v), ())
            } {
                ::core::option::Option::Some(()) => {
                    return;
                },
                ::core::option::Option::None => {},
            }

            if let Some(preds) = collect_predecessors
                && let Some(ins) = self.in_edges.get(v)
            {
                for (u, _) in ins {
                    match {
                        ::core::primitive::bool::then_some(self.alive.contains(u), ())
                    } {
                        ::core::option::Option::Some(()) => {
                            preds.push((u.clone(), v.to_string()));
                        },
                        ::core::option::Option::None => {},
                    }
                }
            }

            match self.in_edges.get(v) {
                Some(ins) => {
                    for (u, wgt) in ins {
                        match {
                            ::core::primitive::bool::then_some(!self.alive.contains(u), ())
                        } {
                            ::core::option::Option::Some(()) => {
                                continue;
                            },
                            ::core::option::Option::None => {},
                        }

                        match self.out_w.get_mut(u) {
                            Some(o) => {
                                *o = (*o).saturating_sub(*wgt);
                            },
                            ::core::option::Option::None => {},
                        }

                        self.buckets.assign(u, &*self.in_w, &*self.out_w);
                    }
                },
                ::core::option::Option::None => {},
            }

            match self.out_edges.get(v) {
                Some(outs) => {
                    for (w, wgt) in outs {
                        match {
                            ::core::primitive::bool::then_some(!self.alive.contains(w), ())
                        } {
                            ::core::option::Option::Some(()) => {
                                continue;
                            },
                            ::core::option::Option::None => {},
                        }

                        match self.in_w.get_mut(w) {
                            Some(i) => {
                                *i = (*i).saturating_sub(*wgt);
                            },
                            ::core::option::Option::None => {},
                        }

                        self.buckets.assign(w, &*self.in_w, &*self.out_w);
                    }
                },
                ::core::option::Option::None => {},
            }

            self.in_w.remove(v);
            self.out_w.remove(v);
            self.buckets.remove(v);
        }
    }

    let mut work = Work {
        alive: &mut alive,
        buckets: &mut buckets,
        in_w: &mut in_w,
        out_w: &mut out_w,
        in_edges: &in_edges,
        out_edges: &out_edges,
    };

    while !work.alive.is_empty() {
        // Drain sinks (out == 0).
        while let Some(v) = work.buckets.pop_sink(&*work.alive) {
            work.remove_node(&v);
        }

        // Drain sources (in == 0).
        while let Some(v) = work.buckets.pop_source(&*work.alive) {
            work.remove_node(&v);
        }

        match {
            ::core::primitive::bool::then_some(work.alive.is_empty(), ())
        } {
            ::core::option::Option::Some(()) => {
                break;
            },
            ::core::option::Option::None => {},
        }

        // Pick a node from the highest non-extreme bucket and collect its predecessor edges.
        let picked = work.buckets.pop_highest_score(&*work.alive);

        let Some(v) = picked else {
            // Should not happen, but avoid an infinite loop.
            let v = node_ids
                .iter()
                .find(|id| work.alive.contains(*id))
                .cloned()
                .or_else(|| work.alive.iter().next().cloned());
            let v = match v {
                Some(matched_v) => matched_v,
                ::core::option::Option::None => break,
            };
            work.remove_node(&v);
            continue;
        };

        let mut preds: Vec<(String, String)> = Vec::new();
        work.remove_node_collect_predecessors(&v, &mut preds);
        results.extend(preds);
    }

    // Expand multi-edges back to concrete edge keys from the original graph.
    let mut out: Vec<EdgeKey> = Vec::new();

    for (v, w) in results {
        out.extend(g.out_edges(&v, Some(&w)));
    }

    out
}
