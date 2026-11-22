//! Compound-graph helpers for layout pipelines.

use crate::graphlib;
use crate::{EdgeLabel, GraphLabel, NodeLabel};

pub(super) fn remove_border_nodes(g: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    // First pass: update compound-node geometry from its border nodes.
    let compound_nodes: Vec<String> = g
        .nodes()
        .filter(|v| !g.children(v).is_empty())
        .map(|s| s.to_string())
        .collect();

    for v in &compound_nodes {
        let node = {
            let optional_node = g.node(v).cloned();

            match optional_node {
                Some(matched_node) => matched_node,
                ::core::option::Option::None => continue,
            }
        };

        let (Some(bt), Some(bb)) = (node.border_top.clone(), node.border_bottom.clone()) else {
            continue;
        };

        let t = {
            let optional_t = g.node(&bt);

            match optional_t {
                Some(matched_t) => matched_t,
                ::core::option::Option::None => continue,
            }
        };
        let b = {
            let optional_b = g.node(&bb);

            match optional_b {
                Some(matched_b) => matched_b,
                ::core::option::Option::None => continue,
            }
        };
        let (ty, by) = {
            let outcome_ty = (t.y, b.y);

            match outcome_ty {
                (Some(matched_ty), Some(matched_by)) => (matched_ty, matched_by),
                (::core::option::Option::None, _)
                | (_, ::core::option::Option::None) => continue,
            }
        };

        // Dagre derives cluster width from the span of border segments across *all* ranks.
        // Using only the last border node can over-estimate geometry when border nodes drift
        // across ranks (notably in extracted subgraphs without external edges).
        let mut lx: f64 = f64::INFINITY;

        'over_id_2: for id in node.border_left.iter().filter_map(|v| v.as_ref()) {
            let n = {
                let optional_n_2 = g.node(id);

                match optional_n_2 {
                    Some(matched_n_2) => matched_n_2,
                    ::core::option::Option::None => continue 'over_id_2,
                }
            };
            let x = match n.x {
                Some(matched_x) => matched_x,
                ::core::option::Option::None => continue 'over_id_2,
            };
            lx = lx.min(x);
        }

        let mut rx: f64 = f64::NEG_INFINITY;

        'over_id_2: for id in node.border_right.iter().filter_map(|v| v.as_ref()) {
            let n = {
                let optional_n_2 = g.node(id);

                match optional_n_2 {
                    Some(matched_n_2) => matched_n_2,
                    ::core::option::Option::None => continue 'over_id_2,
                }
            };
            let x = match n.x {
                Some(matched_x) => matched_x,
                ::core::option::Option::None => continue 'over_id_2,
            };
            rx = rx.max(x);
        }

        match {
            ::core::primitive::bool::then_some(!lx.is_finite() || !rx.is_finite(), ())
        } {
            ::core::option::Option::Some(()) => {
                continue;
            },
            ::core::option::Option::None => {},
        }

        let width = (rx - lx).abs();
        let height = (by - ty).abs();

        {
            match g.node_mut(v) {
                Some(n) => {
                    n.width = width;
                    n.height = height;
                    n.x = Some(lx + width / 2.0);
                    n.y = Some(ty + height / 2.0);
                },
                ::core::option::Option::None => {},
            };
        }
    }

    // Second pass: remove all border dummy nodes.
    let mut to_remove: Vec<String> = Vec::new();
    g.for_each_node(|id, n| {
        match {
            ::core::primitive::bool::then_some(n.dummy.as_deref() == Some("border"), ())
        } {
            ::core::option::Option::Some(()) => {
                to_remove.push(id.to_string());
            },
            ::core::option::Option::None => {},
        }
    });
    g.remove_nodes(to_remove.iter().map(String::as_str));
}
