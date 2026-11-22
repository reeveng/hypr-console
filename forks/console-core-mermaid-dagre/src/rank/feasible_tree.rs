//! Feasible tree construction used by the network simplex ranker.

use super::tree;
use crate::graphlib::{Graph, GraphOptions};
use crate::work::{checked_add, checked_mul, checked_ordered_key_updates};
use crate::{EdgeLabel, GraphLabel, NodeLabel};
use crate::{NoopWorkControl, WorkControl, WorkError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TightTreeEdge {
    v_ix: usize,
    w_ix: usize,
    minlen: i128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TightTreeFrame {
    v_ix: usize,
    next_incoming: usize,
    incoming_end: usize,
    next_outgoing: usize,
    outgoing_end: usize,
}

#[derive(Debug, Default)]
struct TightTreeScratch {
    roots: Vec<usize>,
    frames: Vec<TightTreeFrame>,
}

#[must_use]
fn tight_tree_frame(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    v_ix: usize,
) -> Option<TightTreeFrame> {
    g.node_id_by_ix(v_ix)?;
    let (incoming_end, outgoing_end) = match {
        ::core::primitive::bool::then_some(g.is_directed(), ())
    } {
        ::core::option::Option::Some(()) => {
            (g.in_edge_count_ix(v_ix), g.out_edge_count_ix(v_ix))
        },
        ::core::option::Option::None => {
            let incident = g.undirected_edge_count_ix(v_ix);
            (incident, incident)
        },
    };
    Some(TightTreeFrame {
        v_ix,
        next_incoming: 0,
        incoming_end,
        next_outgoing: 0,
        outgoing_end,
    })
}

fn next_tight_tree_edge(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    frame: &mut TightTreeFrame,
) -> Option<TightTreeEdge> {
    match {
        ::core::primitive::bool::then_some(g.is_directed(), ())
    } {
        ::core::option::Option::Some(()) => {
            match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(frame.next_incoming), &::core::convert::identity(frame.incoming_end)); _explicit_ordering } {
                ::core::cmp::Ordering::Less => {
                    let position = frame.next_incoming;
                    frame.next_incoming += 1;
                    let (v_ix, w_ix, _key, label) = g.in_edge_entry_ix_at(frame.v_ix, position)?;
                    return Some(TightTreeEdge {
                        v_ix,
                        w_ix,
                        minlen: label.minlen as i128,
                    });
                },
                ::core::cmp::Ordering::Equal | ::core::cmp::Ordering::Greater => {},
            }

            match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(frame.next_outgoing), &::core::convert::identity(frame.outgoing_end)); _explicit_ordering } {
                ::core::cmp::Ordering::Less => {
                    let position = frame.next_outgoing;
                    frame.next_outgoing += 1;
                    let (v_ix, w_ix, _key, label) = g.out_edge_entry_ix_at(frame.v_ix, position)?;
                    return Some(TightTreeEdge {
                        v_ix,
                        w_ix,
                        minlen: label.minlen as i128,
                    });
                },
                ::core::cmp::Ordering::Equal | ::core::cmp::Ordering::Greater => {},
            }

            return None;
        },
        ::core::option::Option::None => {},
    }

    // Graphlib `nodeEdges(v)` is canonical incoming edges followed by canonical outgoing edges.
    // The undirected CSR stores global incident order, so scan that shared slice twice and filter
    // by the canonical endpoint without allocating a second adjacency index.
    while frame.next_incoming < frame.incoming_end {
        let position = frame.next_incoming;
        frame.next_incoming += 1;
        let (v_ix, w_ix, _key, label) = g.undirected_edge_entry_ix_at(frame.v_ix, position)?;

        match {
            ::core::primitive::bool::then_some(w_ix == frame.v_ix, ())
        } {
            ::core::option::Option::Some(()) => {
                return Some(TightTreeEdge {
                    v_ix,
                    w_ix,
                    minlen: label.minlen as i128,
                });
            },
            ::core::option::Option::None => {},
        }
    }

    while frame.next_outgoing < frame.outgoing_end {
        let position = frame.next_outgoing;
        frame.next_outgoing += 1;
        let (v_ix, w_ix, _key, label) = g.undirected_edge_entry_ix_at(frame.v_ix, position)?;

        match {
            ::core::primitive::bool::then_some(v_ix == frame.v_ix, ())
        } {
            ::core::option::Option::Some(()) => {
                return Some(TightTreeEdge {
                    v_ix,
                    w_ix,
                    minlen: label.minlen as i128,
                });
            },
            ::core::option::Option::None => {},
        }
    }

    None
}

pub fn feasible_tree(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()> {
    let mut work_control = NoopWorkControl;
    feasible_tree_controlled(g, &mut work_control)
        .expect("the checked no-op Dugong work control cannot reject feasible-tree work")
}

pub(crate) fn feasible_tree_controlled(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>, WorkError> {
    let prepare_adjacency_cache = !g.is_adjacency_cache_current();
    work_control.charge(feasible_tree_setup_work_units(g)?)?;
    crate::rank::validate_rank_arithmetic(g)?;

    match ::core::primitive::bool::then_some(prepare_adjacency_cache, ()) {
        ::core::option::Option::Some(()) => {
            g.prepare_adjacency_cache();
        },
        ::core::option::Option::None => {},
    }

    let mut rank_by_ix: Vec<i128> = Vec::new();
    g.for_each_node_ix(|ix, _id, lbl| {
        match {
            ::core::primitive::bool::then_some(ix >= rank_by_ix.len(), ())
        } {
            ::core::option::Option::Some(()) => {
                rank_by_ix.resize(ix + 1, 0);
            },
            ::core::option::Option::None => {},
        }

        rank_by_ix[ix] = i128::from(match lbl.rank {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0,
        });
    });
    let mut in_tree_by_ix: Vec<bool> = vec![false; rank_by_ix.len()];
    let mut tree_g_ixs: Vec<usize> = Vec::new();

    let mut t: Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()> = Graph::new(GraphOptions {
        directed: false,
        ..Default::default()
    });
    let mut tight_scratch = TightTreeScratch::default();

    let start = {
        let optional_start = g.nodes().next().map(|s| s.to_string());

        match optional_start {
            Some(matched_start) => matched_start,
            ::core::option::Option::None => return Ok(t),
        }
    };
    let size = g.node_count();
    t.set_node(start.clone(), tree::TreeNodeLabel::default());

    match g.node_ix(&start) {
        Some(ix) => {
            match {
                ::core::primitive::bool::then_some(ix >= in_tree_by_ix.len(), ())
            } {
                ::core::option::Option::Some(()) => {
                    in_tree_by_ix.resize(ix + 1, false);
                    rank_by_ix.resize(ix + 1, 0);
                },
                ::core::option::Option::None => {},
            }

            in_tree_by_ix[ix] = true;
            tree_g_ixs.push(ix);
        },
        ::core::option::Option::None => {},
    }

    loop {
        work_control.charge(feasible_tree_iteration_work_units(g)?)?;

        match {
            ::core::primitive::bool::then_some(tight_tree(
                &mut t,
                g,
                &rank_by_ix,
                &mut in_tree_by_ix,
                &mut tree_g_ixs,
                &mut tight_scratch,
            ) >= size, ())
        } {
            ::core::option::Option::Some(()) => {
                break;
            },
            ::core::option::Option::None => {},
        }

        let Some((slack, in_v)) = find_min_slack_edge(g, &rank_by_ix, &in_tree_by_ix) else {
            // Disconnected graphs can occur in downstream usage. Dagre effectively works
            // per component; here we create a forest by starting a new component root.
            let mut next_root: Option<(usize, String)> = None;
            g.for_each_node_ix(|ix, id, _lbl| {
                match next_root {
                    ::core::option::Option::Some(_) => {
                        return;
                    },
                    ::core::option::Option::None => {},
                }

                match {
                    ::core::primitive::bool::then_some(match in_tree_by_ix.get(ix).copied() {
                        ::core::option::Option::Some(present) => present,
                        ::core::option::Option::None => false,
                    }, ())
                } {
                    ::core::option::Option::Some(()) => {
                        return;
                    },
                    ::core::option::Option::None => {},
                }

                next_root = Some((ix, id.to_string()));
            });
            let (ix, next_root) = match next_root {
                Some((matched_ix, matched_next_root)) => (matched_ix, matched_next_root),
                ::core::option::Option::None => break,
            };

            match {
                ::core::primitive::bool::then_some(ix >= in_tree_by_ix.len(), ())
            } {
                ::core::option::Option::Some(()) => {
                    in_tree_by_ix.resize(ix + 1, false);
                    rank_by_ix.resize(ix + 1, 0);
                },
                ::core::option::Option::None => {},
            }

            in_tree_by_ix[ix] = true;
            tree_g_ixs.push(ix);
            t.set_node(next_root, tree::TreeNodeLabel::default());
            continue;
        };

        let delta = match ::core::primitive::bool::then_some(in_v, ()) {
            ::core::option::Option::Some(()) => { slack },
            ::core::option::Option::None => { -slack },
        };
        shift_ranks(g, &mut rank_by_ix, &tree_g_ixs, delta)?;
    }

    Ok(t)
}

fn feasible_tree_setup_work_units(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<usize, WorkError> {
    // Rank state and the owner-local DFS frame buffers use a bounded number of node-slot passes.
    let node_work = checked_mul(g.node_slot_count(), 10)?;
    // Rank validation scans every slot once. A stale shared CSR cache performs two additional slot
    // scans, then initializes one flat entry per directed endpoint (or two per undirected edge)
    // before filling those entries. Materialization begins only after the complete setup tranche
    // is accepted.
    let edge_work = match {
        ::core::primitive::bool::then_some(g.is_adjacency_cache_current(), ())
    } {
        ::core::option::Option::Some(()) => {
            g.edge_slot_count()
        },
        ::core::option::Option::None => {
            checked_add(
                checked_mul(g.edge_slot_count(), 3)?,
                checked_mul(g.edge_count(), 2)?,
            )?
        },
    };
    let numeric_node_work =
        checked_ordered_key_updates(g.node_count(), g.array_index_node_count())?;
    // The undirected feasible tree preserves Graphlib's counted predecessor/successor Object.keys
    // state while it is assembled. Every tree edge is selected from `g`, so the source graph's
    // numeric endpoint-property count is a sound upper bound for all ordered adjacency inserts.
    let numeric_adjacency_work =
        checked_ordered_key_updates(g.node_count(), g.pinned_array_index_adjacency_entry_count())?;
    checked_add(
        checked_add(node_work, edge_work)?,
        checked_add(numeric_node_work, numeric_adjacency_work)?,
    )
}

fn feasible_tree_iteration_work_units(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<usize, WorkError> {
    let node_work = checked_mul(g.node_count(), 4)?;
    let adjacency_work = checked_mul(g.edge_count(), match {
        ::core::primitive::bool::then_some(g.is_directed(), ())
    } {
        ::core::option::Option::Some(()) => { 2 },
        ::core::option::Option::None => { 4 },
    })?;
    let edge_work = checked_add(g.edge_slot_count(), adjacency_work)?;
    checked_add(node_work, edge_work)
}

fn tight_tree(
    t: &mut Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    rank_by_ix: &[i128],
    in_tree_by_ix: &mut [bool],
    tree_g_ixs: &mut Vec<usize>,
    scratch: &mut TightTreeScratch,
) -> usize {
    let TightTreeScratch { roots, frames } = scratch;
    roots.clear();
    t.for_each_node(|id, _label| {
        {
            match g.node_ix(id) {
                Some(g_ix) => {
                    roots.push(g_ix);
                },
                ::core::option::Option::None => {},
            };
        }
    });

    for &root_ix in roots.iter() {
        frames.clear();
        let root_frame = {
            let optional_root_frame = tight_tree_frame(g, root_ix);

            match optional_root_frame {
                Some(matched_root_frame) => matched_root_frame,
                ::core::option::Option::None => continue,
            }
        };
        frames.push(root_frame);

        'inner_while_2: while let Some(frame) = frames.last_mut() {
            let tree_v_ix = frame.v_ix;

            let Some(edge) = next_tight_tree_edge(g, frame) else {
                frames.pop();
                continue 'inner_while_2;
            };

            let w_ix = match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(tree_v_ix), &::core::convert::identity(edge.v_ix)); _explicit_ordering } {
                ::core::cmp::Ordering::Equal => {
                    edge.w_ix
                },
                ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Greater => match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(tree_v_ix), &::core::convert::identity(edge.w_ix)); _explicit_ordering } {
                    ::core::cmp::Ordering::Equal => {
                        edge.v_ix
                    },
                    ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Greater => {
                        continue 'inner_while_2;
                    },
                },
            };

            match {
                ::core::primitive::bool::then_some(match in_tree_by_ix.get(w_ix).copied() {
                    ::core::option::Option::Some(present) => present,
                    ::core::option::Option::None => false,
                }, ())
            } {
                ::core::option::Option::Some(()) => {
                    continue 'inner_while_2;
                },
                ::core::option::Option::None => {},
            }

            let v_rank = match rank_by_ix.get(edge.v_ix).copied() {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => 0,
            };
            let w_rank = match rank_by_ix.get(edge.w_ix).copied() {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => 0,
            };

            match {
                ::core::primitive::bool::then_some(w_rank - v_rank - edge.minlen != 0, ())
            } {
                ::core::option::Option::Some(()) => {
                    continue 'inner_while_2;
                },
                ::core::option::Option::None => {},
            }

            let (v_id, w_id) = {
                let outcome_v_id = (g.node_id_by_ix(tree_v_ix), g.node_id_by_ix(w_ix));

                match outcome_v_id {
                    (Some(matched_v_id), Some(matched_w_id)) => (matched_v_id, matched_w_id),
                    (::core::option::Option::None, _)
                    | (_, ::core::option::Option::None) => continue 'inner_while_2,
                }
            };
            in_tree_by_ix[w_ix] = true;
            tree_g_ixs.push(w_ix);
            t.set_edge(v_id, w_id);

            {
                match tight_tree_frame(g, w_ix) {
                    Some(child_frame) => {
                        frames.push(child_frame);
                    },
                    ::core::option::Option::None => {},
                };
            }
        }
    }

    t.node_count()
}

#[must_use]
fn find_min_slack_edge(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    rank_by_ix: &[i128],
    in_tree_by_ix: &[bool],
) -> Option<(i128, bool)> {
    let mut best: Option<(i128, bool)> = None;
    g.for_each_edge_ix(|v_ix, w_ix, _key, lbl| {
        let in_v = match in_tree_by_ix.get(v_ix).copied() {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => false,
        };
        let in_w = match in_tree_by_ix.get(w_ix).copied() {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => false,
        };

        match {
            ::core::primitive::bool::then_some(in_v == in_w, ())
        } {
            ::core::option::Option::Some(()) => {
                return;
            },
            ::core::option::Option::None => {},
        }

        let v_rank = match rank_by_ix.get(v_ix).copied() {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0,
        };
        let w_rank = match rank_by_ix.get(w_ix).copied() {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0,
        };
        let minlen = lbl.minlen as i128;
        let slack = w_rank - v_rank - minlen;

        match &best {
            Some((best_slack, _)) if slack >= *best_slack => {}
            _ => best = Some((slack, in_v)),
        }
    });
    best
}

fn shift_ranks(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    rank_by_ix: &mut Vec<i128>,
    tree_g_ixs: &[usize],
    delta: i128,
) -> Result<(), WorkError> {
    let mut updates = Vec::with_capacity(tree_g_ixs.len());

    for &ix in tree_g_ixs {
        match {
            ::core::primitive::bool::then_some(ix >= rank_by_ix.len(), ())
        } {
            ::core::option::Option::Some(()) => {
                rank_by_ix.resize(ix + 1, 0);
            },
            ::core::option::Option::None => {},
        }

        let new_rank = rank_by_ix[ix] + delta;
        let new_rank_i32 = i32::try_from(new_rank).map_err(|_| WorkError::ArithmeticOverflow)?;
        updates.push((ix, new_rank, new_rank_i32));
    }

    for (ix, new_rank, new_rank_i32) in updates {
        rank_by_ix[ix] = new_rank;

        {
            match g.node_label_mut_by_ix(ix) {
                Some(label) => {
                    label.rank = Some(new_rank_i32);
                },
                ::core::option::Option::None => {},
            };
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingWorkControl {
        charges: Vec<usize>,
        remaining: Option<usize>,
    }

    impl RecordingWorkControl {
        fn with_limit(limit: usize) -> Self {
            Self {
                remaining: Some(limit),
                ..Self::default()
            }
        }
    }

    impl WorkControl for RecordingWorkControl {
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.charges.push(units);
            let Some(remaining) = self.remaining else {
                return Ok(());
            };
            let Some(next) = remaining.checked_sub(units) else {
                return Err(WorkError::Interrupted);
            };
            self.remaining = Some(next);
            Ok(())
        }
    }

    fn tight_chain(numeric: bool) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        let ids = if numeric {
            ["0", "1", "2", "3"]
        } else {
            ["node-0", "node-1", "node-2", "node-3"]
        };
        for (rank, id) in ids.into_iter().enumerate() {
            graph.set_node(
                id,
                NodeLabel {
                    rank: Some(i32::try_from(rank).unwrap()),
                    ..NodeLabel::default()
                },
            );
        }
        for pair in ids.windows(2) {
            graph.set_edge_with_label(
                pair[0],
                pair[1],
                EdgeLabel {
                    minlen: 1,
                    weight: 1.0,
                    ..EdgeLabel::default()
                },
            );
        }
        graph
    }

    #[test]
    fn feasible_tree_precharges_numeric_node_and_tree_adjacency_order() {
        let mut numeric = tight_chain(true);
        let mut ordinary = tight_chain(false);
        let numeric_order_work =
            checked_ordered_key_updates(numeric.node_count(), numeric.array_index_node_count())
                .unwrap();
        let numeric_adjacency_work = checked_ordered_key_updates(
            numeric.node_count(),
            numeric.pinned_array_index_adjacency_entry_count(),
        )
        .unwrap();
        let numeric_setup = feasible_tree_setup_work_units(&numeric).unwrap();
        let ordinary_setup = feasible_tree_setup_work_units(&ordinary).unwrap();
        assert_eq!(
            numeric_setup,
            ordinary_setup + numeric_order_work + numeric_adjacency_work
        );
        assert!(numeric.directed_array_index_adjacency_entry_count() > 0);

        let iteration_work = feasible_tree_iteration_work_units(&numeric).unwrap();
        let exact = checked_add(numeric_setup, iteration_work).unwrap();
        let source_nodes = numeric.node_ids();
        let source_edges = numeric.edge_keys();
        let source_ranks = source_nodes
            .iter()
            .map(|id| numeric.node(id).and_then(|node| node.rank))
            .collect::<Vec<_>>();

        let mut measured = RecordingWorkControl::default();
        let tree = feasible_tree_controlled(&mut numeric, &mut measured)
            .expect("the unbounded control admits the numeric feasible tree");
        assert_eq!(measured.charges, [numeric_setup, iteration_work]);
        assert_eq!(tree.node_count(), numeric.node_count());
        assert_eq!(
            tree.array_index_node_count(),
            numeric.array_index_node_count()
        );
        assert!(!tree.is_directed());
        assert_eq!(tree.directed_array_index_adjacency_entry_count(), 0);

        let mut ordinary_measured = RecordingWorkControl::default();
        let ordinary_tree = feasible_tree_controlled(&mut ordinary, &mut ordinary_measured)
            .expect("the unbounded control admits the ordinary feasible tree");
        assert_eq!(
            ordinary_measured.charges,
            [
                ordinary_setup,
                feasible_tree_iteration_work_units(&ordinary).unwrap()
            ]
        );
        assert_eq!(ordinary_tree.array_index_node_count(), 0);

        let mut rejected_graph = tight_chain(true);
        let mut rejected = RecordingWorkControl::with_limit(numeric_setup - 1);
        assert!(matches!(
            feasible_tree_controlled(&mut rejected_graph, &mut rejected),
            Err(WorkError::Interrupted)
        ));
        assert_eq!(rejected.charges, [numeric_setup]);
        assert_eq!(rejected.remaining, Some(numeric_setup - 1));
        assert_eq!(rejected_graph.node_ids(), source_nodes);
        assert_eq!(rejected_graph.edge_keys(), source_edges);
        assert_eq!(
            source_nodes
                .iter()
                .map(|id| rejected_graph.node(id).and_then(|node| node.rank))
                .collect::<Vec<_>>(),
            source_ranks
        );

        let mut below_graph = tight_chain(true);
        let mut below = RecordingWorkControl::with_limit(exact - 1);
        assert!(matches!(
            feasible_tree_controlled(&mut below_graph, &mut below),
            Err(WorkError::Interrupted)
        ));
        assert_eq!(below.charges, [numeric_setup, iteration_work]);
        assert_eq!(below.remaining, Some(iteration_work - 1));

        for limit in [exact, exact + 1] {
            let mut admitted_graph = tight_chain(true);
            let mut admitted = RecordingWorkControl::with_limit(limit);
            let tree = feasible_tree_controlled(&mut admitted_graph, &mut admitted)
                .expect("equal and above numeric feasible-tree budgets succeed");
            assert_eq!(tree.node_count(), admitted_graph.node_count());
        }
    }

    #[test]
    fn feasible_tree_precharges_stale_csr_slots_and_reuses_a_warm_cache() {
        let mut sparse = tight_chain(false);
        assert!(sparse.remove_edge("node-1", "node-2", None));
        sparse.set_edge_with_label(
            "node-1",
            "node-2",
            EdgeLabel {
                minlen: 1,
                weight: 1.0,
                ..EdgeLabel::default()
            },
        );
        assert_eq!(sparse.edge_count(), 3);
        assert_eq!(sparse.edge_slot_count(), 4);
        assert!(!sparse.is_adjacency_cache_current());

        let cold_setup = feasible_tree_setup_work_units(&sparse).unwrap();
        let dense = tight_chain(false);
        let dense_setup = feasible_tree_setup_work_units(&dense).unwrap();
        assert_eq!(cold_setup, dense_setup + 3);

        let mut rejected = RecordingWorkControl::with_limit(cold_setup - 1);
        assert!(matches!(
            feasible_tree_controlled(&mut sparse, &mut rejected),
            Err(WorkError::Interrupted)
        ));
        assert_eq!(rejected.charges, [cold_setup]);
        assert!(!sparse.is_adjacency_cache_current());

        let iteration_work = feasible_tree_iteration_work_units(&sparse).unwrap();
        let mut admitted =
            RecordingWorkControl::with_limit(checked_add(cold_setup, iteration_work).unwrap());
        let spanning_tree = feasible_tree_controlled(&mut sparse, &mut admitted)
            .expect("the cold CSR path fits its exact setup and iteration budget");
        assert_eq!(spanning_tree.node_count(), sparse.node_count());
        assert_eq!(admitted.remaining, Some(0));
        assert!(sparse.is_adjacency_cache_current());
        let warm_setup = feasible_tree_setup_work_units(&sparse).unwrap();
        assert_eq!(
            cold_setup,
            checked_add(
                warm_setup,
                checked_add(
                    checked_mul(sparse.edge_slot_count(), 2).unwrap(),
                    checked_mul(sparse.edge_count(), 2).unwrap(),
                )
                .unwrap()
            )
            .unwrap()
        );
    }
}
