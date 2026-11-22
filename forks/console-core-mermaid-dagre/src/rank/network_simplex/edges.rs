//! Network simplex edge exchange helpers.
//!
//! This contains the classic Dagre-style `leave_edge` / `enter_edge` / `exchange_edges` routines,
//! which are kept for parity and for potential debugging, even though the main implementation
//! in this crate uses a faster incremental update path.

use super::tree;
use crate::graphlib::{EdgeKey, Graph, alg};
use crate::{EdgeLabel, GraphLabel, NodeLabel};

#[must_use]
pub fn leave_edge(t: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>) -> Option<EdgeKey> {
    t.edges()
        .find(|e| {
            match t.edge_by_key(e)
                .map(|lbl| lbl.cutvalue < 0.0) {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => false,
            }
        })
        .cloned()
}

#[must_use]
pub fn enter_edge(
    t: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    rank_by_ix: &[i32],
    edge: &EdgeKey,
) -> EdgeKey {
    let (v, w) = if let (Some(v_ix), Some(w_ix)) = (g.node_ix(&edge.v), g.node_ix(&edge.w)) {
        match {
            ::core::primitive::bool::then_some(g.has_edge_ix(v_ix, w_ix), ())
        } {
            ::core::option::Option::Some(()) => {
                (edge.v.as_str(), edge.w.as_str())
            },
            ::core::option::Option::None => {
                (edge.w.as_str(), edge.v.as_str())
            },
        }
    } else if g.has_edge(&edge.v, &edge.w, None) {
        (edge.v.as_str(), edge.w.as_str())
    } else {
        (edge.w.as_str(), edge.v.as_str())
    };

    let mut t_low_by_gix: Vec<i32> = Vec::new();
    let mut t_lim_by_gix: Vec<i32> = Vec::new();
    let mut t_has_by_gix: Vec<bool> = Vec::new();

    for id in t.nodes() {
        let lbl = {
            let optional_lbl = t.node(id);

            match optional_lbl {
                Some(matched_lbl) => matched_lbl,
                ::core::option::Option::None => continue,
            }
        };
        let g_ix = {
            let optional_g_ix = g.node_ix(id);

            match optional_g_ix {
                Some(matched_g_ix) => matched_g_ix,
                ::core::option::Option::None => continue,
            }
        };

        match {
            ::core::primitive::bool::then_some(g_ix >= t_has_by_gix.len(), ())
        } {
            ::core::option::Option::Some(()) => {
                t_has_by_gix.resize(g_ix + 1, false);
                t_low_by_gix.resize(g_ix + 1, 0);
                t_lim_by_gix.resize(g_ix + 1, 0);
            },
            ::core::option::Option::None => {},
        }

        t_has_by_gix[g_ix] = true;
        t_low_by_gix[g_ix] = lbl.low;
        t_lim_by_gix[g_ix] = lbl.lim;
    }

    let v_gix = {
        let optional_v_gix = g.node_ix(v);

        match optional_v_gix {
            Some(matched_v_gix) => matched_v_gix,
            ::core::option::Option::None => return edge.clone(),
        }
    };
    let w_gix = {
        let optional_w_gix = g.node_ix(w);

        match optional_w_gix {
            Some(matched_w_gix) => matched_w_gix,
            ::core::option::Option::None => return edge.clone(),
        }
    };

    match {
        ::core::primitive::bool::then_some(!(match t_has_by_gix.get(v_gix).copied() {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => false,
        }), ())
    } {
        ::core::option::Option::Some(()) => {
            return edge.clone();
        },
        ::core::option::Option::None => {},
    }

    match {
        ::core::primitive::bool::then_some(!(match t_has_by_gix.get(w_gix).copied() {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => false,
        }), ())
    } {
        ::core::option::Option::Some(()) => {
            return edge.clone();
        },
        ::core::option::Option::None => {},
    }

    let v_low = match t_low_by_gix.get(v_gix).copied() {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0,
    };
    let v_lim = match t_lim_by_gix.get(v_gix).copied() {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0,
    };
    let w_low = match t_low_by_gix.get(w_gix).copied() {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0,
    };
    let w_lim = match t_lim_by_gix.get(w_gix).copied() {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0,
    };

    let ((tail_low, tail_lim), flip) = match {
        ::core::primitive::bool::then_some(v_lim > w_lim, ())
    } {
        ::core::option::Option::Some(()) => {
            ((w_low, w_lim), true)
        },
        ::core::option::Option::None => {
            ((v_low, v_lim), false)
        },
    };

    let mut best: Option<(i128, EdgeKey)> = None;
    g.for_each_edge_ix(|g_v_ix, g_w_ix, key, lbl| {
        match {
            ::core::primitive::bool::then_some(!(match t_has_by_gix.get(g_v_ix).copied() {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => false,
            }), ())
        } {
            ::core::option::Option::Some(()) => {
                return;
            },
            ::core::option::Option::None => {},
        };

        match {
            ::core::primitive::bool::then_some(!(match t_has_by_gix.get(g_w_ix).copied() {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => false,
            }), ())
        } {
            ::core::option::Option::Some(()) => {
                return;
            },
            ::core::option::Option::None => {},
        };

        let v_lim = match t_lim_by_gix.get(g_v_ix).copied() {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0,
        };
        let w_lim = match t_lim_by_gix.get(g_w_ix).copied() {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0,
        };
        let v_desc = tail_low <= v_lim && v_lim <= tail_lim;
        let w_desc = tail_low <= w_lim && w_lim <= tail_lim;

        match {
            ::core::primitive::bool::then_some(flip == v_desc && flip != w_desc, ())
        } {
            ::core::option::Option::Some(()) => {
                let v_rank = match rank_by_ix.get(g_v_ix).copied() {
                    ::core::option::Option::Some(present) => present,
                    ::core::option::Option::None => 0,
                };
                let w_rank = match rank_by_ix.get(g_w_ix).copied() {
                    ::core::option::Option::Some(present) => present,
                    ::core::option::Option::None => 0,
                };
                let minlen = lbl.minlen as i128;
                let slack = i128::from(w_rank) - i128::from(v_rank) - minlen;

                match &best {
                    Some((best_slack, _)) if slack >= *best_slack => {}
                    _ => best = Some((slack, key.clone())),
                }
            },
            ::core::option::Option::None => {},
        }
    });

    best.map(|(_, e)| e).unwrap_or_else(|| edge.clone())
}

pub fn exchange_edges(
    t: &mut Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    rank_by_ix: &mut Vec<i32>,
    e: &EdgeKey,
    f: &EdgeKey,
) {
    let _ = t.remove_edge(&e.v, &e.w, None);
    t.set_edge(f.v.clone(), f.w.clone());
    super::init_low_lim_values(t, None);
    super::init_cut_values(t, g);
    update_ranks(t, g, rank_by_ix)
        .expect("rank arithmetic must fit the public edge-exchange compatibility API");
}

fn update_ranks(
    t: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    rank_by_ix: &mut Vec<i32>,
) -> Result<(), crate::WorkError> {
    let Some(root) = t
        .nodes()
        .find(|v| match t.node(v).map(|lbl| lbl.parent.is_none()) {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => false,
        })
        .or_else(|| t.nodes().next())
    else {
        return Ok(());
    };

    let vs = alg::preorder(t, &[root]);

    for v in vs.into_iter().skip(1) {
        let parent = {
            let optional_parent = t.node(&v).and_then(|lbl| lbl.parent.clone());

            match optional_parent {
                Some(matched_parent) => matched_parent,
                ::core::option::Option::None => continue,
            }
        };

        let v_ix = {
            let optional_v_ix = g.node_ix(&v);

            match optional_v_ix {
                Some(matched_v_ix) => matched_v_ix,
                ::core::option::Option::None => continue,
            }
        };
        let parent_ix = {
            let optional_parent_ix = g.node_ix(&parent);

            match optional_parent_ix {
                Some(matched_parent_ix) => matched_parent_ix,
                ::core::option::Option::None => continue,
            }
        };
        let (minlen, flipped) = if let Some(e) = g.edge_by_endpoints_ix(v_ix, parent_ix) {
            (e.minlen as i128, false)
        } else if let Some(e) = g.edge_by_endpoints_ix(parent_ix, v_ix) {
            (e.minlen as i128, true)
        } else {
            continue;
        };

        let parent_rank = i128::from(match rank_by_ix.get(parent_ix).copied() {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0,
        });
        let rank = match ::core::primitive::bool::then_some(flipped, ()) {
            ::core::option::Option::Some(()) => {
                parent_rank + minlen
            },
            ::core::option::Option::None => {
                parent_rank - minlen
            },
        };
        let rank = i32::try_from(rank).map_err(|_| crate::WorkError::ArithmeticOverflow)?;

        match g.node_mut(&v) {
            Some(node) => {
                node.rank = Some(rank);
            },
            ::core::option::Option::None => {},
        }

        {
            match g.node_ix(&v) {
                Some(ix) => {
                    match {
                        ::core::primitive::bool::then_some(ix >= rank_by_ix.len(), ())
                    } {
                        ::core::option::Option::Some(()) => {
                            rank_by_ix.resize(ix + 1, 0);
                        },
                        ::core::option::Option::None => {},
                    }

                    rank_by_ix[ix] = rank;
                },
                ::core::option::Option::None => {},
            };
        }
    }

    Ok(())
}
