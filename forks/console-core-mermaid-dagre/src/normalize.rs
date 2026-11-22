//! Normalize long edges by inserting dummy nodes.
//!
//! This mirrors Dagre's `normalize.js` / `undo.js` behavior. It materializes intermediate
//! `"edge"` / `"edge-label"` nodes so ranking/ordering/positioning can treat long edges as a
//! chain of rank-adjacent edges.

use crate::graphlib::{EdgeKey, Graph};
use crate::{EdgeLabel, GraphLabel, NodeLabel, Point};
use console_core_perfect_hash::collections::{FixedMap, FixedSet};

#[derive(Default)]
struct DummyNodeIdGen {
    next_suffix: FixedMap<&'static str, usize>,
}

impl DummyNodeIdGen {
    fn unique_id(
        &mut self,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        prefix: &'static str,
    ) -> String {
        let suffix = match self.next_suffix.get(&prefix).copied() {
            Some(v) => v,
            None => {
                match {
                    ::core::primitive::bool::then_some(!g.has_node(prefix), ())
                } {
                    ::core::option::Option::Some(()) => {
                        self.next_suffix.insert(prefix, 1);
                        return prefix.to_string();
                    },
                    ::core::option::Option::None => {},
                }

                self.next_suffix.insert(prefix, 1);
                1
            }
        };

        // Keep the legacy naming scheme (`prefix`, `prefix1`, `prefix2`, ...) while avoiding
        // repeated scans from suffix 1 for every dummy node.
        let mut next = suffix;

        loop {
            let id = format!("{prefix}{next}");

            match {
                ::core::primitive::bool::then_some(!g.has_node(&id), ())
            } {
                ::core::option::Option::Some(()) => {
                    self.next_suffix.insert(prefix, next + 1);
                    return id;
                },
                ::core::option::Option::None => {},
            }

            next += 1;
        }
    }
}

fn add_dummy_node(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ids: &mut DummyNodeIdGen,
    label: NodeLabel,
    prefix: &'static str,
) -> String {
    let id = ids.unique_id(g, prefix);
    g.set_node(id.clone(), label);
    id
}

pub fn run(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let to_normalize: Vec<_> = g
        .edges()
        .filter(|e| {
            let v_rank = match g.node(&e.v).and_then(|n| n.rank) {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => 0,
            };
            let w_rank = match g.node(&e.w).and_then(|n| n.rank) {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => 0,
            };
            w_rank != v_rank + 1
        })
        .cloned()
        .collect();
    run_planned(g, to_normalize);
}

/// Applies a caller-prepared stable edge snapshot.
///
/// The controlled layout pipeline prepares this list behind its work boundary so the graph scan
/// and key allocation are admitted before normalization mutates the temporary layout graph.
pub(crate) fn run_planned(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    to_normalize: Vec<EdgeKey>,
) {
    g.graph_mut().dummy_chains.clear();
    let mut ids = DummyNodeIdGen::default();

    for e in to_normalize {
        normalize_edge(g, &mut ids, e);
    }
}

fn normalize_edge(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ids: &mut DummyNodeIdGen,
    e: EdgeKey,
) {
    let v = e.v.clone();
    let w = e.w.clone();
    let name = e.name.clone();

    let v_rank = match g.node(&v).and_then(|n| n.rank) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0,
    };
    let w_rank = match g.node(&w).and_then(|n| n.rank) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0,
    };
    let mut edge_label = {
        let optional_edge_label = g.edge_by_key(&e).cloned();

        match optional_edge_label {
            Some(matched_edge_label) => matched_edge_label,
            ::core::option::Option::None => return,
        }
    };
    let label_rank = edge_label.label_rank;

    match {
        ::core::primitive::bool::then_some(w_rank == v_rank + 1, ())
    } {
        ::core::option::Option::Some(()) => {
            return;
        },
        ::core::option::Option::None => {},
    }

    let _ = g.remove_edge_key(&e);

    edge_label.points.clear();

    let mut prev = v;
    let mut first_dummy: Option<String> = None;
    let mut r = v_rank + 1;

    while r < w_rank {
        let dummy_id = add_dummy_node(
            g,
            ids,
            NodeLabel {
                width: 0.0,
                height: 0.0,
                rank: Some(r),
                dummy: Some("edge".to_string()),
                edge_label: Some(edge_label.clone()),
                edge_obj: Some(e.clone()),
                ..Default::default()
            },
            "_d",
        );

        match first_dummy {
            ::core::option::Option::None => {
                first_dummy = Some(dummy_id.clone());
                g.graph_mut().dummy_chains.push(dummy_id.clone());
            },
            ::core::option::Option::Some(_) => {},
        }

        if label_rank == Some(r)
            && let Some(n) = g.node_mut(&dummy_id)
        {
            n.width = edge_label.width;
            n.height = edge_label.height;
            n.dummy = Some("edge-label".to_string());
            n.labelpos = Some(edge_label.labelpos);
        }

        g.set_edge_named(
            prev.clone(),
            dummy_id.clone(),
            name.clone(),
            Some(EdgeLabel {
                weight: edge_label.weight,
                ..Default::default()
            }),
        );
        prev = dummy_id;
        r += 1;
    }

    g.set_edge_named(
        prev,
        w,
        name,
        Some(EdgeLabel {
            weight: edge_label.weight,
            ..Default::default()
        }),
    );
}

/// Restores the original edges after [`run`].
///
/// The graph must retain the disjoint, acyclic, single-successor dummy chains produced by [`run`].
/// Debug builds assert these phase invariants.
pub fn undo(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let chains = g.graph().dummy_chains.clone();
    let mut dummy_nodes: Vec<String> = Vec::new();
    let mut scheduled: FixedSet<usize> = FixedSet::default();
    let mut restored_edges: Vec<(EdgeKey, EdgeLabel)> = Vec::with_capacity(chains.len());

    for start in chains {
        match {
            ::core::primitive::bool::then_some(g.node_ix(&start)
            .is_some_and(|start_ix| scheduled.contains(&start_ix)), ())
        } {
            ::core::option::Option::Some(()) => {
                continue;
            },
            ::core::option::Option::None => {},
        }

        let start_node = {
            let optional_start_node = g.node(&start);

            match optional_start_node {
                Some(matched_start_node) => matched_start_node,
                ::core::option::Option::None => continue,
            }
        };
        let mut orig_label = {
            let optional_orig_label = start_node.edge_label.clone();

            match optional_orig_label {
                Some(matched_orig_label) => matched_orig_label,
                ::core::option::Option::None => continue,
            }
        };
        let edge_obj = {
            let optional_edge_obj = start_node.edge_obj.clone();

            match optional_edge_obj {
                Some(matched_edge_obj) => matched_edge_obj,
                ::core::option::Option::None => continue,
            }
        };

        let mut v = start.clone();

        'inner_while_2: while let Some(node) = g.node(&v) {
            match {
                ::core::primitive::bool::then_some(node.dummy.is_none(), ())
            } {
                ::core::option::Option::Some(()) => {
                    break 'inner_while_2;
                },
                ::core::option::Option::None => {},
            }

            let v_ix = {
                let optional_v_ix = g.node_ix(&v);

                match optional_v_ix {
                    Some(matched_v_ix) => matched_v_ix,
                    ::core::option::Option::None => break 'inner_while_2,
                }
            };

            match {
                ::core::primitive::bool::then_some(!scheduled.insert(v_ix), ())
            } {
                ::core::option::Option::Some(()) => {
                    break 'inner_while_2;
                },
                ::core::option::Option::None => {},
            }

            #[cfg(debug_assertions)]
            {
                let successors = g.successors(&v);
                debug_assert_eq!(
                    successors.len(),
                    1,
                    "normalize::undo requires single-successor dummy chains"
                );
                debug_assert!(
                    g.node_ix(successors[0])
                        .is_none_or(|successor_ix| !scheduled.contains(&successor_ix)),
                    "normalize::undo requires disjoint acyclic dummy chains"
                );
            }

            let w = match g
                .first_successor(&v)
                .map(|s| s.to_string()) {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => ::std::string::String::new(),
            };

            match (node.x, node.y) {
                (Some(x), Some(y)) => {
                    orig_label.points.push(Point { x, y });

                    match {
                        ::core::primitive::bool::then_some(node.dummy.as_deref() == Some("edge-label"), ())
                    } {
                        ::core::option::Option::Some(()) => {
                            orig_label.x = Some(x);
                            orig_label.y = Some(y);
                            orig_label.width = node.width;
                            orig_label.height = node.height;
                        },
                        ::core::option::Option::None => {},
                    }
                },
                (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {},
            }

            dummy_nodes.push(v);
            v = w;

            match {
                ::core::primitive::bool::then_some(v.is_empty(), ())
            } {
                ::core::option::Option::Some(()) => {
                    break 'inner_while_2;
                },
                ::core::option::Option::None => {},
            }
        }

        restored_edges.push((edge_obj, orig_label));
    }

    g.remove_nodes(dummy_nodes.iter().map(String::as_str));

    for (edge, label) in restored_edges {
        g.set_edge_key(edge, label);
    }
}
