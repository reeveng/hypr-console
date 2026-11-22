use super::barycenter::{BarycenterEntryIx, SortEntryIx, SortResultIx, sort_ix};
use super::{OrderEdgeWeight, OrderNodeLabel, Relationship};
use crate::graphlib::Graph;
use crate::work::{checked_add, checked_mul, checked_n_log_n};
use crate::{WorkControl, WorkError};
use console_core_perfect_hash::collections::{FixedMap as HashMap, FixedSet as HashSet};

#[derive(Debug, Clone, Copy)]
struct WeightedNeighbor {
    original_ix: usize,
    weight: f64,
}

#[derive(Debug)]
struct LayerNode {
    original_ix: Option<usize>,
    parent: Option<usize>,
    children: Vec<usize>,
    use_original_order: bool,
    border_left: Option<usize>,
    border_right: Option<usize>,
}

impl LayerNode {
    #[must_use]
    fn root() -> Self {
        Self {
            original_ix: None,
            parent: None,
            children: Vec::new(),
            use_original_order: false,
            border_left: None,
            border_right: None,
        }
    }

    #[must_use]
    fn original(original_ix: usize) -> Self {
        Self {
            original_ix: Some(original_ix),
            parent: None,
            children: Vec::new(),
            use_original_order: true,
            border_left: None,
            border_right: None,
        }
    }
}

#[derive(Debug)]
struct LayerRank {
    nodes: Vec<LayerNode>,
    local_by_original: HashMap<usize, usize>,
    root: usize,
    hierarchy_link_visits: usize,
    child_sort_work: usize,
}

impl LayerRank {
    fn build<N, E, G>(
        g: &Graph<N, E, G>,
        rank: i32,
        nodes_with_rank: &[usize],
    ) -> Result<Self, WorkError>
    where
        N: Default + OrderNodeLabel + 'static,
        E: Default + 'static,
        G: Default,
    {
        let mut layer = Self {
            nodes: vec![LayerNode::root()],
            local_by_original: HashMap::default(),
            root: 0,
            hierarchy_link_visits: 0,
            child_sort_work: 0,
        };
        layer.local_by_original.reserve(nodes_with_rank.len());

        for &original_ix in nodes_with_rank {
            let id = {
                let optional_id = g.node_id_by_ix(original_ix);

                match optional_id {
                    Some(matched_id) => matched_id,
                    ::core::option::Option::None => continue,
                }
            };
            let label = {
                let optional_label = g.node_label_by_ix(original_ix);

                match optional_label {
                    Some(matched_label) => matched_label,
                    ::core::option::Option::None => continue,
                }
            };

            let has_min_rank = label.has_min_rank();
            let border_left = label
                .border_left_at(rank)
                .as_deref()
                .and_then(|border| g.node_ix(border));
            let border_right = label
                .border_right_at(rank)
                .as_deref()
                .and_then(|border| g.node_ix(border));
            let parent_original = g.parent(id).and_then(|parent| g.node_ix(parent));

            let local_ix = layer.ensure_original(original_ix);
            let parent_local = match parent_original
                .map(|parent_ix| layer.ensure_original(parent_ix)) {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => layer.root,
            };
            let node = &mut layer.nodes[local_ix];
            node.use_original_order = !has_min_rank;
            node.border_left = border_left;
            node.border_right = border_right;
            layer.set_parent(local_ix, parent_local);
        }

        let mut visited = vec![false; layer.nodes.len()];
        let mut stack = vec![(layer.root, 0usize)];

        while let Some((local_ix, depth)) = stack.pop() {
            match {
                ::core::primitive::bool::then_some(local_ix >= visited.len() || visited[local_ix], ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            visited[local_ix] = true;
            layer.hierarchy_link_visits = checked_add(layer.hierarchy_link_visits, depth)?;
            let children = layer.children(local_ix).to_vec();
            layer.child_sort_work =
                checked_add(layer.child_sort_work, checked_n_log_n(children.len())?)?;
            let child_depth = checked_add(depth, 1)?;
            stack.extend(children.into_iter().rev().map(|child| (child, child_depth)));
        }

        Ok(layer)
    }

    fn ensure_original(&mut self, original_ix: usize) -> usize {
        match self.local_by_original.get(&original_ix) {
            Some(&local_ix) => {
                return local_ix;
            },
            ::core::option::Option::None => {},
        }

        let local_ix = self.nodes.len();
        self.nodes.push(LayerNode::original(original_ix));
        self.local_by_original.insert(original_ix, local_ix);
        local_ix
    }

    fn set_parent(&mut self, child: usize, parent: usize) {
        let previous = self.nodes.get(child).and_then(|node| node.parent);

        match {
            ::core::primitive::bool::then_some(previous == Some(parent), ())
        } {
            ::core::option::Option::Some(()) => {
                return;
            },
            ::core::option::Option::None => {},
        }

        if let Some(previous) = previous
            && let Some(children) = self.nodes.get_mut(previous).map(|node| &mut node.children)
        {
            children.retain(|&candidate| candidate != child);
        }

        match self.nodes.get_mut(child) {
            Some(node) => {
                node.parent = Some(parent);
            },
            ::core::option::Option::None => {},
        }

        {
            match self.nodes.get_mut(parent).map(|node| &mut node.children) {
                Some(children) => {
                    children.push(child);
                },
                ::core::option::Option::None => {},
            };
        }
    }

    #[must_use]
    fn original_ix(&self, local_ix: usize) -> Option<usize> {
        self.nodes.get(local_ix)?.original_ix
    }

    #[must_use]
    fn local_ix(&self, original_ix: usize) -> Option<usize> {
        self.local_by_original.get(&original_ix).copied()
    }

    #[must_use]
    fn children(&self, local_ix: usize) -> &[usize] {
        self.nodes
            .get(local_ix)
            .map(|node| node.children.as_slice())
            .unwrap_or(&[])
    }

    #[must_use]
    fn parent(&self, local_ix: usize) -> Option<usize> {
        self.nodes.get(local_ix).and_then(|node| node.parent)
    }

    #[must_use]
    fn order<N, E, G>(&self, g: &Graph<N, E, G>, local_ix: usize) -> Option<usize>
    where
        N: Default + OrderNodeLabel + 'static,
        E: Default + 'static,
        G: Default,
    {
        let node = self.nodes.get(local_ix)?;

        match {
            ::core::primitive::bool::then_some(!node.use_original_order, ())
        } {
            ::core::option::Option::Some(()) => {
                return None;
            },
            ::core::option::Option::None => {},
        }

        g.node_label_by_ix(node.original_ix?)?.order()
    }

    #[must_use]
    fn order_by_original<N, E, G>(
        &self,
        g: &Graph<N, E, G>,
        tracked_orders: &[bool],
        original_ix: usize,
    ) -> Option<usize>
    where
        N: Default + OrderNodeLabel + 'static,
        E: Default + 'static,
        G: Default,
    {
        match {
            ::core::primitive::bool::then_some(!(match tracked_orders.get(original_ix).copied() {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => false,
            }), ())
        } {
            ::core::option::Option::Some(()) => {
                return None;
            },
            ::core::option::Option::None => {},
        }

        if let Some(local_ix) = self.local_ix(original_ix) {
            self.order(g, local_ix)
        } else {
            g.node_label_by_ix(original_ix)?.order()
        }
    }
}

#[derive(Debug, Default)]
struct ConstraintGraph {
    outgoing: HashMap<usize, Vec<usize>>,
    seen: HashSet<(usize, usize)>,
}

impl ConstraintGraph {
    fn clear(&mut self) {
        self.outgoing.clear();
        self.seen.clear();
    }

    fn insert(&mut self, from: usize, to: usize) {
        match {
            ::core::primitive::bool::then_some(self.seen.insert((from, to)), ())
        } {
            ::core::option::Option::Some(()) => {
                self.outgoing.entry(from).or_default().push(to);
            },
            ::core::option::Option::None => {},
        }
    }
}

#[derive(Debug, Default)]
struct SortScratch {
    entry_by_original: Vec<Option<usize>>,
    touched_originals: Vec<usize>,
}

impl SortScratch {
    fn prepare_entries(&mut self, node_slots: usize) {
        match {
            ::core::primitive::bool::then_some(self.entry_by_original.len() < node_slots, ())
        } {
            ::core::option::Option::Some(()) => {
                self.entry_by_original.resize(node_slots, None);
            },
            ::core::option::Option::None => {},
        }

        for original_ix in self.touched_originals.drain(..) {
            self.entry_by_original[original_ix] = None;
        }
    }

    fn clear_entries(&mut self) {
        for original_ix in self.touched_originals.drain(..) {
            self.entry_by_original[original_ix] = None;
        }
    }
}

pub(super) struct OrderWorkspace {
    layers: Vec<LayerRank>,
    in_neighbors: Vec<Vec<WeightedNeighbor>>,
    out_neighbors: Vec<Vec<WeightedNeighbor>>,
    tracked_orders: Vec<bool>,
    constraints: ConstraintGraph,
    scratch: SortScratch,
    in_iteration_work_prefix: Vec<usize>,
    out_iteration_work_prefix: Vec<usize>,
}

impl OrderWorkspace {
    pub(super) fn new<N, E, G>(
        g: &Graph<N, E, G>,
        nodes_by_rank: &[Vec<usize>],
        max_rank: i32,
    ) -> Result<Self, WorkError>
    where
        N: Default + OrderNodeLabel + 'static,
        E: Default + OrderEdgeWeight + 'static,
        G: Default,
    {
        let mut node_slots = 0usize;
        g.for_each_node_ix(|node_ix, _id, _node| {
            node_slots = node_slots.max(node_ix.saturating_add(1));
        });

        let mut tracked_orders = vec![false; node_slots];
        g.for_each_node_ix(|node_ix, _id, node| {
            tracked_orders[node_ix] = node.order().is_some();
        });
        let in_neighbors = build_weighted_neighbors(g, node_slots, Relationship::InEdges);
        let out_neighbors = build_weighted_neighbors(g, node_slots, Relationship::OutEdges);
        let rank_count =
            usize::try_from(i64::from(max_rank) + 1).map_err(|_| WorkError::ArithmeticOverflow)?;
        let mut layers = Vec::with_capacity(rank_count);

        for rank_index in 0..rank_count {
            let rank = i32::try_from(rank_index).map_err(|_| WorkError::ArithmeticOverflow)?;
            let nodes = nodes_by_rank
                .get(rank_index)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            layers.push(LayerRank::build(g, rank, nodes)?);
        }

        let mut in_iteration_work_prefix = Vec::with_capacity(layers.len() + 1);
        let mut out_iteration_work_prefix = Vec::with_capacity(layers.len() + 1);
        in_iteration_work_prefix.push(0usize);
        out_iteration_work_prefix.push(0usize);

        for layer in &layers {
            let mut in_neighbor_visits = 0usize;
            let mut out_neighbor_visits = 0usize;

            'over_node_2: for node in &layer.nodes {
                let original_ix = match node.original_ix {
                    Some(matched_original_ix) => matched_original_ix,
                    ::core::option::Option::None => continue 'over_node_2,
                };
                in_neighbor_visits = checked_add(
                    in_neighbor_visits,
                    in_neighbors.get(original_ix).map_or(0, Vec::len),
                )?;
                out_neighbor_visits = checked_add(
                    out_neighbor_visits,
                    out_neighbors.get(original_ix).map_or(0, Vec::len),
                )?;
            }

            let hierarchy_work = checked_mul(layer.hierarchy_link_visits, 3)?;
            let common_work = checked_add(
                layer.nodes.len(),
                checked_add(hierarchy_work, layer.child_sort_work)?,
            )?;
            let in_work = checked_add(common_work, in_neighbor_visits)?;
            let out_work = checked_add(common_work, out_neighbor_visits)?;
            in_iteration_work_prefix.push(checked_add(
                *in_iteration_work_prefix.last().unwrap_or(&0),
                in_work,
            )?);
            out_iteration_work_prefix.push(checked_add(
                *out_iteration_work_prefix.last().unwrap_or(&0),
                out_work,
            )?);
        }

        Ok(Self {
            layers,
            in_neighbors,
            out_neighbors,
            tracked_orders,
            constraints: ConstraintGraph::default(),
            scratch: SortScratch::default(),
            in_iteration_work_prefix,
            out_iteration_work_prefix,
        })
    }

    pub(super) fn begin_sweep(&mut self) {
        self.constraints.clear();
    }

    pub(super) fn iteration_work_units(
        &self,
        ranks: &[i32],
        relationship: Relationship,
    ) -> Result<usize, WorkError> {
        let (first, last) = {
            let optional_first = ranks.first().zip(ranks.last());

            match optional_first {
                Some((&matched_first, &matched_last)) => (matched_first, matched_last),
                ::core::option::Option::None => return Ok(0),
            }
        };
        let start = usize::try_from(first.min(last)).map_err(|_| WorkError::ArithmeticOverflow)?;
        let end = usize::try_from(first.max(last))
            .map_err(|_| WorkError::ArithmeticOverflow)?
            .checked_add(1)
            .ok_or(WorkError::ArithmeticOverflow)?;
        let prefix = match relationship {
            Relationship::InEdges => &self.in_iteration_work_prefix,
            Relationship::OutEdges => &self.out_iteration_work_prefix,
        };
        let (before, after) = {
            let optional_before = prefix.get(start).zip(prefix.get(end));

            match optional_before {
                Some((&matched_before, &matched_after)) => (matched_before, matched_after),
                ::core::option::Option::None => return Err(WorkError::ArithmeticOverflow),
            }
        };
        after
            .checked_sub(before)
            .ok_or(WorkError::ArithmeticOverflow)
    }

    pub(super) fn sort_rank<N, E, G>(
        &mut self,
        g: &Graph<N, E, G>,
        rank: usize,
        relationship: Relationship,
        bias_right: bool,
        work_control: &mut dyn WorkControl,
    ) -> Result<SortResultIx, WorkError>
    where
        N: Default + OrderNodeLabel + 'static,
        E: Default + 'static,
        G: Default,
    {
        let Some(layer) = self.layers.get(rank) else {
            return Ok(SortResultIx {
                vs: Vec::new(),
                barycenter: None,
                weight: None,
            });
        };

        let neighbors = match relationship {
            Relationship::InEdges => &self.in_neighbors,
            Relationship::OutEdges => &self.out_neighbors,
        };
        sort_subgraph(
            SortSubgraphContext {
                graph: g,
                layer,
                neighbors,
                tracked_orders: &self.tracked_orders,
                constraints: &self.constraints,
            },
            &mut self.scratch,
            bias_right,
            work_control,
        )
    }

    #[must_use]
    pub(super) fn original_ix(&self, rank: usize, local_ix: usize) -> Option<usize> {
        self.layers.get(rank)?.original_ix(local_ix)
    }

    pub(super) fn add_constraints(&mut self, rank: usize, sorted: &[usize]) {
        let layer = {
            let optional_layer = self.layers.get(rank);

            match optional_layer {
                Some(matched_layer) => matched_layer,
                ::core::option::Option::None => return,
            }
        };
        add_subgraph_constraints(layer, &mut self.constraints, sorted);
    }
}

#[must_use]
fn build_weighted_neighbors<N, E, G>(
    g: &Graph<N, E, G>,
    node_slots: usize,
    relationship: Relationship,
) -> Vec<Vec<WeightedNeighbor>>
where
    N: Default + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    let mut result: Vec<Vec<WeightedNeighbor>> = (0..node_slots).map(|_| Vec::new()).collect();
    let mut position_by_neighbor = vec![usize::MAX; node_slots];
    let mut touched_neighbors = Vec::new();

    g.for_each_node_ix(|node_ix, id, _node| {
        for neighbor_ix in touched_neighbors.drain(..) {
            position_by_neighbor[neighbor_ix] = usize::MAX;
        }

        let mut add = |neighbor_ix: usize, weight: f64| {
            match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(neighbor_ix), &::core::convert::identity(node_slots)); _explicit_ordering } {
                ::core::cmp::Ordering::Equal | ::core::cmp::Ordering::Greater => {
                    return;
                },
                ::core::cmp::Ordering::Less => {},
            }

            let position = match {
                ::core::primitive::bool::then_some(position_by_neighbor[neighbor_ix] != usize::MAX, ())
            } {
                ::core::option::Option::Some(()) => {
                    position_by_neighbor[neighbor_ix]
                },
                ::core::option::Option::None => {
                    let position = result[node_ix].len();
                    result[node_ix].push(WeightedNeighbor {
                        original_ix: neighbor_ix,
                        weight: 0.0,
                    });
                    position_by_neighbor[neighbor_ix] = position;
                    touched_neighbors.push(neighbor_ix);
                    position
                },
            };
            result[node_ix][position].weight += weight;
        };

        match (g.is_directed(), relationship) {
            (true, Relationship::InEdges) => {
                g.for_each_in_edge_ix(node_ix, None, |from_ix, _to_ix, _key, label| {
                    add(from_ix, label.weight());
                });
            }
            (true, Relationship::OutEdges) => {
                g.for_each_out_edge_ix(node_ix, None, |_from_ix, to_ix, _key, label| {
                    add(to_ix, label.weight());
                });
            }
            (false, Relationship::InEdges) => {
                g.for_each_in_edge(id, None, |key, label| {
                    {
                        match g.node_ix(&key.v) {
                            Some(neighbor_ix) => {
                                add(neighbor_ix, label.weight());
                            },
                            ::core::option::Option::None => {},
                        };
                    }
                });
            }
            (false, Relationship::OutEdges) => {
                g.for_each_out_edge(id, None, |key, label| {
                    {
                        match g.node_ix(&key.w) {
                            Some(neighbor_ix) => {
                                add(neighbor_ix, label.weight());
                            },
                            ::core::option::Option::None => {},
                        };
                    }
                });
            }
        }
    });

    result
}

fn add_subgraph_constraints(
    layer: &LayerRank,
    constraints: &mut ConstraintGraph,
    sorted: &[usize],
) {
    let mut previous_by_parent: HashMap<usize, usize> = HashMap::default();
    let mut previous_root: Option<usize> = None;

    for &local_ix in sorted {
        let mut child = layer.parent(local_ix);

        'inner_while_2: while let Some(current) = child {
            let parent = layer.parent(current);
            let previous = match parent {
                Some(parent) => {
                    previous_by_parent.insert(parent, current)
                },
                ::core::option::Option::None => {
                    previous_root.replace(current)
                },
            };

            if let Some(previous) = previous
                && previous != current
            {
                match (layer.original_ix(previous), layer.original_ix(current)) {
                    (Some(from), Some(to)) => {
                        constraints.insert(from, to);
                    },
                    (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {},
                }

                break 'inner_while_2;
            }

            child = parent;
        }
    }
}

struct SortSubgraphContext<'a, N: Default + 'static, E: Default + 'static, G: Default> {
    graph: &'a Graph<N, E, G>,
    layer: &'a LayerRank,
    neighbors: &'a [Vec<WeightedNeighbor>],
    tracked_orders: &'a [bool],
    constraints: &'a ConstraintGraph,
}

fn sort_subgraph<N, E, G>(
    context: SortSubgraphContext<'_, N, E, G>,
    scratch: &mut SortScratch,
    bias_right: bool,
    work_control: &mut dyn WorkControl,
) -> Result<SortResultIx, WorkError>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + 'static,
    G: Default,
{
    struct Frame {
        local_ix: usize,
        barycenters: Vec<BarycenterEntryIx>,
        border_left: Option<usize>,
        border_right: Option<usize>,
    }

    enum Step {
        Enter(usize),
        Exit(Frame),
    }

    let SortSubgraphContext {
        graph,
        layer,
        neighbors,
        tracked_orders,
        constraints,
    } = context;
    let mut results: Vec<Option<SortResultIx>> = (0..layer.nodes.len()).map(|_| None).collect();
    let mut stack = vec![Step::Enter(layer.root)];

    while let Some(step) = stack.pop() {
        match step {
            Step::Enter(local_ix) => {
                let node = &layer.nodes[local_ix];
                let border_left = node.border_left.and_then(|ix| layer.local_ix(ix));
                let border_right = node.border_right.and_then(|ix| layer.local_ix(ix));
                let mut movable = layer.children(local_ix).to_vec();

                match {
                    ::core::primitive::bool::then_some(border_left.is_some() && border_right.is_some(), ())
                } {
                    ::core::option::Option::Some(()) => {
                        movable.retain(|candidate| {
                            Some(*candidate) != border_left && Some(*candidate) != border_right
                        });
                    },
                    ::core::option::Option::None => {},
                }

                let barycenters = barycenters(graph, layer, neighbors, tracked_orders, &movable);
                let mut nested = Vec::new();

                for entry in &barycenters {
                    match {
                        ::core::primitive::bool::then_some(!layer.children(entry.v_ix).is_empty(), ())
                    } {
                        ::core::option::Option::Some(()) => {
                            nested.push(entry.v_ix);
                        },
                        ::core::option::Option::None => {},
                    }
                }

                stack.push(Step::Exit(Frame {
                    local_ix,
                    barycenters,
                    border_left,
                    border_right,
                }));

                for child in nested.into_iter().rev() {
                    stack.push(Step::Enter(child));
                }
            }
            Step::Exit(mut frame) => {
                'over_entry_2: for entry in &mut frame.barycenters {
                    let subgraph = {
                        let optional_subgraph = results[entry.v_ix].as_ref();

                        match optional_subgraph {
                            Some(matched_subgraph) => matched_subgraph,
                            ::core::option::Option::None => continue 'over_entry_2,
                        }
                    };
                    merge_barycenters(entry, subgraph);
                }

                let mut entries = resolve_conflicts(
                    layer,
                    &frame.barycenters,
                    constraints,
                    scratch,
                    neighbors.len(),
                    work_control,
                )?;
                expand_subgraphs(&mut entries, &results);

                for entry in &frame.barycenters {
                    results[entry.v_ix].take();
                }

                let mut result = sort_ix(&entries, bias_right);

                match (frame.border_left, frame.border_right) {
                    (Some(border_left), Some(border_right)) => {
                        let mut ordered = Vec::with_capacity(result.vs.len() + 2);
                        ordered.push(border_left);
                        ordered.extend(result.vs);
                        ordered.push(border_right);
                        result.vs = ordered;

                        let left_predecessor = first_neighbor_original(layer, neighbors, border_left);
                        let right_predecessor = first_neighbor_original(layer, neighbors, border_right);

                        match (left_predecessor, right_predecessor) {
                            (Some(left), Some(right)) => {
                                let left_order = (match layer
                                    .order_by_original(graph, tracked_orders, left) {
                                    ::core::option::Option::Some(present) => present,
                                    ::core::option::Option::None => 0,
                                }) as f64;
                                let right_order = (match layer
                                    .order_by_original(graph, tracked_orders, right) {
                                    ::core::option::Option::Some(present) => present,
                                    ::core::option::Option::None => 0,
                                }) as f64;
                                let barycenter = match result.barycenter {
                                    ::core::option::Option::Some(present) => present,
                                    ::core::option::Option::None => 0.0,
                                };
                                let weight = match result.weight {
                                    ::core::option::Option::Some(present) => present,
                                    ::core::option::Option::None => 0.0,
                                };
                                let denominator = weight + 2.0;
                                result.barycenter =
                                    Some((barycenter * weight + left_order + right_order) / denominator);
                                result.weight = Some(denominator);
                            },
                            (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {},
                        }
                    },
                    (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {},
                }

                results[frame.local_ix] = Some(result);
            }
        }
    }

    Ok(match (results[layer.root].take(), SortResultIx {
            vs: Vec::new(),
            barycenter: None,
            weight: None,
        }) {
        (::core::option::Option::Some(present), alternate) => { ::core::mem::drop(alternate); present },
        (::core::option::Option::None, alternate) => alternate,
    })
}

#[must_use]
fn barycenters<N, E, G>(
    g: &Graph<N, E, G>,
    layer: &LayerRank,
    neighbors: &[Vec<WeightedNeighbor>],
    tracked_orders: &[bool],
    movable: &[usize],
) -> Vec<BarycenterEntryIx>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + 'static,
    G: Default,
{
    movable
        .iter()
        .map(|&local_ix| {
            let Some(original_ix) = layer.original_ix(local_ix) else {
                return BarycenterEntryIx {
                    v_ix: local_ix,
                    barycenter: None,
                    weight: None,
                };
            };

            let Some(node_neighbors) = neighbors.get(original_ix) else {
                return BarycenterEntryIx {
                    v_ix: local_ix,
                    barycenter: None,
                    weight: None,
                };
            };

            match {
                ::core::primitive::bool::then_some(node_neighbors.is_empty(), ())
            } {
                ::core::option::Option::Some(()) => {
                    return BarycenterEntryIx {
                        v_ix: local_ix,
                        barycenter: None,
                        weight: None,
                    };
                },
                ::core::option::Option::None => {},
            }

            let mut sum = 0.0;
            let mut weight = 0.0;

            for neighbor in node_neighbors {
                let order = (match layer
                    .order_by_original(g, tracked_orders, neighbor.original_ix) {
                    ::core::option::Option::Some(present) => present,
                    ::core::option::Option::None => 0,
                }) as f64;
                sum += neighbor.weight * order;
                weight += neighbor.weight;
            }

            BarycenterEntryIx {
                v_ix: local_ix,
                barycenter: Some(sum / weight),
                weight: Some(weight),
            }
        })
        .collect()
}

#[must_use]
fn first_neighbor_original(
    layer: &LayerRank,
    neighbors: &[Vec<WeightedNeighbor>],
    local_ix: usize,
) -> Option<usize> {
    let original_ix = layer.original_ix(local_ix)?;
    neighbors
        .get(original_ix)?
        .first()
        .map(|edge| edge.original_ix)
}

fn merge_barycenters(target: &mut BarycenterEntryIx, other: &SortResultIx) {
    let other_barycenter = match other.barycenter {
        Some(matched_other_barycenter) => matched_other_barycenter,
        ::core::option::Option::None => return,
    };
    let other_weight = match other.weight {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0.0,
    };

    match (target.barycenter, target.weight) {
        (Some(barycenter), Some(weight)) => {
            let denominator = weight + other_weight;
            target.barycenter =
                Some((barycenter * weight + other_barycenter * other_weight) / denominator);
            target.weight = Some(denominator);
        },
        (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {
            target.barycenter = Some(other_barycenter);
            target.weight = Some(other_weight);
        },
    }
}

fn expand_subgraphs(entries: &mut [SortEntryIx], subgraphs: &[Option<SortResultIx>]) {
    for entry in entries {
        let mut expanded = Vec::new();

        for local_ix in std::mem::take(&mut entry.vs) {
            if let Some(Some(subgraph)) = subgraphs.get(local_ix) {
                expanded.extend(subgraph.vs.iter().copied());
            } else {
                expanded.push(local_ix);
            }
        }

        entry.vs = expanded;
    }
}

#[derive(Debug)]
struct ConflictEntry {
    indegree: usize,
    ins: Vec<usize>,
    outs: Vec<usize>,
    head: Option<usize>,
    tail: Option<usize>,
    i: usize,
    barycenter: Option<f64>,
    weight: Option<f64>,
    merged: bool,
}

fn resolve_conflicts(
    layer: &LayerRank,
    entries: &[BarycenterEntryIx],
    constraints: &ConstraintGraph,
    scratch: &mut SortScratch,
    node_slots: usize,
    work_control: &mut dyn WorkControl,
) -> Result<Vec<SortEntryIx>, WorkError> {
    scratch.prepare_entries(node_slots);
    let mut conflicts = Vec::with_capacity(entries.len());
    let mut next_in_group = vec![None; entries.len()];

    for (entry_ix, entry) in entries.iter().enumerate() {
        match layer.original_ix(entry.v_ix) {
            Some(original_ix) => {
                scratch.entry_by_original[original_ix] = Some(entry_ix);
                scratch.touched_originals.push(original_ix);
            },
            ::core::option::Option::None => {},
        }

        conflicts.push(ConflictEntry {
            indegree: 0,
            ins: Vec::new(),
            outs: Vec::new(),
            head: Some(entry_ix),
            tail: Some(entry_ix),
            i: entry_ix,
            barycenter: entry.barycenter,
            weight: entry.weight,
            merged: false,
        });
    }

    // Pinned Graphlib's global edge scan appends every constraint to its source entry's `out`
    // array. Interleaving edges from different sources cannot affect those per-source arrays, so
    // replay sources in entry order while preserving each source's outgoing insertion order. This
    // is equivalent to Dagre's adjacency construction without collecting and sorting global edge
    // ordinals.
    for (from_entry, entry) in entries.iter().enumerate() {
        let from = {
            let optional_from = layer.original_ix(entry.v_ix);

            match optional_from {
                Some(matched_from) => matched_from,
                ::core::option::Option::None => continue,
            }
        };
        let outgoing = {
            let optional_outgoing = constraints.outgoing.get(&from);

            match optional_outgoing {
                Some(matched_outgoing) => matched_outgoing,
                ::core::option::Option::None => continue,
            }
        };
        work_control.charge(outgoing.len())?;

        'inner_for_2: for &to in outgoing {
            let to_entry = {
                let outcome_to_entry = scratch.entry_by_original.get(to).copied();

                match outcome_to_entry {
                    Some(Some(matched_to_entry)) => matched_to_entry,
                    ::core::option::Option::Some(::core::option::Option::None) | ::core::option::Option::None => continue 'inner_for_2,
                }
            };
            conflicts[to_entry].indegree += 1;
            conflicts[from_entry].outs.push(to_entry);
        }
    }

    scratch.clear_entries();

    let mut sources = Vec::new();

    for (entry_ix, entry) in conflicts.iter().enumerate() {
        match entry.indegree {
            0 => {
                sources.push(entry_ix);
            },
            _ => {},
        }
    }

    let mut processed = Vec::new();

    while let Some(entry_ix) = sources.pop() {
        processed.push(entry_ix);
        let ins = std::mem::take(&mut conflicts[entry_ix].ins);

        'over_incoming_2: for incoming in ins.into_iter().rev() {
            match {
                ::core::primitive::bool::then_some(conflicts[incoming].merged, ())
            } {
                ::core::option::Option::Some(()) => {
                    continue 'over_incoming_2;
                },
                ::core::option::Option::None => {},
            }

            let should_merge = match (
                conflicts[incoming].barycenter,
                conflicts[entry_ix].barycenter,
            ) {
                (None, _) | (_, None) => true,
                (Some(incoming), Some(current)) => incoming >= current,
            };

            match ::core::primitive::bool::then_some(should_merge, ()) {
                ::core::option::Option::Some(()) => {
                    merge_conflict_entries(&mut conflicts, &mut next_in_group, entry_ix, incoming);
                },
                ::core::option::Option::None => {},
            }
        }

        let outs = std::mem::take(&mut conflicts[entry_ix].outs);

        for outgoing in outs {
            conflicts[outgoing].ins.push(entry_ix);
            conflicts[outgoing].indegree = conflicts[outgoing].indegree.saturating_sub(1);

            match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(conflicts[outgoing].indegree), &::core::convert::identity(0)); _explicit_ordering } {
                ::core::cmp::Ordering::Equal => {
                    sources.push(outgoing);
                },
                ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Greater => {},
            }
        }
    }

    let mut resolved = Vec::new();

    for entry_ix in processed {
        let entry = &conflicts[entry_ix];

        match {
            ::core::primitive::bool::then_some(entry.merged, ())
        } {
            ::core::option::Option::Some(()) => {
                continue;
            },
            ::core::option::Option::None => {},
        }

        let mut vs = Vec::new();
        let mut current = entry.head;

        while let Some(entry_ix) = current {
            vs.push(entries[entry_ix].v_ix);
            current = next_in_group[entry_ix];
        }

        resolved.push(SortEntryIx {
            vs,
            i: entry.i,
            barycenter: entry.barycenter,
            weight: entry.weight,
        });
    }

    Ok(resolved)
}

fn merge_conflict_entries(
    entries: &mut [ConflictEntry],
    next_in_group: &mut [Option<usize>],
    target: usize,
    source: usize,
) {
    match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(target), &::core::convert::identity(source)); _explicit_ordering } {
        ::core::cmp::Ordering::Equal => {
            return;
        },
        ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Greater => {},
    }

    let (target_entry, source_entry) = match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(target), &::core::convert::identity(source)); _explicit_ordering } {
        ::core::cmp::Ordering::Less => {
            let (left, right) = entries.split_at_mut(source);
            (&mut left[target], &mut right[0])
        },
        ::core::cmp::Ordering::Equal | ::core::cmp::Ordering::Greater => {
            let (left, right) = entries.split_at_mut(target);
            (&mut right[0], &mut left[source])
        },
    };

    let mut sum = 0.0;
    let mut weight = 0.0;

    if let (Some(barycenter), Some(entry_weight)) = (target_entry.barycenter, target_entry.weight)
        && entry_weight != 0.0
    {
        sum += barycenter * entry_weight;
        weight += entry_weight;
    }

    if let (Some(barycenter), Some(entry_weight)) = (source_entry.barycenter, source_entry.weight)
        && entry_weight != 0.0
    {
        sum += barycenter * entry_weight;
        weight += entry_weight;
    }

    if let Some(source_tail) = source_entry.tail
        && source_tail < next_in_group.len()
    {
        next_in_group[source_tail] = target_entry.head;
    }

    target_entry.head = source_entry.head.or(target_entry.head);

    match {
        ::core::primitive::bool::then_some(target_entry.tail.is_none(), ())
    } {
        ::core::option::Option::Some(()) => {
            target_entry.tail = source_entry.tail;
        },
        ::core::option::Option::None => {},
    }

    source_entry.head = None;
    source_entry.tail = None;

    match {
        ::core::primitive::bool::then_some(weight != 0.0, ())
    } {
        ::core::option::Option::Some(()) => {
            target_entry.barycenter = Some(sum / weight);
            target_entry.weight = Some(weight);
        },
        ::core::option::Option::None => {},
    }

    target_entry.i = target_entry.i.min(source_entry.i);
    source_entry.merged = true;
}

#[cfg(test)]
mod tests;
