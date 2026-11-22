//! Nesting graph construction for compound graphs.
//!
//! This mirrors Dagre's `nesting-graph.js`: it creates a synthetic root, adds border nodes for
//! clusters, and injects nesting edges so the ranker sees a connected graph.

use crate::graphlib::{EdgeKey, Graph};
use crate::work::{checked_add, checked_mul, checked_unparented_leaf_parent_batch_work};
use crate::{EdgeLabel, GraphLabel, NodeLabel};
use crate::{NoopWorkControl, WorkControl, WorkError};
use console_core_perfect_hash::collections::FixedMap;
use console_core_perfect_hash::collections::FixedSet;
use std::collections::VecDeque;

#[derive(Default)]
struct DummyNodeIdGen {
    next_suffix: FixedMap<&'static str, usize>,
}

impl DummyNodeIdGen {
    fn unique_id(
        &mut self,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        prefix: &'static str,
    ) -> Result<String, WorkError> {
        let suffix = match self.next_suffix.get(&prefix).copied() {
            Some(v) => v,
            None => {
                match {
                    ::core::primitive::bool::then_some(!g.has_node(prefix), ())
                } {
                    ::core::option::Option::Some(()) => {
                        self.next_suffix.insert(prefix, 1);
                        return Ok(prefix.to_string());
                    },
                    ::core::option::Option::None => {},
                }

                self.next_suffix.insert(prefix, 1);
                1
            }
        };

        // Keep the exact legacy naming scheme (`prefix`, `prefix1`, `prefix2`, ...) but avoid
        // scanning from `1` on every call (which is O(n^2) with repeated allocations).
        let mut next = suffix;

        loop {
            let id = format!("{prefix}{next}");

            match {
                ::core::primitive::bool::then_some(!g.has_node(&id), ())
            } {
                ::core::option::Option::Some(()) => {
                    self.next_suffix.insert(prefix, checked_add(next, 1)?);
                    return Ok(id);
                },
                ::core::option::Option::None => {},
            }

            next = checked_add(next, 1)?;
        }
    }
}

fn add_dummy_node(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ids: &mut DummyNodeIdGen,
    dummy: &str,
    mut label: NodeLabel,
    name: &'static str,
) -> Result<String, WorkError> {
    let id = ids.unique_id(g, name)?;
    label.dummy = Some(dummy.to_string());
    g.set_node(id.clone(), label);
    Ok(id)
}

fn add_border_node(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ids: &mut DummyNodeIdGen,
    prefix: &'static str,
) -> Result<String, WorkError> {
    add_dummy_node(
        g,
        ids,
        "border",
        NodeLabel {
            width: 0.0,
            height: 0.0,
            ..Default::default()
        },
        prefix,
    )
}

fn tree_depths(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<FixedMap<String, usize>, WorkError> {
    let mut out: FixedMap<String, usize> = FixedMap::default();
    let mut stack: Vec<(String, usize)> = g
        .children_root()
        .into_iter()
        .rev()
        .map(|v| (v.to_string(), 1))
        .collect();

    while let Some((v, depth)) = stack.pop() {
        out.insert(v.clone(), depth);
        let children: Vec<String> = g.children_iter(&v).map(|s| s.to_string()).collect();

        for child in children.into_iter().rev() {
            stack.push((child, checked_add(depth, 1)?));
        }
    }

    Ok(out)
}

enum NestingDfsFrame {
    Enter(String),
    LinkChild {
        parent: String,
        top: String,
        bottom: String,
        child: String,
    },
    LinkRoot {
        node: String,
        top: String,
    },
}

fn add_root_leaf_edge(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ctx: &NestingDfsCtx<'_>,
    v: &str,
) {
    match {
        ::core::primitive::bool::then_some(v != ctx.root, ())
    } {
        ::core::option::Option::Some(()) => {
            g.set_edge_with_label(
                ctx.root,
                v,
                EdgeLabel {
                    weight: 0.0,
                    minlen: ctx.node_sep,
                    ..Default::default()
                },
            );
        },
        ::core::option::Option::None => {},
    }
}

fn add_child_nesting_edges(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ctx: &NestingDfsCtx<'_>,
    parent: &str,
    top: &str,
    bottom: &str,
    child: &str,
) -> Result<(), WorkError> {
    let child_node = g.node(child).cloned().unwrap_or_default();
    let child_top = child_node
        .border_top
        .as_deref()
        .unwrap_or(child)
        .to_string();
    let child_bottom = child_node
        .border_bottom
        .as_deref()
        .unwrap_or(child)
        .to_string();
    let this_weight = match {
        ::core::primitive::bool::then_some(child_node.border_top.is_some(), ())
    } {
        ::core::option::Option::Some(()) => {
            ctx.weight
        },
        ::core::option::Option::None => {
            2.0 * ctx.weight
        },
    };
    let minlen = match {
        ::core::primitive::bool::then_some(child_top != child_bottom, ())
    } {
        ::core::option::Option::Some(()) => {
            1usize
        },
        ::core::option::Option::None => {
            let dv = match ctx.depths.get(parent).copied() {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => 1,
            };
            checked_add(ctx.height.saturating_sub(dv), 1)?
        },
    };

    g.set_edge_with_label(
        top.to_string(),
        child_top,
        EdgeLabel {
            weight: this_weight,
            minlen,
            nesting_edge: true,
            ..Default::default()
        },
    );
    g.set_edge_with_label(
        child_bottom,
        bottom.to_string(),
        EdgeLabel {
            weight: this_weight,
            minlen,
            nesting_edge: true,
            ..Default::default()
        },
    );
    Ok(())
}

fn add_root_cluster_edge(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ctx: &NestingDfsCtx<'_>,
    v: &str,
    top: &str,
) -> Result<(), WorkError> {
    match {
        ::core::primitive::bool::then_some(g.parent(v).is_none(), ())
    } {
        ::core::option::Option::Some(()) => {
            let dv = match ctx.depths.get(v).copied() {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => 1,
            };
            g.set_edge_with_label(
                ctx.root,
                top,
                EdgeLabel {
                    weight: 0.0,
                    minlen: checked_add(ctx.height, dv)?,
                    nesting_edge: true,
                    ..Default::default()
                },
            );
        },
        ::core::option::Option::None => {},
    }

    Ok(())
}

fn nesting_dfs(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ctx: &NestingDfsCtx<'_>,
    ids: &mut DummyNodeIdGen,
    parent_assignments: &mut Vec<(usize, usize)>,
    root_child: String,
) -> Result<(), WorkError> {
    let mut stack = vec![NestingDfsFrame::Enter(root_child)];

    while let Some(frame) = stack.pop() {
        match frame {
            NestingDfsFrame::Enter(v) => {
                let children: Vec<String> = g.children_iter(&v).map(|s| s.to_string()).collect();

                match {
                    ::core::primitive::bool::then_some(children.is_empty(), ())
                } {
                    ::core::option::Option::Some(()) => {
                        add_root_leaf_edge(g, ctx, &v);
                        continue;
                    },
                    ::core::option::Option::None => {},
                }

                let top = add_border_node(g, ids, "_bt")?;
                let bottom = add_border_node(g, ids, "_bb")?;
                let parent_ix = g
                    .node_ix(&v)
                    .expect("the nesting parent must remain live while adding borders");
                let top_ix = g
                    .node_ix(&top)
                    .expect("a freshly inserted top border must have a node index");
                let bottom_ix = g
                    .node_ix(&bottom)
                    .expect("a freshly inserted bottom border must have a node index");
                parent_assignments.push((top_ix, parent_ix));
                parent_assignments.push((bottom_ix, parent_ix));

                match g.node_mut(&v) {
                    Some(lbl) => {
                        lbl.border_top = Some(top.clone());
                    },
                    ::core::option::Option::None => {},
                }

                match g.node_mut(&v) {
                    Some(lbl) => {
                        lbl.border_bottom = Some(bottom.clone());
                    },
                    ::core::option::Option::None => {},
                }

                stack.push(NestingDfsFrame::LinkRoot {
                    node: v.clone(),
                    top: top.clone(),
                });

                for child in children.into_iter().rev() {
                    stack.push(NestingDfsFrame::LinkChild {
                        parent: v.clone(),
                        top: top.clone(),
                        bottom: bottom.clone(),
                        child: child.clone(),
                    });
                    stack.push(NestingDfsFrame::Enter(child));
                }
            }
            NestingDfsFrame::LinkChild {
                parent,
                top,
                bottom,
                child,
            } => add_child_nesting_edges(g, ctx, &parent, &top, &bottom, &child)?,
            NestingDfsFrame::LinkRoot { node, top } => add_root_cluster_edge(g, ctx, &node, &top)?,
        }
    }

    Ok(())
}

struct NestingDfsCtx<'a> {
    root: &'a str,
    node_sep: usize,
    weight: f64,
    height: usize,
    depths: &'a FixedMap<String, usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NestingWorkPlan {
    execution_work_units: usize,
    border_parent_assignment_count: usize,
}

fn checked_nesting_work_plan(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<NestingWorkPlan, WorkError> {
    let mut clusters = 0usize;
    let mut child_links = 0usize;
    let mut hierarchy_order_slots = g.root_child_order_slot_count();
    let mut arithmetic_error = None;

    g.for_each_node(|id, _node| {
        match arithmetic_error {
            ::core::option::Option::Some(_) => {
                return;
            },
            ::core::option::Option::None => {},
        }

        let children = g.child_count(id);
        let result = (|| {
            match children {
                0 => {},
                _ => {
                    clusters = checked_add(clusters, 1)?;
                    child_links = checked_add(child_links, children)?;
                },
            }

            hierarchy_order_slots =
                checked_add(hierarchy_order_slots, g.child_order_slot_count(id))?;
            Ok(())
        })();

        match result {
            Err(error) => {
                arithmetic_error = Some(error);
            },
            ::core::result::Result::Ok(_) => {},
        }
    });

    match arithmetic_error {
        Some(error) => {
            return Err(error);
        },
        ::core::option::Option::None => {},
    }

    let border_parent_assignment_count = checked_mul(clusters, 2)?;
    let derived_nodes = checked_add(1, border_parent_assignment_count)?;
    let derived_edges = checked_add(
        checked_mul(child_links, 2)?,
        checked_mul(g.node_count(), 2)?,
    )?;
    let future_node_slots = checked_add(g.node_slot_count(), derived_nodes)?;
    let parent_work = checked_unparented_leaf_parent_batch_work(
        future_node_slots,
        border_parent_assignment_count,
        0,
    )?;
    let existing_node_work = checked_mul(g.node_order_slot_count(), 5)?;
    let derived_node_work = checked_mul(derived_nodes, 8)?;
    let hierarchy_work = checked_mul(hierarchy_order_slots, 2)?;
    let node_work = checked_add(
        checked_add(existing_node_work, derived_node_work)?,
        hierarchy_work,
    )?;
    let edge_work = checked_mul(checked_add(g.edge_slot_count(), derived_edges)?, 4)?;
    let execution_work_units = checked_add(checked_add(node_work, edge_work)?, parent_work)?;

    Ok(NestingWorkPlan {
        execution_work_units,
        border_parent_assignment_count,
    })
}

fn checked_scaled_minlen_and_weight_plan(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    node_sep: usize,
) -> Result<(Vec<(EdgeKey, usize)>, f64), WorkError> {
    let mut scaled_minlen = Vec::with_capacity(g.edge_count());
    let mut weight = 1.0;
    let mut arithmetic_error = None;
    g.for_each_edge(|key, edge| {
        match arithmetic_error {
            ::core::option::Option::Some(_) => {
                return;
            },
            ::core::option::Option::None => {},
        }

        match checked_mul(edge.minlen, node_sep.max(1)) {
            Ok(minlen) => {
                scaled_minlen.push((key.clone(), minlen));
                weight += edge.weight;
            }
            Err(error) => arithmetic_error = Some(error),
        }
    });

    match arithmetic_error {
        Some(error) => {
            return Err(error);
        },
        ::core::option::Option::None => {},
    }

    Ok((scaled_minlen, weight))
}

pub fn run(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let mut work_control = NoopWorkControl;
    run_controlled(g, &mut work_control)
        .expect("the checked no-op Dugong work control cannot reject nesting work");
}

pub(crate) fn run_controlled(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<(), WorkError> {
    // The first tranche admits the ordered node scan used to derive exact amplification counts.
    // The graph is still untouched if either this scan or the complete execution tranche fails.
    work_control.charge(g.node_order_slot_count())?;
    let work_plan = checked_nesting_work_plan(g)?;
    work_control.charge(work_plan.execution_work_units)?;

    let depths = tree_depths(g)?;
    let height = match depths
        .values()
        .copied()
        .max() {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 1,
    }
        .saturating_sub(1);
    let node_sep = checked_add(checked_mul(2, height)?, 1)?;

    let (scaled_minlen, weight) = checked_scaled_minlen_and_weight_plan(g, node_sep)?;

    let mut ids = DummyNodeIdGen::default();
    let root = add_dummy_node(
        g,
        &mut ids,
        "root",
        NodeLabel {
            ..Default::default()
        },
        "_root",
    )?;

    match g.graph_mut().nesting_root.replace(root.clone()) {
        Some(gl) => {
            let _ = gl;
        },
        ::core::option::Option::None => {},
    }

    for (key, minlen) in scaled_minlen {
        {
            match g.edge_mut_by_key(&key) {
                Some(edge) => {
                    edge.minlen = minlen;
                },
                ::core::option::Option::None => {},
            };
        }
    }

    let ctx = NestingDfsCtx {
        root: &root,
        node_sep,
        weight,
        height,
        depths: &depths,
    };

    let children = g
        .children_root()
        .into_iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();
    let mut parent_assignments = Vec::with_capacity(work_plan.border_parent_assignment_count);

    for child in children {
        nesting_dfs(g, &ctx, &mut ids, &mut parent_assignments, child)?;
    }

    debug_assert_eq!(
        parent_assignments.len(),
        work_plan.border_parent_assignment_count
    );

    match {
        ::core::primitive::bool::then_some(!parent_assignments.is_empty(), ())
    } {
        ::core::option::Option::Some(()) => {
            g.try_set_unparented_leaf_parents_ix(&parent_assignments)
                .expect("fresh nesting borders must satisfy the construction-only parent batch");
        },
        ::core::option::Option::None => {},
    }

    g.graph_mut().node_rank_factor = Some(node_sep);

    // Dagre assumes the nesting graph pass makes the graph connected before ranking.
    // Our upstream parity tests include cases where the input graph is not fully connected
    // by the nesting edges alone (e.g. edges incident on cluster nodes). Connect any
    // remaining components through the nesting root so network-simplex does not panic.
    let comps = components(g);

    match {
        ::core::primitive::bool::then_some(comps.len() > 1, ())
    } {
        ::core::option::Option::Some(()) => {
            for comp in comps {
                match {
                    ::core::primitive::bool::then_some(comp.iter().any(|v| v == &root), ())
                } {
                    ::core::option::Option::Some(()) => {
                        continue;
                    },
                    ::core::option::Option::None => {},
                }

                let v = {
                    let optional_v = comp.first();

                    match optional_v {
                        Some(matched_v) => matched_v,
                        ::core::option::Option::None => continue,
                    }
                };

                match {
                    ::core::primitive::bool::then_some(v == &root, ())
                } {
                    ::core::option::Option::Some(()) => {
                        continue;
                    },
                    ::core::option::Option::None => {},
                }

                match {
                    ::core::primitive::bool::then_some(g.edge(&root, v, None).is_some(), ())
                } {
                    ::core::option::Option::Some(()) => {
                        continue;
                    },
                    ::core::option::Option::None => {},
                }

                g.set_edge_with_label(
                    root.clone(),
                    v.clone(),
                    EdgeLabel {
                        weight: 0.0,
                        // Match Dagre's nesting graph behavior: connect components through the
                        // nesting root using the same `nodeSep`-scaled minlen so rank constraints
                        // remain consistent with compound graphs.
                        minlen: node_sep.max(1),
                        nesting_edge: true,
                        ..Default::default()
                    },
                );
            }
        },
        ::core::option::Option::None => {},
    }

    Ok(())
}

#[must_use]
fn components(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>) -> Vec<Vec<String>> {
    let mut seen = FixedSet::default();
    let mut out = Vec::new();

    for start in g.node_ids() {
        match {
            ::core::primitive::bool::then_some(!seen.insert(start.clone()), ())
        } {
            ::core::option::Option::Some(()) => {
                continue;
            },
            ::core::option::Option::None => {},
        }

        let mut component = Vec::new();
        let mut queue = VecDeque::from([start]);

        while let Some(node) = queue.pop_front() {
            component.push(node.clone());

            for neighbor in g.successors(&node) {
                match {
                    ::core::primitive::bool::then_some(seen.insert(neighbor.to_string()), ())
                } {
                    ::core::option::Option::Some(()) => {
                        queue.push_back(neighbor.to_string());
                    },
                    ::core::option::Option::None => {},
                }
            }

            for neighbor in g.predecessors(&node) {
                match {
                    ::core::primitive::bool::then_some(seen.insert(neighbor.to_string()), ())
                } {
                    ::core::option::Option::Some(()) => {
                        queue.push_back(neighbor.to_string());
                    },
                    ::core::option::Option::None => {},
                }
            }
        }

        out.push(component);
    }

    out
}

pub fn cleanup(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let root = g.graph().nesting_root.clone();

    match root {
        Some(root) => {
            let _ = g.remove_node(&root);
            g.graph_mut().nesting_root = None;
        },
        ::core::option::Option::None => {},
    }

    let mut to_remove: Vec<EdgeKey> = Vec::new();
    g.for_each_edge(|k, e| {
        match {
            ::core::primitive::bool::then_some(e.nesting_edge, ())
        } {
            ::core::option::Option::Some(()) => {
                to_remove.push(k.clone());
            },
            ::core::option::Option::None => {},
        }
    });

    for k in to_remove {
        let _ = g.remove_edge_key(&k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphlib::GraphOptions;

    #[derive(Default)]
    struct RejectSecondCharge {
        calls: Vec<usize>,
    }

    impl WorkControl for RejectSecondCharge {
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.calls.push(units);
            if self.calls.len() == 2 {
                Err(WorkError::Interrupted)
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn controlled_nesting_rejects_after_planning_before_mutation() {
        let mut graph = Graph::new(GraphOptions {
            compound: true,
            multigraph: true,
            ..GraphOptions::default()
        });
        graph.set_graph(GraphLabel::default());
        graph.set_default_node_label(NodeLabel::default);
        graph.set_default_edge_label(EdgeLabel::default);
        graph.set_parent("leaf", "cluster");
        graph.set_edge_with_label(
            "source",
            "leaf",
            EdgeLabel {
                minlen: 2,
                weight: 3.0,
                ..EdgeLabel::default()
            },
        );

        let before_nodes = graph.node_ids();
        let before_edges = graph.edge_keys();
        let before_children = graph
            .children("cluster")
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let before_edge = graph
            .edge("source", "leaf", None)
            .cloned()
            .expect("fixture edge");
        let planning_work = graph.node_order_slot_count();
        let mut work_control = RejectSecondCharge::default();

        let error = run_controlled(&mut graph, &mut work_control).unwrap_err();

        assert_eq!(error, WorkError::Interrupted);
        assert_eq!(work_control.calls.len(), 2);
        assert_eq!(work_control.calls[0], planning_work);
        assert_eq!(graph.node_ids(), before_nodes);
        assert_eq!(graph.edge_keys(), before_edges);
        assert_eq!(graph.children("cluster"), before_children);
        assert_eq!(graph.edge("source", "leaf", None), Some(&before_edge));
        assert_eq!(graph.graph().nesting_root, None);
    }

    #[test]
    fn nesting_parent_batch_preserves_graphlib_child_creation_order() {
        let mut graph = Graph::new(GraphOptions {
            compound: true,
            multigraph: true,
            ..GraphOptions::default()
        });
        graph.set_graph(GraphLabel::default());
        graph.set_default_node_label(NodeLabel::default);
        graph.set_default_edge_label(EdgeLabel::default);
        graph.set_parent("first", "cluster");
        graph.set_parent("second", "cluster");

        run(&mut graph);

        let cluster = graph.node("cluster").cloned().expect("cluster label");
        let top = cluster.border_top.expect("top border");
        let bottom = cluster.border_bottom.expect("bottom border");
        assert_eq!(
            graph.children("cluster"),
            vec!["first", "second", top.as_str(), bottom.as_str()]
        );
    }
}
