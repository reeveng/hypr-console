//! Re-parent dummy chains in compound graphs.
//!
//! Dagre assigns dummy `"edge"` nodes to the most appropriate parent cluster based on the LCA
//! between the edge endpoints and the cluster min/max ranks. This mirrors upstream behavior.

use crate::graphlib::Graph;
use crate::work::{checked_add, checked_mul};
use crate::{EdgeLabel, GraphLabel, NodeLabel};
use crate::{NoopWorkControl, WorkControl, WorkError};
use console_core_perfect_hash::collections::FixedMap as HashMap;

pub fn parent_dummy_chains(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let mut work_control = NoopWorkControl;
    parent_dummy_chains_controlled(g, &mut work_control)
        .expect("the checked no-op Dugong work control cannot reject dummy-chain work");
}

pub(crate) fn parent_dummy_chains_controlled(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<(), WorkError> {
    work_control.charge(parent_dummy_chain_base_work_units(g)?)?;
    work_control.charge(parent_dummy_chain_path_work_units(g)?)?;
    let postorder_nums = postorder(g)?;

    let chains = g.graph().dummy_chains.clone();

    for mut v in chains {
        let node = {
            let optional_node = g.node(&v);

            match optional_node {
                Some(matched_node) => matched_node,
                ::core::option::Option::None => continue,
            }
        };
        let edge_obj = {
            let optional_edge_obj = node.edge_obj.clone();

            match optional_edge_obj {
                Some(matched_edge_obj) => matched_edge_obj,
                ::core::option::Option::None => continue,
            }
        };

        let path_data = find_path(g, &postorder_nums, &edge_obj.v, &edge_obj.w);
        let path = path_data.path;
        let lca = path_data.lca;

        let mut path_idx: usize = 0;
        let mut path_v = match (path.get(path_idx).cloned(), None) {
            (::core::option::Option::Some(present), alternate) => { ::core::mem::drop(alternate); present },
            (::core::option::Option::None, alternate) => alternate,
        };
        let mut ascending = true;

        'inner_while_2: while v != edge_obj.w {
            let rank = match g.node(&v).and_then(|n| n.rank) {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => 0,
            };

            match ::core::primitive::bool::then_some(ascending, ()) {
                ::core::option::Option::Some(()) => {
                    while path_v != lca
                        && path_v
                            .as_deref()
                            .and_then(|pv| g.node(pv))
                            .and_then(|n| n.max_rank)
                            .unwrap_or(i32::MAX / 2)
                            < rank
                    {
                        path_idx += 1;
                        path_v = match (path.get(path_idx).cloned(), None) {
                            (::core::option::Option::Some(present), alternate) => { ::core::mem::drop(alternate); present },
                            (::core::option::Option::None, alternate) => alternate,
                        };
                    }

                    match {
                        ::core::primitive::bool::then_some(path_v == lca, ())
                    } {
                        ::core::option::Option::Some(()) => {
                            ascending = false;
                        },
                        ::core::option::Option::None => {},
                    }
                },
                ::core::option::Option::None => {},
            }

            match {
                ::core::primitive::bool::then_some(!ascending, ())
            } {
                ::core::option::Option::Some(()) => {
                    while path_idx + 1 < path.len()
                        && path
                            .get(path_idx + 1)
                            .and_then(|p| p.as_ref())
                            .and_then(|pv| g.node(pv))
                            .and_then(|n| n.min_rank)
                            .unwrap_or(i32::MIN / 2)
                            <= rank
                    {
                        path_idx += 1;
                    }

                    path_v = match (path.get(path_idx).cloned(), None) {
                        (::core::option::Option::Some(present), alternate) => { ::core::mem::drop(alternate); present },
                        (::core::option::Option::None, alternate) => alternate,
                    };
                },
                ::core::option::Option::None => {},
            }

            match &path_v {
                Some(parent) => {
                    g.set_parent_ref(v.as_str(), parent.as_str());
                }
                None => {
                    g.clear_parent(&v);
                }
            }

            let next = {
                let optional_next = g.first_successor(&v).map(|s| s.to_string());

                match optional_next {
                    Some(matched_next) => matched_next,
                    ::core::option::Option::None => break 'inner_while_2,
                }
            };
            v = next;
        }
    }

    Ok(())
}

fn parent_dummy_chain_base_work_units(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<usize, WorkError> {
    checked_add(checked_mul(g.node_count(), 3)?, g.edge_count())
}

fn parent_dummy_chain_path_work_units(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<usize, WorkError> {
    let mut depths: HashMap<String, usize> = HashMap::default();
    let mut stack: Vec<(String, usize)> = g
        .children_root()
        .into_iter()
        .rev()
        .map(|id| (id.to_string(), 0))
        .collect();

    while let Some((id, depth)) = stack.pop() {
        depths.insert(id.clone(), depth);
        let child_depth = checked_add(depth, 1)?;
        stack.extend(
            g.children(&id)
                .into_iter()
                .rev()
                .map(|child| (child.to_string(), child_depth)),
        );
    }

    let mut path_work = 0usize;

    for chain in &g.graph().dummy_chains {
        let edge = {
            let optional_edge = g.node(chain).and_then(|node| node.edge_obj.as_ref());

            match optional_edge {
                Some(matched_edge) => matched_edge,
                ::core::option::Option::None => continue,
            }
        };
        let endpoint_depth = checked_add(
            match depths.get(&edge.v).copied() {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => 0,
            },
            match depths.get(&edge.w).copied() {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => 0,
            },
        )?;
        path_work = checked_add(path_work, checked_add(endpoint_depth, 2)?)?;
    }

    Ok(path_work)
}

struct PostorderNum {
    low: usize,
    lim: usize,
}

struct PathData {
    path: Vec<Option<String>>,
    lca: Option<String>,
}

#[must_use]
fn find_path(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    postorder_nums: &HashMap<String, PostorderNum>,
    v: &str,
    w: &str,
) -> PathData {
    let v_po = &postorder_nums[v];
    let w_po = &postorder_nums[w];
    let low = v_po.low.min(w_po.low);
    let lim = v_po.lim.max(w_po.lim);

    // Traverse up from v to find the LCA.
    let mut v_path: Vec<Option<String>> = Vec::new();
    let mut parent = Some(v.to_string());
    let lca: Option<String>;

    loop {
        parent = parent
            .as_deref()
            .and_then(|p| g.parent(p))
            .map(|s| s.to_string());
        v_path.push(parent.clone());

        let Some(p) = parent.clone() else {
            lca = None;
            break;
        };

        let po = &postorder_nums[&p];

        match {
            ::core::primitive::bool::then_some(!(po.low > low || lim > po.lim), ())
        } {
            ::core::option::Option::Some(()) => {
                lca = Some(p);
                break;
            },
            ::core::option::Option::None => {},
        }
    }

    // Traverse from w to LCA.
    let mut w_path: Vec<Option<String>> = Vec::new();
    let mut cur = w.to_string();

    loop {
        let p = g.parent(&cur).map(|s| s.to_string());

        match {
            ::core::primitive::bool::then_some(p == lca, ())
        } {
            ::core::option::Option::Some(()) => {
                break;
            },
            ::core::option::Option::None => {},
        }

        match p {
            ::core::option::Option::None => {
                break;
            },
            ::core::option::Option::Some(_) => {},
        }

        w_path.push(p.clone());

        match p {
            Some(p) => {
                cur = p;
            },
            ::core::option::Option::None => {
                break;
            },
        }
    }

    let mut path = v_path;
    w_path.reverse();
    path.extend(w_path);
    PathData { path, lca }
}

fn postorder(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<HashMap<String, PostorderNum>, WorkError> {
    let mut result: HashMap<String, PostorderNum> = HashMap::default();
    let mut lim: usize = 0;

    let mut stack: Vec<(String, bool, usize)> = g
        .children_root()
        .into_iter()
        .rev()
        .map(|v| (v.to_string(), false, 0))
        .collect();

    while let Some((v, expanded, low)) = stack.pop() {
        match ::core::primitive::bool::then_some(expanded, ()) {
            ::core::option::Option::Some(()) => {
                result.insert(v, PostorderNum { low, lim });
                lim = checked_add(lim, 1)?;
                continue;
            },
            ::core::option::Option::None => {},
        }

        let low = lim;
        stack.push((v.clone(), true, low));

        for child in g.children(&v).into_iter().rev() {
            stack.push((child.to_string(), false, 0));
        }
    }

    Ok(result)
}
