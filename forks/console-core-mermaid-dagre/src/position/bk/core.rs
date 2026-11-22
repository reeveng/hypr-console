//! Brandes & Köpf (BK) horizontal compaction.
//!
//! This module is a parity-oriented port of Dagre's `position/bk` helpers.
//!
//! Note: this file is being split into submodules to keep individual algorithms focused.

use crate::graphlib::{Graph, GraphOptions};
use crate::work::{ceil_log2, checked_add, checked_mul};
use crate::{EdgeLabel, GraphLabel, NodeLabel, NoopWorkControl, WorkControl, WorkError};
use console_core_perfect_hash::collections::FixedMap as HashMap;
use console_core_perfect_hash::collections::FixedSet as HashSet;
use std::collections::{BTreeMap, BTreeSet};

use super::util::{SepNodeMetrics, sep, sep_metrics, width};

pub type Conflicts = BTreeMap<String, BTreeSet<String>>;

pub fn add_conflict(conflicts: &mut Conflicts, v: &str, w: &str) {
    let (v, w) = match {
        ::core::primitive::bool::then_some(v <= w, ())
    } {
        ::core::option::Option::Some(()) => { (v, w) },
        ::core::option::Option::None => { (w, v) },
    };
    conflicts
        .entry(v.to_string())
        .or_default()
        .insert(w.to_string());
}

#[must_use]
pub fn has_conflict(conflicts: &Conflicts, v: &str, w: &str) -> bool {
    let (v, w) = match {
        ::core::primitive::bool::then_some(v <= w, ())
    } {
        ::core::option::Option::Some(()) => { (v, w) },
        ::core::option::Option::None => { (w, v) },
    };

    match conflicts.get(v).map(|m| m.contains(w)) {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Type2Boundary {
    south_index: usize,
    north_order: Option<isize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Type2Bounds {
    lower: isize,
    upper: isize,
}

#[must_use]
fn type2_conflict_bounds(
    south_len: usize,
    north_len: isize,
    boundaries: &[Type2Boundary],
) -> Vec<Type2Bounds> {
    let mut bounds = vec![
        Type2Bounds {
            lower: -1,
            upper: north_len,
        };
        south_len
    ];

    let mut lower = -1;
    let mut boundary_index = 0;

    for (south_index, current) in bounds.iter_mut().enumerate() {
        match {
            ::core::primitive::bool::then_some(boundaries
            .get(boundary_index)
            .is_some_and(|boundary| boundary.south_index == south_index), ())
        } {
            ::core::option::Option::Some(()) => {
                lower = lower.max(match boundaries[boundary_index].north_order {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => -1,
                });
                boundary_index += 1;
            },
            ::core::option::Option::None => {},
        }

        current.lower = lower;
    }

    let mut upper = north_len;
    let mut boundary_index = boundaries.len();

    for south_index in (0..south_len).rev() {
        bounds[south_index].upper = upper;

        match {
            ::core::primitive::bool::then_some(boundary_index > 0 && boundaries[boundary_index - 1].south_index == south_index, ())
        } {
            ::core::option::Option::Some(()) => {
                boundary_index -= 1;
                let boundary_upper = match boundaries[boundary_index].north_order {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => -1,
                };
                // Upstream repeatedly scans the remaining suffix through `north.length`, so a later
                // border can only narrow that cap.
                upper = north_len.min(boundary_upper);
            },
            ::core::option::Option::None => {},
        }
    }

    bounds
}

#[must_use]
fn first_dummy_predecessor<'a>(
    g: &'a Graph<NodeLabel, EdgeLabel, GraphLabel>,
    v: &str,
) -> Option<&'a str> {
    let mut out: Option<&'a str> = None;
    g.for_each_predecessor(v, |u| {
        match out {
            ::core::option::Option::Some(_) => {
                return;
            },
            ::core::option::Option::None => {},
        }

        match {
            ::core::primitive::bool::then_some(match g.node(u).map(|n| n.dummy.is_some()) {
                ::core::option::Option::Some(option_value) => option_value,
                ::core::option::Option::None => false,
            }, ())
        } {
            ::core::option::Option::Some(()) => {
                out = Some(u);
            },
            ::core::option::Option::None => {},
        }
    });
    out
}

#[must_use]
pub fn find_type1_conflicts(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &[Vec<String>],
) -> Conflicts {
    let mut conflicts: Conflicts = BTreeMap::new();

    match layering {
        [] => {
            return conflicts;
        },
        [_, ..] => {},
    }

    for i in 1..layering.len() {
        let prev_layer = &layering[i - 1];
        let layer = &layering[i];

        let mut k0: usize = 0;
        let mut scan_pos: usize = 0;
        let prev_layer_len = prev_layer.len();
        let last_node = layer.last().map(|s| s.as_str());

        for (idx, v) in layer.iter().enumerate() {
            let w = find_other_inner_segment_node(g, v);
            let k1 = match w
                .as_deref()
                .and_then(|w| g.node(w))
                .and_then(|n| n.order) {
                ::core::option::Option::Some(option_value) => option_value,
                ::core::option::Option::None => prev_layer_len,
            };

            match {
                ::core::primitive::bool::then_some(w.is_some() || last_node == Some(v.as_str()), ())
            } {
                ::core::option::Option::Some(()) => {
                    for scan_node in layer.iter().skip(scan_pos).take(idx + 1 - scan_pos) {
                        let scan_dummy = match g
                            .node(scan_node)
                            .map(|n| n.dummy.is_some()) {
                            ::core::option::Option::Some(option_value) => option_value,
                            ::core::option::Option::None => false,
                        };
                        g.for_each_predecessor(scan_node, |u| {
                            let u_label = {
                                let optional_u_label = g.node(u);

                                match optional_u_label {
                                    Some(matched_u_label) => matched_u_label,
                                    ::core::option::Option::None => return,
                                }
                            };
                            let u_pos = match u_label.order {
                                ::core::option::Option::Some(option_value) => option_value,
                                ::core::option::Option::None => 0,
                            };
                            let u_dummy = u_label.dummy.is_some();

                            match {
                                ::core::primitive::bool::then_some((u_pos < k0 || k1 < u_pos) && !(u_dummy && scan_dummy), ())
                            } {
                                ::core::option::Option::Some(()) => {
                                    add_conflict(&mut conflicts, u, scan_node);
                                },
                                ::core::option::Option::None => {},
                            }
                        });
                    }

                    scan_pos = idx + 1;
                    k0 = k1;
                },
                ::core::option::Option::None => {},
            }
        }
    }

    conflicts
}

#[must_use]
pub fn find_type2_conflicts(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &[Vec<String>],
) -> Conflicts {
    let mut conflicts: Conflicts = BTreeMap::new();

    match layering {
        [] => {
            return conflicts;
        },
        [_, ..] => {},
    }

    for i in 1..layering.len() {
        let north = &layering[i - 1];
        let south = &layering[i];
        let mut boundaries = Vec::new();

        'inner_for_2: for (south_index, v) in south.iter().enumerate() {
            let is_border = g
                .node(v)
                .and_then(|n| n.dummy.as_deref())
                .is_some_and(|d| d == "border");

            match {
                ::core::primitive::bool::then_some(!is_border, ())
            } {
                ::core::option::Option::Some(()) => {
                    continue 'inner_for_2;
                },
                ::core::option::Option::None => {},
            }

            {
                match g.first_predecessor(v) {
                    Some(u) => {
                        boundaries.push(Type2Boundary {
                            south_index,
                            north_order: g.node(u).and_then(|n| n.order).map(|n| n as isize),
                        });
                    },
                    ::core::option::Option::None => {},
                };
            }
        }

        let bounds = type2_conflict_bounds(south.len(), north.len() as isize, &boundaries);

        'inner_for_2: for (v, bounds) in south.iter().zip(bounds) {
            match {
                ::core::primitive::bool::then_some(g.node(v).and_then(|node| node.dummy.as_deref()).is_none(), ())
            } {
                ::core::option::Option::Some(()) => {
                    continue 'inner_for_2;
                },
                ::core::option::Option::None => {},
            }

            g.for_each_predecessor(v, |u| {
                let u_node = {
                    let optional_u_node = g.node(u);

                    match optional_u_node {
                        Some(matched_u_node) => matched_u_node,
                        ::core::option::Option::None => return,
                    }
                };

                match {
                    ::core::primitive::bool::then_some(u_node.dummy.is_some(), ())
                } {
                    ::core::option::Option::Some(()) => {
                        let u_order = (match u_node.order {
                            ::core::option::Option::Some(option_value) => option_value,
                            ::core::option::Option::None => 0,
                        }) as isize;

                        match {
                            ::core::primitive::bool::then_some(u_order < bounds.lower || u_order > bounds.upper, ())
                        } {
                            ::core::option::Option::Some(()) => {
                                add_conflict(&mut conflicts, u, v);
                            },
                            ::core::option::Option::None => {},
                        }
                    },
                    ::core::option::Option::None => {},
                }
            });
        }
    }

    conflicts
}

#[must_use]
fn find_other_inner_segment_node(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    v: &str,
) -> Option<String> {
    match {
        ::core::primitive::bool::then_some(match g.node(v).map(|n| n.dummy.is_some()) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => false,
        }, ())
    } {
        ::core::option::Option::Some(()) => {
            return first_dummy_predecessor(g, v).map(|u| u.to_string());
        },
        ::core::option::Option::None => {},
    }

    None
}

#[derive(Debug, Clone, PartialEq)]
pub struct Alignment {
    pub root: HashMap<String, String>,
    pub align: HashMap<String, String>,
}

#[must_use]
pub fn vertical_alignment<F>(
    _g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &[Vec<String>],
    conflicts: &Conflicts,
    neighbor_fn: F,
) -> Alignment
where
    F: Fn(&str) -> Vec<String>,
{
    let mut root: HashMap<String, String> = HashMap::default();
    let mut align: HashMap<String, String> = HashMap::default();
    let mut pos: HashMap<String, usize> = HashMap::default();

    for layer in layering {
        for (order, v) in layer.iter().enumerate() {
            root.insert(v.clone(), v.clone());
            align.insert(v.clone(), v.clone());
            pos.insert(v.clone(), order);
        }
    }

    for layer in layering {
        let mut prev_idx: isize = -1;

        'over_v_2: for v in layer {
            let mut ws = neighbor_fn(v);

            match {
                ::core::primitive::bool::then_some(ws.is_empty(), ())
            } {
                ::core::option::Option::Some(()) => {
                    continue 'over_v_2;
                },
                ::core::option::Option::None => {},
            }

            ws.sort_by_key(|w| match pos.get(w).copied() {
                ::core::option::Option::Some(option_value) => option_value,
                ::core::option::Option::None => usize::MAX,
            });

            let mp = (ws.len() - 1) as f64 / 2.0;
            let i0 = mp.floor() as usize;
            let i1 = mp.ceil() as usize;

            for w in ws.iter().take(i1 + 1).skip(i0) {
                let v_align = align.get(v).cloned().unwrap_or_else(|| v.clone());
                let w_pos = (match pos.get(w).copied() {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => usize::MAX,
                }) as isize;

                match {
                    ::core::primitive::bool::then_some(v_align == *v && prev_idx < w_pos && !has_conflict(conflicts, v, w), ())
                } {
                    ::core::option::Option::Some(()) => {
                        align.insert(w.clone(), v.clone());
                        let w_root = root.get(w).cloned().unwrap_or_else(|| w.clone());
                        align.insert(v.clone(), w_root.clone());
                        root.insert(v.clone(), w_root);
                        prev_idx = w_pos;
                    },
                    ::core::option::Option::None => {},
                }
            }
        }
    }

    Alignment { root, align }
}

const NONE: usize = usize::MAX;

#[derive(Clone, Copy, PartialEq, Eq)]
enum BorderType {
    Other,
    Left,
    Right,
}

impl BorderType {
    #[must_use]
    fn from_label(label: &NodeLabel) -> Self {
        match label.border_type.as_deref() {
            Some("borderLeft") => Self::Left,
            Some("borderRight") => Self::Right,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Copy)]
struct BkNode {
    graph_ix: usize,
    order: Option<usize>,
    layer_index: usize,
    forward_position: usize,
    metrics: SepNodeMetrics,
    is_border: bool,
    border_type: BorderType,
}

#[derive(Clone, Copy)]
struct Orientation {
    index: usize,
    reverse_layers: bool,
    reverse_inner: bool,
}

const ORIENTATIONS: [Orientation; 4] = [
    Orientation {
        index: 0,
        reverse_layers: false,
        reverse_inner: false,
    },
    Orientation {
        index: 1,
        reverse_layers: false,
        reverse_inner: true,
    },
    Orientation {
        index: 2,
        reverse_layers: true,
        reverse_inner: false,
    },
    Orientation {
        index: 3,
        reverse_layers: true,
        reverse_inner: true,
    },
];

enum NeighborOrder {
    // Every directional neighbor set belongs to one adjacent layer with unique positions, so the
    // Dagre stable-sort result can be materialized once and reversed by index when required.
    Preordered,
    // Mixed layers or repeated layering occurrences make Dagre's orientation-local position map
    // and stable tie order observable, so retain the exact per-orientation fallback.
    StableSortFallback {
        positions: Vec<usize>,
        scratch: Vec<usize>,
    },
}

impl NeighborOrder {
    #[must_use]
    fn is_preordered(&self) -> bool {
        matches!(self, Self::Preordered)
    }

    fn prepare_orientation(&mut self, layering: &[Vec<usize>], orientation: Orientation) {
        let Self::StableSortFallback { positions, .. } = self else {
            return;
        };

        positions.fill(NONE);

        for layer_offset in 0..layering.len() {
            let layer_index = match {
                ::core::primitive::bool::then_some(orientation.reverse_layers, ())
            } {
                ::core::option::Option::Some(()) => {
                    layering.len() - 1 - layer_offset
                },
                ::core::option::Option::None => {
                    layer_offset
                },
            };
            let layer = &layering[layer_index];

            for (order, layer_element) in <[_]>::iter(layer).enumerate() {
                let node = match {
                    ::core::primitive::bool::then_some(orientation.reverse_inner, ())
                } {
                    ::core::option::Option::Some(()) => {
                        layer[layer.len() - 1 - order]
                    },
                    ::core::option::Option::None => {
                        *layer_element
                    },
                };
                positions[node] = order;
            }
        }
    }

    fn median_candidates(
        &mut self,
        neighbors: &[usize],
        nodes: &[BkNode],
        layering: &[Vec<usize>],
        orientation: Orientation,
    ) -> [Option<(usize, usize)>; 2] {
        debug_assert!(!neighbors.is_empty());

        let lower_median = (neighbors.len() - 1) / 2;
        let upper_median = neighbors.len() / 2;

        match self {
            Self::Preordered => {
                let candidate_at = |index: usize| {
                    let neighbor = match {
                        ::core::primitive::bool::then_some(orientation.reverse_inner, ())
                    } {
                        ::core::option::Option::Some(()) => {
                            neighbors[neighbors.len() - 1 - index]
                        },
                        ::core::option::Option::None => {
                            neighbors[index]
                        },
                    };
                    let node = nodes[neighbor];
                    let position = match {
                        ::core::primitive::bool::then_some(orientation.reverse_inner, ())
                    } {
                        ::core::option::Option::Some(()) => {
                            layering[node.layer_index].len() - 1 - node.forward_position
                        },
                        ::core::option::Option::None => {
                            node.forward_position
                        },
                    };
                    (neighbor, position)
                };
                [
                    Some(candidate_at(lower_median)),
                    (upper_median != lower_median).then(|| candidate_at(upper_median)),
                ]
            }
            Self::StableSortFallback { positions, scratch } => {
                scratch.clear();
                scratch.extend_from_slice(neighbors);
                scratch.sort_by_key(|&neighbor| positions[neighbor]);
                let candidate_at = |index: usize| {
                    let neighbor = scratch[index];
                    (neighbor, positions[neighbor])
                };
                [
                    Some(candidate_at(lower_median)),
                    (upper_median != lower_median).then(|| candidate_at(upper_median)),
                ]
            }
        }
    }
}

fn charge_nonzero(work_control: &mut dyn WorkControl, units: usize) -> Result<(), WorkError> {
    match units {
        0 => {
            return Ok(());
        },
        _ => {},
    }

    work_control.charge(units)
}

fn local_sort_work(degree: usize) -> Result<usize, WorkError> {
    match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(degree), &::core::convert::identity(1)); _explicit_ordering } {
        ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Equal => {
            return Ok(0);
        },
        ::core::cmp::Ordering::Greater => {},
    }

    // Each vertical-alignment sort is local to one node's adjacency list. There is no graph-wide
    // node or edge sort in BK positioning.
    checked_mul(degree, ceil_log2(degree))
}

fn record_neighbor_layer(slot: &mut Option<usize>, layer_index: usize) -> bool {
    match *slot {
        Some(existing) => existing == layer_index,
        None => {
            *slot = Some(layer_index);
            true
        }
    }
}

fn record_local_sort_entry(total: &mut Result<usize, WorkError>, degree_before: usize) {
    *total = (*total).and_then(|current| {
        let degree_after = checked_add(degree_before, 1)?;
        let before = local_sort_work(degree_before)?;
        let after = local_sort_work(degree_after)?;
        checked_add(
            current,
            after
                .checked_sub(before)
                .ok_or(WorkError::ArithmeticOverflow)?,
        )
    });
}

fn layered_adjacency_work(
    layering: &[Vec<usize>],
    predecessors: &[Vec<usize>],
    successors: &[Vec<usize>],
) -> Result<(usize, usize, usize, usize), WorkError> {
    layering.iter().flatten().try_fold(
        (0usize, 0usize, 0usize, 0usize),
        |(predecessor_entries, successor_entries, predecessor_sort_work, successor_sort_work),
         &node| {
            let predecessor_degree = predecessors[node].len();
            let successor_degree = successors[node].len();
            Ok((
                checked_add(predecessor_entries, predecessor_degree)?,
                checked_add(successor_entries, successor_degree)?,
                checked_add(predecessor_sort_work, local_sort_work(predecessor_degree)?)?,
                checked_add(successor_sort_work, local_sort_work(successor_degree)?)?,
            ))
        },
    )
}

enum GraphToLocal {
    // A dense table is faster on the canonical graph, where participating graph indices are
    // contiguous. Never allocate it when tombstones make the index span larger than the live
    // participating entry budget.
    Dense(Vec<usize>),
    Sparse(HashMap<usize, usize>),
}

impl GraphToLocal {
    #[must_use]
    fn new(required_graph_slots: usize, capacity: usize) -> Self {
        match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(required_graph_slots), &::core::convert::identity(capacity)); _explicit_ordering } {
            ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Equal => {
                Self::Dense(vec![NONE; required_graph_slots])
            },
            ::core::cmp::Ordering::Greater => {
                let mut map = HashMap::default();
                map.reserve(capacity);
                Self::Sparse(map)
            },
        }
    }

    #[must_use]
    fn get(&self, graph_ix: usize) -> Option<usize> {
        match self {
            Self::Dense(values) => values
                .get(graph_ix)
                .copied()
                .filter(|&local_ix| local_ix != NONE),
            Self::Sparse(values) => values.get(&graph_ix).copied(),
        }
    }

    fn insert(&mut self, graph_ix: usize, local_ix: usize) {
        match self {
            Self::Dense(values) => values[graph_ix] = local_ix,
            Self::Sparse(values) => {
                values.insert(graph_ix, local_ix);
            }
        }
    }
}

struct BkWorkspace<'a> {
    g: &'a Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: Vec<Vec<usize>>,
    nodes: Vec<BkNode>,
    predecessors: Vec<Vec<usize>>,
    successors: Vec<Vec<usize>>,
    first_predecessor: Vec<Option<usize>>,
    first_dummy_predecessor: Vec<Option<usize>>,
    conflicts: HashSet<(usize, usize)>,
    neighbor_order: NeighborOrder,
    root: Vec<usize>,
    align: Vec<usize>,
    coords: [Vec<f64>; 4],
    root_to_block: Vec<usize>,
    block_roots: Vec<usize>,
    block_edges: HashMap<(usize, usize), f64>,
    block_edge_order: Vec<(usize, usize)>,
    block_predecessors: Vec<Vec<(usize, f64)>>,
    block_successors: Vec<Vec<(usize, f64)>>,
    block_x: Vec<f64>,
    scheduled: Vec<bool>,
    stack: Vec<(usize, bool)>,
    layer_entries: usize,
    adjacent_layer_pairs: usize,
    predecessor_entries: usize,
    successor_entries: usize,
    predecessor_sort_work: usize,
    successor_sort_work: usize,
}

impl<'a> BkWorkspace<'a> {
    #[must_use]
    fn new(
        g: &'a Graph<NodeLabel, EdgeLabel, GraphLabel>,
        source_layering: &[Vec<String>],
    ) -> Self {
        let mut work_control = NoopWorkControl;
        Self::new_controlled(g, source_layering, &mut work_control)
            .expect("the checked no-op Dugong work control cannot reject BK setup")
    }

    fn new_controlled(
        g: &'a Graph<NodeLabel, EdgeLabel, GraphLabel>,
        source_layering: &[Vec<String>],
        work_control: &mut dyn WorkControl,
    ) -> Result<Self, WorkError> {
        charge_nonzero(work_control, source_layering.len())?;
        let source_layer_entries = source_layering
            .iter()
            .try_fold(0usize, |total, layer| checked_add(total, layer.len()))?;
        let edge_scan_entries = match {
            ::core::primitive::bool::then_some(g.is_directed(), ())
        } {
            ::core::option::Option::Some(()) => {
                g.edge_slot_count()
            },
            ::core::option::Option::None => {
                // The slot-backed scan visits every tombstone once, then processes each live edge for
                // its second undirected endpoint.
                checked_add(g.edge_slot_count(), g.edge_count())?
            },
        };
        charge_nonzero(
            work_control,
            checked_add(
                checked_add(
                    checked_add(source_layering.len(), source_layer_entries)?,
                    source_layer_entries,
                )?,
                edge_scan_entries,
            )?,
        )?;

        // Graphlib can retain a wide slot span outside this layout operation. Bound the dense
        // lookup table to live nodes actually named by the source layering.
        let mut required_graph_slots = 0usize;
        let mut layering: Vec<Vec<usize>> = Vec::with_capacity(source_layering.len());

        for source_layer in source_layering {
            let mut layer = Vec::with_capacity(source_layer.len());

            'over_id_2: for id in source_layer {
                let graph_ix = {
                    let optional_graph_ix = g.node_ix(id);

                    match optional_graph_ix {
                        Some(matched_graph_ix) => matched_graph_ix,
                        ::core::option::Option::None => continue 'over_id_2,
                    }
                };
                required_graph_slots = required_graph_slots.max(checked_add(graph_ix, 1)?);
                layer.push(graph_ix);
            }

            layering.push(layer);
        }

        Self::new_indexed_owned_controlled(
            g,
            layering,
            source_layer_entries,
            required_graph_slots,
            work_control,
        )
    }

    fn new_indexed_controlled(
        g: &'a Graph<NodeLabel, EdgeLabel, GraphLabel>,
        source_layering: &[Vec<usize>],
        work_control: &mut dyn WorkControl,
    ) -> Result<Self, WorkError> {
        charge_nonzero(work_control, source_layering.len())?;
        let source_layer_entries = source_layering
            .iter()
            .try_fold(0usize, |total, layer| checked_add(total, layer.len()))?;
        let edge_scan_entries = match {
            ::core::primitive::bool::then_some(g.is_directed(), ())
        } {
            ::core::option::Option::Some(()) => {
                g.edge_slot_count()
            },
            ::core::option::Option::None => {
                checked_add(g.edge_slot_count(), g.edge_count())?
            },
        };
        charge_nonzero(
            work_control,
            checked_add(
                checked_add(source_layering.len(), source_layer_entries)?,
                edge_scan_entries,
            )?,
        )?;

        let mut required_graph_slots = 0usize;
        let mut layering = Vec::with_capacity(source_layering.len());

        for source_layer in source_layering {
            let mut layer = Vec::with_capacity(source_layer.len());

            'inner_for_2: for &graph_ix in source_layer {
                match {
                    ::core::primitive::bool::then_some(g.node_label_by_ix(graph_ix).is_none(), ())
                } {
                    ::core::option::Option::Some(()) => {
                        continue 'inner_for_2;
                    },
                    ::core::option::Option::None => {},
                }

                required_graph_slots = required_graph_slots.max(checked_add(graph_ix, 1)?);
                layer.push(graph_ix);
            }

            layering.push(layer);
        }

        Self::new_indexed_owned_controlled(
            g,
            layering,
            source_layer_entries,
            required_graph_slots,
            work_control,
        )
    }

    fn new_indexed_owned_controlled(
        g: &'a Graph<NodeLabel, EdgeLabel, GraphLabel>,
        mut layering: Vec<Vec<usize>>,
        source_layer_entries: usize,
        required_graph_slots: usize,
        work_control: &mut dyn WorkControl,
    ) -> Result<Self, WorkError> {
        let lookup_capacity = source_layer_entries.min(g.node_count());

        charge_nonzero(
            work_control,
            checked_add(
                required_graph_slots.min(lookup_capacity),
                source_layer_entries,
            )?,
        )?;
        let mut graph_to_local = GraphToLocal::new(required_graph_slots, lookup_capacity);
        let mut nodes: Vec<BkNode> = Vec::with_capacity(source_layer_entries.min(g.node_count()));
        let mut can_preorder_neighbors = true;
        let mut layer_entries = 0usize;
        let mut adjacent_layer_pairs = 0usize;

        for (layer_index, layer) in layering.iter_mut().enumerate() {
            'inner_for_2: for (position, graph_ix) in layer.iter_mut().enumerate() {
                match graph_to_local.get(*graph_ix) {
                    Some(local_ix) => {
                        // Repeated layering occurrences make orientation-specific stable tie order
                        // observable. Preserve Dagre's per-orientation stable-sort path in that case.
                        can_preorder_neighbors = false;
                        *graph_ix = local_ix;
                        continue 'inner_for_2;
                    },
                    ::core::option::Option::None => {},
                }

                let label = g
                    .node_label_by_ix(*graph_ix)
                    .expect("a live graph node index must retain its label");

                let local_ix = nodes.len();
                graph_to_local.insert(*graph_ix, local_ix);
                nodes.push(BkNode {
                    graph_ix: *graph_ix,
                    order: label.order,
                    layer_index,
                    forward_position: position,
                    metrics: SepNodeMetrics::from(label),
                    is_border: label.dummy.as_deref() == Some("border"),
                    border_type: BorderType::from_label(label),
                });
                *graph_ix = local_ix;
            }

            layer_entries = checked_add(layer_entries, layer.len())?;
            adjacent_layer_pairs =
                checked_add(adjacent_layer_pairs, layer.len().saturating_sub(1))?;
        }

        let node_count = nodes.len();
        charge_nonzero(work_control, node_count)?;
        let mut predecessors = vec![Vec::new(); node_count];
        let mut successors = vec![Vec::new(); node_count];
        let mut first_predecessor = vec![None; node_count];
        let mut first_dummy_predecessor = vec![None; node_count];
        let mut saw_predecessor = vec![false; node_count];
        let mut saw_dummy_predecessor = vec![false; node_count];
        let mut predecessor_layers = vec![None; node_count];
        let mut successor_layers = vec![None; node_count];
        let mut predecessor_entries_acc = Ok(0usize);
        let mut successor_entries_acc = Ok(0usize);
        let mut predecessor_sort_work_acc = Ok(0usize);
        let mut successor_sort_work_acc = Ok(0usize);

        match {
            ::core::primitive::bool::then_some(g.is_directed(), ())
        } {
            ::core::option::Option::Some(()) => {
                // Graphlib exposes predecessor and successor *nodes*, not edge instances. Named
                // parallel edges therefore contribute one BK neighbor even though the slot-backed
                // scan still visits and charges every edge slot.
                let mut seen_adjacencies: HashSet<(usize, usize)> = HashSet::default();
                g.for_each_edge_ix(|from_graph_ix, to_graph_ix, _, _| {
                    let to_local = graph_to_local
                        .get(to_graph_ix)
                        .filter(|&ix| ix < node_count);
                    let to_local = match to_local {
                        Some(matched_to_local) => matched_to_local,
                        ::core::option::Option::None => return,
                    };

                    match {
                        ::core::primitive::bool::then_some(!seen_adjacencies.insert((from_graph_ix, to_graph_ix)), ())
                    } {
                        ::core::option::Option::Some(()) => {
                            return;
                        },
                        ::core::option::Option::None => {},
                    }

                    let from_local = graph_to_local
                        .get(from_graph_ix)
                        .filter(|&ix| ix < node_count);

                    match {
                        ::core::primitive::bool::then_some(!saw_predecessor[to_local], ())
                    } {
                        ::core::option::Option::Some(()) => {
                            saw_predecessor[to_local] = true;
                            first_predecessor[to_local] = from_local;
                        },
                        ::core::option::Option::None => {},
                    }

                    let from_is_dummy = from_local
                        .map(|from_local| nodes[from_local].metrics.is_dummy)
                        .unwrap_or_else(|| {
                            g.node_label_by_ix(from_graph_ix)
                                .is_some_and(|label| label.dummy.is_some())
                        });

                    match {
                        ::core::primitive::bool::then_some(from_is_dummy && !saw_dummy_predecessor[to_local], ())
                    } {
                        ::core::option::Option::Some(()) => {
                            saw_dummy_predecessor[to_local] = true;
                            first_dummy_predecessor[to_local] = from_local;
                        },
                        ::core::option::Option::None => {},
                    }

                    match from_local {
                        Some(from_local) => {
                            let from_layer = nodes[from_local].layer_index;
                            let to_layer = nodes[to_local].layer_index;
                            let adjacent = from_layer.abs_diff(to_layer) == 1;
                            let predecessor_layer_ok =
                                record_neighbor_layer(&mut predecessor_layers[to_local], from_layer);
                            let successor_layer_ok =
                                record_neighbor_layer(&mut successor_layers[from_local], to_layer);
                            can_preorder_neighbors &=
                                adjacent && predecessor_layer_ok && successor_layer_ok;
                            record_local_sort_entry(
                                &mut predecessor_sort_work_acc,
                                predecessors[to_local].len(),
                            );
                            record_local_sort_entry(
                                &mut successor_sort_work_acc,
                                successors[from_local].len(),
                            );
                            predecessor_entries_acc =
                                predecessor_entries_acc.and_then(|total| checked_add(total, 1));
                            successor_entries_acc =
                                successor_entries_acc.and_then(|total| checked_add(total, 1));
                            predecessors[to_local].push(from_local);
                            successors[from_local].push(to_local);
                        },
                        ::core::option::Option::None => {},
                    }
                });
            },
            ::core::option::Option::None => {
                let mut seen_neighbors: Vec<HashSet<usize>> =
                    (0..node_count).map(|_| HashSet::default()).collect();
                g.for_each_edge_ix(|from_graph_ix, to_graph_ix, _, _| {
                    for (node_graph_ix, neighbor_graph_ix) in
                        [(from_graph_ix, to_graph_ix), (to_graph_ix, from_graph_ix)]
                    {
                        let node_local = graph_to_local
                            .get(node_graph_ix)
                            .filter(|&ix| ix < node_count);
                        let node_local = match node_local {
                            Some(matched_node_local) => matched_node_local,
                            ::core::option::Option::None => continue,
                        };

                        match {
                            ::core::primitive::bool::then_some(!seen_neighbors[node_local].insert(neighbor_graph_ix), ())
                        } {
                            ::core::option::Option::Some(()) => {
                                continue;
                            },
                            ::core::option::Option::None => {},
                        }

                        let neighbor_local = graph_to_local
                            .get(neighbor_graph_ix)
                            .filter(|&ix| ix < node_count);

                        match {
                            ::core::primitive::bool::then_some(!saw_predecessor[node_local], ())
                        } {
                            ::core::option::Option::Some(()) => {
                                saw_predecessor[node_local] = true;
                                first_predecessor[node_local] = neighbor_local;
                            },
                            ::core::option::Option::None => {},
                        }

                        let neighbor_is_dummy = neighbor_local
                            .map(|neighbor_local| nodes[neighbor_local].metrics.is_dummy)
                            .unwrap_or_else(|| {
                                g.node_label_by_ix(neighbor_graph_ix)
                                    .is_some_and(|label| label.dummy.is_some())
                            });

                        match {
                            ::core::primitive::bool::then_some(neighbor_is_dummy && !saw_dummy_predecessor[node_local], ())
                        } {
                            ::core::option::Option::Some(()) => {
                                saw_dummy_predecessor[node_local] = true;
                                first_dummy_predecessor[node_local] = neighbor_local;
                            },
                            ::core::option::Option::None => {},
                        }

                        match neighbor_local {
                            Some(neighbor_local) => {
                                let node_layer = nodes[node_local].layer_index;
                                let neighbor_layer = nodes[neighbor_local].layer_index;
                                let adjacent = node_layer.abs_diff(neighbor_layer) == 1;
                                let predecessor_layer_ok = record_neighbor_layer(
                                    &mut predecessor_layers[node_local],
                                    neighbor_layer,
                                );
                                let successor_layer_ok = record_neighbor_layer(
                                    &mut successor_layers[node_local],
                                    neighbor_layer,
                                );
                                can_preorder_neighbors &=
                                    adjacent && predecessor_layer_ok && successor_layer_ok;
                                record_local_sort_entry(
                                    &mut predecessor_sort_work_acc,
                                    predecessors[node_local].len(),
                                );
                                record_local_sort_entry(
                                    &mut successor_sort_work_acc,
                                    successors[node_local].len(),
                                );
                                predecessor_entries_acc =
                                    predecessor_entries_acc.and_then(|total| checked_add(total, 1));
                                successor_entries_acc =
                                    successor_entries_acc.and_then(|total| checked_add(total, 1));
                                predecessors[node_local].push(neighbor_local);
                                successors[node_local].push(neighbor_local);
                            },
                            ::core::option::Option::None => {},
                        }
                    }
                });
            },
        }

        let (predecessor_entries, successor_entries, predecessor_sort_work, successor_sort_work) =
            match ::core::primitive::bool::then_some(can_preorder_neighbors, ()) {
                ::core::option::Option::Some(()) => {
                    (
                        predecessor_entries_acc?,
                        successor_entries_acc?,
                        predecessor_sort_work_acc?,
                        successor_sort_work_acc?,
                    )
                },
                ::core::option::Option::None => {
                    charge_nonzero(work_control, layer_entries)?;
                    layered_adjacency_work(&layering, &predecessors, &successors)?
                },
            };

        match ::core::primitive::bool::then_some(can_preorder_neighbors, ()) {
            ::core::option::Option::Some(()) => {
                charge_nonzero(
                    work_control,
                    checked_add(predecessor_sort_work, successor_sort_work)?,
                )?;

                // With one occurrence per node and adjacent-layer neighbors, every position is unique.
                // Dagre's stable ascending order can therefore be reused directly for left-biased
                // orientations and traversed backwards for right-biased orientations.
                for neighbors in &mut predecessors {
                    neighbors.sort_by_key(|&neighbor| nodes[neighbor].forward_position);
                }

                for neighbors in &mut successors {
                    neighbors.sort_by_key(|&neighbor| nodes[neighbor].forward_position);
                }
            },
            ::core::option::Option::None => {},
        }

        let neighbor_order = match ::core::primitive::bool::then_some(can_preorder_neighbors, ()) {
            ::core::option::Option::Some(()) => {
                NeighborOrder::Preordered
            },
            ::core::option::Option::None => {
                NeighborOrder::StableSortFallback {
                    positions: vec![NONE; node_count],
                    scratch: Vec::new(),
                }
            },
        };

        Ok(Self {
            g,
            layering,
            nodes,
            predecessors,
            successors,
            first_predecessor,
            first_dummy_predecessor,
            conflicts: HashSet::default(),
            neighbor_order,
            root: (0..node_count).collect(),
            align: (0..node_count).collect(),
            coords: std::array::from_fn(|_| vec![0.0; node_count]),
            root_to_block: vec![NONE; node_count],
            block_roots: Vec::new(),
            block_edges: HashMap::default(),
            block_edge_order: Vec::new(),
            block_predecessors: vec![Vec::new(); node_count],
            block_successors: vec![Vec::new(); node_count],
            block_x: vec![0.0; node_count],
            scheduled: vec![false; node_count],
            stack: Vec::new(),
            layer_entries,
            adjacent_layer_pairs,
            predecessor_entries,
            successor_entries,
            predecessor_sort_work,
            successor_sort_work,
        })
    }

    #[must_use]
    fn run(self) -> HashMap<String, f64> {
        let mut work_control = NoopWorkControl;
        self.run_controlled(&mut work_control)
            .expect("the checked no-op Dugong work control cannot reject BK positioning")
    }

    fn run_controlled(
        self,
        work_control: &mut dyn WorkControl,
    ) -> Result<HashMap<String, f64>, WorkError> {
        let graph = self.g;
        let indexed = self.run_indexed_controlled(work_control)?;
        charge_nonzero(work_control, checked_mul(indexed.len(), 2)?)?;
        let mut out = HashMap::default();
        out.reserve(indexed.len());

        for (graph_ix, x) in indexed {
            {
                match graph.node_id_by_ix(graph_ix) {
                    Some(id) => {
                        out.insert(id.to_string(), x);
                    },
                    ::core::option::Option::None => {},
                };
            }
        }

        Ok(out)
    }

    fn run_indexed_controlled(
        mut self,
        work_control: &mut dyn WorkControl,
    ) -> Result<Vec<(usize, f64)>, WorkError> {
        match {
            ::core::primitive::bool::then_some(self.nodes.is_empty(), ())
        } {
            ::core::option::Option::Some(()) => {
                return Ok(Vec::new());
            },
            ::core::option::Option::None => {},
        }

        charge_nonzero(work_control, self.conflict_work_units()?)?;
        self.find_conflicts();

        for orientation in ORIENTATIONS {
            charge_nonzero(
                work_control,
                self.vertical_alignment_work_units(orientation)?,
            )?;
            charge_nonzero(work_control, self.neighbor_sort_work_units(orientation))?;
            self.vertical_alignment(orientation);
            charge_nonzero(work_control, self.horizontal_compaction_work_units()?)?;
            self.horizontal_compaction(orientation);
        }

        charge_nonzero(
            work_control,
            checked_mul(self.nodes.len(), ORIENTATIONS.len())?,
        )?;
        let smallest = self.find_smallest_width_alignment();
        charge_nonzero(
            work_control,
            checked_mul(self.nodes.len(), ORIENTATIONS.len())?,
        )?;
        self.align_coordinates(smallest);
        charge_nonzero(work_control, self.nodes.len())?;
        Ok(self.balance_indexed())
    }

    fn conflict_work_units(&self) -> Result<usize, WorkError> {
        // Type-1 scans layering and predecessors once. Type-2 additionally builds and traverses
        // its per-layer boundary intervals before its predecessor scan.
        checked_add(
            checked_mul(self.layer_entries, 4)?,
            checked_mul(self.predecessor_entries, 2)?,
        )
    }

    fn vertical_alignment_work_units(&self, orientation: Orientation) -> Result<usize, WorkError> {
        match {
            ::core::primitive::bool::then_some(self.neighbor_order.is_preordered(), ())
        } {
            ::core::option::Option::Some(()) => {
                // Positions and stable neighbor order are operation-owned setup artifacts. Each
                // orientation retains the root/alignment reset, node traversal, and up to two median
                // probes per layering occurrence without rebuilding positions or adjacency scratch.
                return checked_add(self.nodes.len(), checked_mul(self.layer_entries, 3)?);
            },
            ::core::option::Option::None => {},
        }

        let directional_entries = match {
            ::core::primitive::bool::then_some(orientation.reverse_layers, ())
        } {
            ::core::option::Option::Some(()) => {
                self.successor_entries
            },
            ::core::option::Option::None => {
                self.predecessor_entries
            },
        };
        // One workspace reset, one position pass, one node pass, up to two median probes per
        // layering occurrence, and the selected local adjacency entries. The k log k sort tranche
        // is charged separately immediately before execution.
        checked_add(
            checked_add(self.nodes.len(), checked_mul(self.layer_entries, 4)?)?,
            directional_entries,
        )
    }

    #[must_use]
    fn neighbor_sort_work_units(&self, orientation: Orientation) -> usize {
        match {
            ::core::primitive::bool::then_some(self.neighbor_order.is_preordered(), ())
        } {
            ::core::option::Option::Some(()) => {
                return 0;
            },
            ::core::option::Option::None => {},
        }

        match {
            ::core::primitive::bool::then_some(orientation.reverse_layers, ())
        } {
            ::core::option::Option::Some(()) => {
                self.successor_sort_work
            },
            ::core::option::Option::None => {
                self.predecessor_sort_work
            },
        }
    }

    #[cfg(test)]
    fn total_neighbor_sort_work_units(&self) -> Result<usize, WorkError> {
        let one_direction_pair = checked_add(self.predecessor_sort_work, self.successor_sort_work)?;
        if self.neighbor_order.is_preordered() {
            Ok(one_direction_pair)
        } else {
            checked_mul(one_direction_pair, 2)
        }
    }

    fn horizontal_compaction_work_units(&self) -> Result<usize, WorkError> {
        // Compaction builds and traverses a block graph from workspace nodes and adjacent layer
        // pairs. Charge both materialization and traversal; it never sorts the complete edge set.
        checked_mul(
            checked_add(
                checked_add(self.nodes.len(), self.layer_entries)?,
                self.adjacent_layer_pairs,
            )?,
            2,
        )
    }

    fn find_conflicts(&mut self) {
        self.find_type1_conflicts();
        self.find_type2_conflicts();
    }

    fn add_conflict(&mut self, v: usize, w: usize) {
        let conflict = match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(v), &::core::convert::identity(w)); _explicit_ordering } {
            ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Equal => { (v, w) },
            ::core::cmp::Ordering::Greater => { (w, v) },
        };
        self.conflicts.insert(conflict);
    }

    #[must_use]
    fn has_conflict(&self, v: usize, w: usize) -> bool {
        let conflict = match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(v), &::core::convert::identity(w)); _explicit_ordering } {
            ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Equal => { (v, w) },
            ::core::cmp::Ordering::Greater => { (w, v) },
        };
        self.conflicts.contains(&conflict)
    }

    fn find_type1_conflicts(&mut self) {
        for layer_index in 1..self.layering.len() {
            let prev_layer_len = self.layering[layer_index - 1].len();
            let layer_len = self.layering[layer_index].len();

            match layer_len {
                0 => {
                    continue;
                },
                _ => {},
            }

            let mut k0 = 0;
            let mut scan_pos = 0;

            for index in 0..layer_len {
                let v = self.layering[layer_index][index];
                let inner_segment = self.nodes[v]
                    .metrics
                    .is_dummy
                    .then_some(self.first_dummy_predecessor[v])
                    .flatten();
                let k1 = match inner_segment
                    .and_then(|w| self.nodes[w].order) {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => prev_layer_len,
                };

                match {
                    ::core::primitive::bool::then_some(inner_segment.is_some() || index + 1 == layer_len, ())
                } {
                    ::core::option::Option::Some(()) => {
                        for scan_index in scan_pos..=index {
                            let scan_node = self.layering[layer_index][scan_index];
                            let scan_dummy = self.nodes[scan_node].metrics.is_dummy;

                            for predecessor_index in 0..self.predecessors[scan_node].len() {
                                let u = self.predecessors[scan_node][predecessor_index];
                                let u_pos = match self.nodes[u].order {
                                    ::core::option::Option::Some(option_value) => option_value,
                                    ::core::option::Option::None => 0,
                                };
                                let u_dummy = self.nodes[u].metrics.is_dummy;

                                match {
                                    ::core::primitive::bool::then_some((u_pos < k0 || k1 < u_pos) && !(u_dummy && scan_dummy), ())
                                } {
                                    ::core::option::Option::Some(()) => {
                                        self.add_conflict(u, scan_node);
                                    },
                                    ::core::option::Option::None => {},
                                }
                            }
                        }

                        scan_pos = index + 1;
                        k0 = k1;
                    },
                    ::core::option::Option::None => {},
                }
            }
        }
    }

    fn find_type2_conflicts(&mut self) {
        for layer_index in 1..self.layering.len() {
            let north_len = self.layering[layer_index - 1].len() as isize;
            let south_len = self.layering[layer_index].len();
            let mut boundaries = Vec::new();

            'over_south_index_2: for south_index in 0..south_len {
                let v = self.layering[layer_index][south_index];

                match {
                    ::core::primitive::bool::then_some(!self.nodes[v].is_border, ())
                } {
                    ::core::option::Option::Some(()) => {
                        continue 'over_south_index_2;
                    },
                    ::core::option::Option::None => {},
                }

                let Some(u) = self.first_predecessor[v] else {
                    continue 'over_south_index_2;
                };

                boundaries.push(Type2Boundary {
                    south_index,
                    north_order: self.nodes[u].order.map(|order| order as isize),
                });
            }

            let bounds = type2_conflict_bounds(south_len, north_len, &boundaries);

            'inner_for_2: for (index, bounds) in bounds.into_iter().enumerate() {
                let v = self.layering[layer_index][index];

                match {
                    ::core::primitive::bool::then_some(!self.nodes[v].metrics.is_dummy, ())
                } {
                    ::core::option::Option::Some(()) => {
                        continue 'inner_for_2;
                    },
                    ::core::option::Option::None => {},
                }

                for predecessor_index in 0..self.predecessors[v].len() {
                    let u = self.predecessors[v][predecessor_index];

                    match {
                        ::core::primitive::bool::then_some(self.nodes[u].metrics.is_dummy, ())
                    } {
                        ::core::option::Option::Some(()) => {
                            let u_order = (match self.nodes[u].order {
                                ::core::option::Option::Some(option_value) => option_value,
                                ::core::option::Option::None => 0,
                            }) as isize;

                            match {
                                ::core::primitive::bool::then_some(u_order < bounds.lower || u_order > bounds.upper, ())
                            } {
                                ::core::option::Option::Some(()) => {
                                    self.add_conflict(u, v);
                                },
                                ::core::option::Option::None => {},
                            }
                        },
                        ::core::option::Option::None => {},
                    }
                }
            }
        }
    }

    #[must_use]
    fn oriented_layer_index(&self, orientation: Orientation, index: usize) -> usize {
        match {
            ::core::primitive::bool::then_some(orientation.reverse_layers, ())
        } {
            ::core::option::Option::Some(()) => {
                self.layering.len() - 1 - index
            },
            ::core::option::Option::None => {
                index
            },
        }
    }

    #[must_use]
    fn oriented_node(&self, orientation: Orientation, layer_index: usize, index: usize) -> usize {
        let layer = &self.layering[layer_index];

        match {
            ::core::primitive::bool::then_some(orientation.reverse_inner, ())
        } {
            ::core::option::Option::Some(()) => {
                layer[layer.len() - 1 - index]
            },
            ::core::option::Option::None => {
                layer[index]
            },
        }
    }

    fn vertical_alignment(&mut self, orientation: Orientation) {
        for node in 0..self.nodes.len() {
            self.root[node] = node;
            self.align[node] = node;
        }

        self.neighbor_order
            .prepare_orientation(&self.layering, orientation);

        for index in 0..self.layering.len() {
            let layer_index = self.oriented_layer_index(orientation, index);
            let mut prev_idx = -1;

            'over_order_2: for order in 0..self.layering[layer_index].len() {
                let v = self.oriented_node(orientation, layer_index, order);
                let neighbors = match {
                    ::core::primitive::bool::then_some(orientation.reverse_layers, ())
                } {
                    ::core::option::Option::Some(()) => {
                        &self.successors[v]
                    },
                    ::core::option::Option::None => {
                        &self.predecessors[v]
                    },
                };

                match {
                    ::core::primitive::bool::then_some(neighbors.is_empty(), ())
                } {
                    ::core::option::Option::Some(()) => {
                        continue 'over_order_2;
                    },
                    ::core::option::Option::None => {},
                }

                let candidates = self.neighbor_order.median_candidates(
                    neighbors,
                    &self.nodes,
                    &self.layering,
                    orientation,
                );

                for (w, w_pos) in candidates.into_iter().flatten() {
                    let w_pos = w_pos as isize;

                    match {
                        ::core::primitive::bool::then_some(self.align[v] == v && prev_idx < w_pos && !self.has_conflict(v, w), ())
                    } {
                        ::core::option::Option::Some(()) => {
                            self.align[w] = v;
                            self.align[v] = self.root[w];
                            self.root[v] = self.root[w];
                            prev_idx = w_pos;
                        },
                        ::core::option::Option::None => {},
                    }
                }
            }
        }
    }

    fn ensure_block(&mut self, root: usize) -> usize {
        let block = self.root_to_block[root];

        match {
            ::core::primitive::bool::then_some(block != NONE, ())
        } {
            ::core::option::Option::Some(()) => {
                return block;
            },
            ::core::option::Option::None => {},
        }

        let block = self.block_roots.len();
        self.root_to_block[root] = block;
        self.block_roots.push(root);
        block
    }

    fn horizontal_compaction(&mut self, orientation: Orientation) {
        self.root_to_block.fill(NONE);
        self.block_roots.clear();
        self.block_edges.clear();
        self.block_edge_order.clear();

        let node_sep = self.g.graph().nodesep;
        let edge_sep = self.g.graph().edgesep;

        for index in 0..self.layering.len() {
            let layer_index = self.oriented_layer_index(orientation, index);
            let mut previous: Option<usize> = None;

            for order in 0..self.layering[layer_index].len() {
                let v = self.oriented_node(orientation, layer_index, order);
                let v_root = self.root[v];
                let v_block = self.ensure_block(v_root);

                match previous {
                    Some(u) => {
                        let u_root = self.root[u];
                        let u_block = self.ensure_block(u_root);
                        let separation = sep_metrics(
                            self.nodes[v].metrics,
                            self.nodes[u].metrics,
                            node_sep,
                            edge_sep,
                            orientation.reverse_inner,
                        );

                        if let Some(weight) = self.block_edges.get_mut(&(u_block, v_block)) {
                            match {
                                ::core::primitive::bool::then_some(separation > *weight, ())
                            } {
                                ::core::option::Option::Some(()) => {
                                    *weight = separation;
                                },
                                ::core::option::Option::None => {},
                            }
                        } else {
                            self.block_edges.insert((u_block, v_block), separation);
                            self.block_edge_order.push((u_block, v_block));
                        }
                    },
                    ::core::option::Option::None => {},
                }

                previous = Some(v);
            }
        }

        let block_count = self.block_roots.len();

        for predecessors in &mut self.block_predecessors[..block_count] {
            predecessors.clear();
        }

        for successors in &mut self.block_successors[..block_count] {
            successors.clear();
        }

        for &(u, v) in &self.block_edge_order {
            let weight = self.block_edges[&(u, v)];
            self.block_predecessors[v].push((u, weight));
            self.block_successors[u].push((v, weight));
        }

        self.block_x[..block_count].fill(0.0);

        self.scheduled[..block_count].fill(false);
        self.stack.clear();
        self.stack
            .extend((0..block_count).map(|block| (block, false)));

        while let Some((block, expanded)) = self.stack.pop() {
            match ::core::primitive::bool::then_some(expanded, ()) {
                ::core::option::Option::Some(()) => {
                    let mut best: f64 = 0.0;

                    for &(predecessor, weight) in &self.block_predecessors[block] {
                        best = best.max(self.block_x[predecessor] + weight);
                    }

                    self.block_x[block] = best;
                    continue;
                },
                ::core::option::Option::None => {},
            }

            match {
                ::core::primitive::bool::then_some(self.scheduled[block], ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            self.scheduled[block] = true;
            self.stack.push((block, true));

            for &(predecessor, _) in &self.block_predecessors[block] {
                self.stack.push((predecessor, false));
            }
        }

        let excluded_border = match {
            ::core::primitive::bool::then_some(orientation.reverse_inner, ())
        } {
            ::core::option::Option::Some(()) => {
                BorderType::Left
            },
            ::core::option::Option::None => {
                BorderType::Right
            },
        };
        self.scheduled[..block_count].fill(false);
        self.stack.clear();
        self.stack
            .extend((0..block_count).map(|block| (block, false)));

        while let Some((block, expanded)) = self.stack.pop() {
            match ::core::primitive::bool::then_some(expanded, ()) {
                ::core::option::Option::Some(()) => {
                    let mut min: f64 = f64::INFINITY;

                    for &(successor, weight) in &self.block_successors[block] {
                        min = min.min(self.block_x[successor] - weight);
                    }

                    let root = self.block_roots[block];

                    match {
                        ::core::primitive::bool::then_some(min.is_finite() && self.nodes[root].border_type != excluded_border, ())
                    } {
                        ::core::option::Option::Some(()) => {
                            self.block_x[block] = self.block_x[block].max(min);
                        },
                        ::core::option::Option::None => {},
                    }

                    continue;
                },
                ::core::option::Option::None => {},
            }

            match {
                ::core::primitive::bool::then_some(self.scheduled[block], ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            self.scheduled[block] = true;
            self.stack.push((block, true));

            for &(successor, _) in &self.block_successors[block] {
                self.stack.push((successor, false));
            }
        }

        let xs = &mut self.coords[orientation.index];

        for (node, x) in xs.iter_mut().enumerate() {
            let block = self.root_to_block[self.root[node]];
            *x = match {
                ::core::primitive::bool::then_some(block == NONE, ())
            } {
                ::core::option::Option::Some(()) => {
                    0.0
                },
                ::core::option::Option::None => {
                    self.block_x[block]
                },
            };
        }

        match {
            ::core::primitive::bool::then_some(orientation.reverse_inner, ())
        } {
            ::core::option::Option::Some(()) => {
                for x in xs {
                    *x = -*x;
                }
            },
            ::core::option::Option::None => {},
        }
    }

    #[must_use]
    fn find_smallest_width_alignment(&self) -> usize {
        let mut best = 0;
        let mut best_width = f64::INFINITY;

        for orientation in ORIENTATIONS {
            let mut max = f64::NEG_INFINITY;
            let mut min = f64::INFINITY;

            for (node, &x) in self.coords[orientation.index].iter().enumerate() {
                let half_width = self.nodes[node].metrics.width / 2.0;
                max = max.max(x + half_width);
                min = min.min(x - half_width);
            }

            let width = max - min;

            match {
                ::core::primitive::bool::then_some(width < best_width, ())
            } {
                ::core::option::Option::Some(()) => {
                    best_width = width;
                    best = orientation.index;
                },
                ::core::option::Option::None => {},
            }
        }

        best
    }

    fn align_coordinates(&mut self, align_to: usize) {
        let align_to_min = self.coords[align_to]
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let align_to_max = self.coords[align_to]
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);

        for orientation in ORIENTATIONS {
            let xs_min = self.coords[orientation.index]
                .iter()
                .copied()
                .fold(f64::INFINITY, f64::min);
            let xs_max = self.coords[orientation.index]
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            let delta = match {
                ::core::primitive::bool::then_some(orientation.reverse_inner, ())
            } {
                ::core::option::Option::Some(()) => {
                    align_to_max - xs_max
                },
                ::core::option::Option::None => {
                    align_to_min - xs_min
                },
            };

            match {
                ::core::primitive::bool::then_some(delta != 0.0, ())
            } {
                ::core::option::Option::Some(()) => {
                    for x in &mut self.coords[orientation.index] {
                        *x += delta;
                    }
                },
                ::core::option::Option::None => {},
            }
        }
    }

    #[must_use]
    fn balance_indexed(&self) -> Vec<(usize, f64)> {
        let align = self.g.graph().align.as_deref().map(str::to_ascii_lowercase);
        let selected = match align.as_deref() {
            Some("ul") => Some(0),
            Some("ur") => Some(1),
            Some("dl") => Some(2),
            Some("dr") => Some(3),
            Some(_) => None,
            None => None,
        };
        let invalid_alignment = align.is_some() && selected.is_none();

        let mut out = Vec::with_capacity(self.nodes.len());

        for node in 0..self.nodes.len() {
            let x = match ::core::primitive::bool::then_some(invalid_alignment, ()) {
                ::core::option::Option::Some(()) => {
                    0.0
                },
                ::core::option::Option::None => match selected {
                    Some(selected) => {
                        self.coords[selected][node]
                    },
                    ::core::option::Option::None => {
                        let mut values = [
                            self.coords[0][node],
                            self.coords[1][node],
                            self.coords[2][node],
                            self.coords[3][node],
                        ];
                        values.sort_by(f64::total_cmp);
                        ((match values { [_, _explicit_element, ..] => _explicit_element }) + (match values { [_, _, _explicit_element, ..] => _explicit_element })) / 2.0
                    },
                },
            };
            out.push((self.nodes[node].graph_ix, x));
        }

        out
    }
}

#[must_use]
pub fn position_x_with_layering(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &[Vec<String>],
) -> HashMap<String, f64> {
    BkWorkspace::new(g, layering).run()
}

/// Computes BK horizontal coordinates under caller-owned work control.
///
/// Setup, conflict discovery, each orientation's local adjacency sorts and compaction, and final
/// coordinate reconciliation are charged before their derived allocations or work execute.
pub fn position_x_with_layering_controlled(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &[Vec<String>],
    work_control: &mut dyn WorkControl,
) -> Result<HashMap<String, f64>, WorkError> {
    BkWorkspace::new_controlled(g, layering, work_control)?.run_controlled(work_control)
}

pub(crate) fn position_x_with_indexed_layering_controlled(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &[Vec<usize>],
    work_control: &mut dyn WorkControl,
) -> Result<Vec<(usize, f64)>, WorkError> {
    BkWorkspace::new_indexed_controlled(g, layering, work_control)?
        .run_indexed_controlled(work_control)
}

#[must_use]
pub fn horizontal_compaction(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &[Vec<String>],
    root: &HashMap<String, String>,
    align: &HashMap<String, String>,
    reverse_sep: bool,
) -> HashMap<String, f64> {
    let root_ref: HashMap<&'_ str, &'_ str> =
        root.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let align_ref: HashMap<&'_ str, &'_ str> = align
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    horizontal_compaction_ref(g, layering, &root_ref, &align_ref, reverse_sep)
}

#[must_use]
fn horizontal_compaction_ref<'a>(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &[Vec<String>],
    root: &HashMap<&'a str, &'a str>,
    align: &HashMap<&'a str, &'a str>,
    reverse_sep: bool,
) -> HashMap<String, f64> {
    let mut xs: HashMap<String, f64> = HashMap::default();
    let block_g = build_block_graph_ref(g, layering, root, reverse_sep);
    let border_type = match ::core::primitive::bool::then_some(reverse_sep, ()) {
        ::core::option::Option::Some(()) => {
            "borderLeft"
        },
        ::core::option::Option::None => {
            "borderRight"
        },
    };

    fn iterate_predecessors<'a, F>(block_g: &'a Graph<(), f64, ()>, mut set_xs: F)
    where
        F: FnMut(&'a str),
    {
        let mut stack: Vec<&'a str> = block_g.nodes().collect();
        let mut entered: HashSet<&'a str> = HashSet::default();
        let mut scratch: Vec<&'a str> = Vec::new();

        while let Some(elem) = stack.pop() {
            match {
                ::core::primitive::bool::then_some(entered.contains(elem), ())
            } {
                ::core::option::Option::Some(()) => {
                    set_xs(elem);
                    continue;
                },
                ::core::option::Option::None => {},
            }

            entered.insert(elem);
            stack.push(elem);

            scratch.clear();
            block_g.extend_predecessors(elem, &mut scratch);
            stack.extend(scratch.iter().copied());
        }
    }

    fn iterate_successors<'a, F>(block_g: &'a Graph<(), f64, ()>, mut set_xs: F)
    where
        F: FnMut(&'a str),
    {
        let mut stack: Vec<&'a str> = block_g.nodes().collect();
        let mut entered: HashSet<&'a str> = HashSet::default();
        let mut scratch: Vec<&'a str> = Vec::new();

        while let Some(elem) = stack.pop() {
            match {
                ::core::primitive::bool::then_some(entered.contains(elem), ())
            } {
                ::core::option::Option::Some(()) => {
                    set_xs(elem);
                    continue;
                },
                ::core::option::Option::None => {},
            }

            entered.insert(elem);
            stack.push(elem);

            scratch.clear();
            block_g.extend_successors(elem, &mut scratch);
            stack.extend(scratch.iter().copied());
        }
    }

    // First pass: assign smallest coordinates
    {
        let mut set = |elem: &str| {
            let mut best: f64 = 0.0;
            block_g.for_each_in_edge(elem, None, |ek, w| {
                let x_v = match xs.get(&ek.v).copied() {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => 0.0,
                };
                best = best.max(x_v + *w);
            });
            xs.insert(elem.to_string(), best);
        };
        iterate_predecessors(&block_g, &mut set);
    }

    // Second pass: assign greatest coordinates
    {
        let mut set = |elem: &str| {
            let mut min: f64 = f64::INFINITY;
            block_g.for_each_out_edge(elem, None, |ek, w| {
                let x_w = match xs.get(&ek.w).copied() {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => 0.0,
                };
                min = min.min(x_w - *w);
            });

            let node = g.node(elem);
            let node = match node {
                Some(matched_node) => matched_node,
                ::core::option::Option::None => return,
            };

            match {
                ::core::primitive::bool::then_some(min.is_finite() && node.border_type.as_deref() != Some(border_type), ())
            } {
                ::core::option::Option::Some(()) => {
                    let cur = match xs.get(elem).copied() {
                        ::core::option::Option::Some(option_value) => option_value,
                        ::core::option::Option::None => 0.0,
                    };
                    xs.insert(elem.to_string(), cur.max(min));
                },
                ::core::option::Option::None => {},
            }
        };
        iterate_successors(&block_g, &mut set);
    }

    // Assign x coordinates to all nodes based on their block root.
    let mut out: HashMap<String, f64> = HashMap::default();

    for (&v, &r) in align {
        let x = match xs
            .get(root.get(v).copied().unwrap_or(r))
            .copied() {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => 0.0,
        };
        out.insert(v.to_string(), x);
    }

    out
}

#[must_use]
fn build_block_graph_ref<'a>(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layering: &[Vec<String>],
    root: &HashMap<&'a str, &'a str>,
    reverse_sep: bool,
) -> Graph<(), f64, ()> {
    let mut block_graph: Graph<(), f64, ()> = Graph::new(GraphOptions::default());

    for layer in layering {
        let mut u: Option<&str> = None;

        for v in layer {
            let v = v.as_str();
            let v_root = root.get(v).copied().unwrap_or(v);
            block_graph.ensure_node(v_root.to_string());

            match u {
                Some(u) => {
                    let u_root = root.get(u).copied().unwrap_or(u);
                    let prev_max = match block_graph
                        .edge(u_root, v_root, None)
                        .copied() {
                        ::core::option::Option::Some(option_value) => option_value,
                        ::core::option::Option::None => 0.0,
                    };
                    let sep = sep(g, v, u, reverse_sep);
                    block_graph.set_edge_with_label(
                        u_root.to_string(),
                        v_root.to_string(),
                        sep.max(prev_max),
                    );
                },
                ::core::option::Option::None => {},
            }

            u = Some(v);
        }
    }

    block_graph
}

#[must_use]
pub fn find_smallest_width_alignment(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    xss: &HashMap<String, HashMap<String, f64>>,
) -> HashMap<String, f64> {
    let mut best_width: f64 = f64::INFINITY;
    let mut best: HashMap<String, f64> = HashMap::default();

    // Match upstream dagre: ties are resolved by a stable iteration order over alignments.
    // The canonical order is: `ul`, `ur`, `dl`, `dr` (insertion order in upstream).
    for key in ["ul", "ur", "dl", "dr"] {
        let xs = {
            let optional_xs = xss.get(key);

            match optional_xs {
                Some(matched_xs) => matched_xs,
                ::core::option::Option::None => continue,
            }
        };
        let mut max: f64 = f64::NEG_INFINITY;
        let mut min: f64 = f64::INFINITY;

        for (v, x) in xs {
            let half_w = width(g, v) / 2.0;
            max = max.max(x + half_w);
            min = min.min(x - half_w);
        }

        let w = max - min;

        match {
            ::core::primitive::bool::then_some(w < best_width, ())
        } {
            ::core::option::Option::Some(()) => {
                best_width = w;
                best = xs.clone();
            },
            ::core::option::Option::None => {},
        }
    }

    best
}

pub fn align_coordinates(
    xss: &mut HashMap<String, HashMap<String, f64>>,
    align_to: &HashMap<String, f64>,
) {
    let align_to_min = align_to.values().copied().fold(f64::INFINITY, f64::min);
    let align_to_max = align_to.values().copied().fold(f64::NEG_INFINITY, f64::max);

    for (vert, horiz) in [("u", "l"), ("u", "r"), ("d", "l"), ("d", "r")] {
        let key = format!("{vert}{horiz}");
        let xs = {
            let optional_xs = xss.get(&key).cloned();

            match optional_xs {
                Some(matched_xs) => matched_xs,
                ::core::option::Option::None => continue,
            }
        };

        let xs_min = xs.values().copied().fold(f64::INFINITY, f64::min);
        let xs_max = xs.values().copied().fold(f64::NEG_INFINITY, f64::max);

        let mut delta = align_to_min - xs_min;

        match {
            ::core::primitive::bool::then_some(horiz != "l", ())
        } {
            ::core::option::Option::Some(()) => {
                delta = align_to_max - xs_max;
            },
            ::core::option::Option::None => {},
        }

        match {
            ::core::primitive::bool::then_some(delta != 0.0, ())
        } {
            ::core::option::Option::Some(()) => {
                xss.insert(key, xs.into_iter().map(|(v, x)| (v, x + delta)).collect());
            },
            ::core::option::Option::None => {},
        }
    }
}

#[must_use]
pub fn balance(
    xss: &HashMap<String, HashMap<String, f64>>,
    align: Option<&str>,
) -> HashMap<String, f64> {
    let xs_ul = {
        let optional_xs_ul = xss.get("ul");

        match optional_xs_ul {
            Some(matched_xs_ul) => matched_xs_ul,
            ::core::option::Option::None => return HashMap::default(),
        }
    };

    let align_key = align.map(|a| a.to_ascii_lowercase());

    let mut out: HashMap<String, f64> = HashMap::default();

    for v in xs_ul.keys() {
        match align_key.as_deref() {
            Some(key) => {
                let x = match xss
                    .get(key)
                    .and_then(|xs| xs.get(v))
                    .copied() {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => 0.0,
                };
                out.insert(v.clone(), x);
                continue;
            },
            ::core::option::Option::None => {},
        }

        let mut vals: Vec<f64> = xss.values().filter_map(|xs| xs.get(v).copied()).collect();
        vals.sort_by(|a, b| a.total_cmp(b));

        match {
            ::core::primitive::bool::then_some(vals.len() >= 4, ())
        } {
            ::core::option::Option::Some(()) => {
                out.insert(v.clone(), (vals[1] + vals[2]) / 2.0);
            },
            ::core::option::Option::None => {},
        }
    }

    out
}

#[must_use]
pub fn position_x(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>) -> HashMap<String, f64> {
    let layering = crate::util::build_layer_matrix(g);
    position_x_with_layering(g, &layering)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingWorkControl {
        charges: Vec<usize>,
        reject_units: Option<usize>,
    }

    impl WorkControl for RecordingWorkControl {
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.charges.push(units);
            if self.reject_units == Some(units) {
                return Err(WorkError::Interrupted);
            }
            Ok(())
        }
    }

    fn chain_graph(
        node_count: usize,
    ) -> (Graph<NodeLabel, EdgeLabel, GraphLabel>, Vec<Vec<String>>) {
        let mut g = Graph::new(GraphOptions {
            directed: true,
            multigraph: true,
            compound: false,
        });
        g.set_graph(GraphLabel::default());
        let mut layering = Vec::with_capacity(node_count);
        for index in 0..node_count {
            let id = format!("chain-{index}");
            g.set_node(
                id.clone(),
                NodeLabel {
                    rank: Some(index as i32),
                    order: Some(0),
                    ..Default::default()
                },
            );
            if index > 0 {
                g.set_edge(format!("chain-{}", index - 1), id.clone());
            }
            layering.push(vec![id]);
        }
        (g, layering)
    }

    fn fanout_graph(
        degrees: &[usize],
    ) -> (Graph<NodeLabel, EdgeLabel, GraphLabel>, Vec<Vec<String>>) {
        let mut g = Graph::new(GraphOptions {
            directed: true,
            multigraph: true,
            compound: false,
        });
        g.set_graph(GraphLabel::default());

        let mut north = Vec::with_capacity(degrees.len());
        let mut south = Vec::new();
        for (source_index, &degree) in degrees.iter().enumerate() {
            let source = format!("source-{source_index}");
            g.set_node(
                source.clone(),
                NodeLabel {
                    rank: Some(0),
                    order: Some(source_index),
                    ..Default::default()
                },
            );
            north.push(source.clone());

            for target_index in 0..degree {
                let target = format!("target-{source_index}-{target_index}");
                let order = south.len();
                g.set_node(
                    target.clone(),
                    NodeLabel {
                        rank: Some(1),
                        order: Some(order),
                        ..Default::default()
                    },
                );
                g.set_edge(source.clone(), target.clone());
                south.push(target);
            }
        }

        (g, vec![north, south])
    }

    fn mixed_rank_neighbor_graph() -> (Graph<NodeLabel, EdgeLabel, GraphLabel>, Vec<Vec<String>>) {
        let mut g = Graph::new(GraphOptions::default());
        g.set_graph(GraphLabel::default());
        let layering = [
            ["north-left", "north-middle", "north-right"],
            ["middle-left", "middle-center", "middle-right"],
            ["south-left", "south-center", "south-right"],
        ]
        .into_iter()
        .enumerate()
        .map(|(rank, layer)| {
            layer
                .into_iter()
                .enumerate()
                .map(|(order, id)| {
                    g.set_node(
                        id,
                        NodeLabel {
                            rank: Some(rank as i32),
                            order: Some(order),
                            ..Default::default()
                        },
                    );
                    id.to_string()
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

        for (from, to) in [
            ("north-left", "middle-left"),
            ("north-right", "middle-left"),
            ("north-middle", "middle-center"),
            ("north-right", "middle-right"),
            ("middle-left", "south-left"),
            ("middle-left", "south-right"),
            ("middle-center", "south-center"),
            ("middle-right", "south-left"),
            // This edge spans two ranks, so Dagre's orientation-local stable sort remains
            // observable and the workspace must not select the preordered strategy.
            ("north-middle", "south-right"),
        ] {
            g.set_edge(from, to);
        }

        (g, layering)
    }

    fn oriented_string_layering(
        layering: &[Vec<String>],
        orientation: Orientation,
    ) -> Vec<Vec<String>> {
        let mut oriented = layering.to_vec();
        if orientation.reverse_layers {
            oriented.reverse();
        }
        if orientation.reverse_inner {
            for layer in &mut oriented {
                layer.reverse();
            }
        }
        oriented
    }

    fn workspace_alignment(
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        workspace: &BkWorkspace<'_>,
    ) -> Alignment {
        let mut root = HashMap::default();
        let mut align = HashMap::default();
        for node in 0..workspace.nodes.len() {
            let node_id = g
                .node_id_by_ix(workspace.nodes[node].graph_ix)
                .expect("workspace node id");
            let root_id = g
                .node_id_by_ix(workspace.nodes[workspace.root[node]].graph_ix)
                .expect("workspace root id");
            let align_id = g
                .node_id_by_ix(workspace.nodes[workspace.align[node]].graph_ix)
                .expect("workspace alignment id");
            root.insert(node_id.to_string(), root_id.to_string());
            align.insert(node_id.to_string(), align_id.to_string());
        }
        Alignment { root, align }
    }

    fn assert_workspace_alignment_matches_public(
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        layering: &[Vec<String>],
        expect_preordered: bool,
    ) {
        let mut workspace = BkWorkspace::new(g, layering);
        assert_eq!(workspace.neighbor_order.is_preordered(), expect_preordered);

        for orientation in ORIENTATIONS {
            let oriented_layering = oriented_string_layering(layering, orientation);
            let expected = vertical_alignment(g, &oriented_layering, &Conflicts::new(), |node| {
                let neighbors = if orientation.reverse_layers {
                    g.successors(node)
                } else {
                    g.predecessors(node)
                };
                neighbors.into_iter().map(str::to_string).collect()
            });

            workspace.vertical_alignment(orientation);

            assert_eq!(
                workspace_alignment(g, &workspace),
                expected,
                "orientation index {}",
                orientation.index
            );
        }
    }

    #[test]
    fn neighbor_sort_work_is_zero_for_a_chain() {
        let (g, layering) = chain_graph(256);
        let workspace = BkWorkspace::new(&g, &layering);

        assert_eq!(workspace.predecessor_sort_work, 0);
        assert_eq!(workspace.successor_sort_work, 0);
        assert_eq!(workspace.total_neighbor_sort_work_units(), Ok(0));
    }

    #[test]
    fn neighbor_sort_work_tracks_star_degree_k_log_k() {
        for degree in [2, 4, 8, 16, 32, 64] {
            let (g, layering) = fanout_graph(&[degree]);
            let workspace = BkWorkspace::new(&g, &layering);
            let one_direction = checked_mul(degree, ceil_log2(degree)).unwrap();

            assert_eq!(workspace.predecessor_sort_work, 0);
            assert_eq!(workspace.successor_sort_work, one_direction);
            assert_eq!(
                workspace.total_neighbor_sort_work_units(),
                Ok(one_direction)
            );
        }
    }

    #[test]
    fn neighbor_sort_work_sums_local_degrees_instead_of_global_edges() {
        let cases = [(vec![16], 64), (vec![4, 4, 4, 4], 32), (vec![1; 16], 0)];

        for (degrees, expected) in cases {
            let (g, layering) = fanout_graph(&degrees);
            assert_eq!(g.edge_count(), 16);
            let workspace = BkWorkspace::new(&g, &layering);
            assert_eq!(workspace.total_neighbor_sort_work_units(), Ok(expected));
        }
    }

    #[test]
    fn neighbor_sort_work_counts_each_layering_occurrence() {
        let degree = 8;
        let (g, mut layering) = fanout_graph(&[degree]);
        layering[0].push("source-0".to_string());
        let workspace = BkWorkspace::new(&g, &layering);
        let one_sort = checked_mul(degree, ceil_log2(degree)).unwrap();

        assert!(!workspace.neighbor_order.is_preordered());
        assert_eq!(workspace.successor_sort_work, one_sort * 2);
        assert_eq!(
            workspace.total_neighbor_sort_work_units(),
            checked_mul(one_sort, 4)
        );
    }

    #[test]
    fn mixed_layer_undirected_neighbors_keep_the_stable_sort_fallback() {
        let mut graph = Graph::new(GraphOptions {
            directed: false,
            multigraph: false,
            compound: false,
        });
        graph.set_graph(GraphLabel::default());
        for (id, rank) in [("north", 0), ("middle", 1), ("south", 2)] {
            graph.set_node(
                id,
                NodeLabel {
                    rank: Some(rank),
                    order: Some(0),
                    ..Default::default()
                },
            );
        }
        graph.set_edge("north", "middle");
        graph.set_edge("middle", "south");
        let layering = vec![
            vec!["north".to_string()],
            vec!["middle".to_string()],
            vec!["south".to_string()],
        ];

        let workspace = BkWorkspace::new(&graph, &layering);

        assert!(!workspace.neighbor_order.is_preordered());
        assert_eq!(workspace.total_neighbor_sort_work_units(), Ok(8));
    }

    #[test]
    fn preordered_neighbors_match_public_stable_sort_in_all_orientations() {
        let (g, layering) = fanout_graph(&[3, 5, 7]);

        assert_workspace_alignment_matches_public(&g, &layering, true);
    }

    #[test]
    fn named_parallel_edges_contribute_one_bk_neighbor() {
        let mut graph = Graph::new(GraphOptions {
            directed: true,
            multigraph: true,
            compound: false,
        });
        graph.set_graph(GraphLabel::default());
        for (id, rank, order) in [("a", 0, 0), ("b", 0, 1), ("target", 1, 0)] {
            graph.set_node(
                id,
                NodeLabel {
                    rank: Some(rank),
                    order: Some(order),
                    ..Default::default()
                },
            );
        }
        graph.set_edge_named("a", "target", Some("first"), Some(EdgeLabel::default()));
        graph.set_edge_named("a", "target", Some("second"), Some(EdgeLabel::default()));
        graph.set_edge_named("b", "target", Some("third"), Some(EdgeLabel::default()));
        let layering = vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["target".to_string()],
        ];

        let workspace = BkWorkspace::new(&graph, &layering);
        let target = workspace
            .nodes
            .iter()
            .position(|node| graph.node_id_by_ix(node.graph_ix) == Some("target"))
            .expect("target workspace node");
        let predecessor_ids = workspace.predecessors[target]
            .iter()
            .map(|&node| {
                graph
                    .node_id_by_ix(workspace.nodes[node].graph_ix)
                    .expect("predecessor workspace node")
            })
            .collect::<Vec<_>>();

        assert_eq!(predecessor_ids, ["a", "b"]);
        assert_eq!(workspace.predecessor_entries, 2);
        assert_eq!(workspace.successor_entries, 2);
        assert_workspace_alignment_matches_public(&graph, &layering, true);
    }

    #[test]
    fn stable_sort_fallback_matches_public_alignment_in_all_orientations() {
        let (g, layering) = mixed_rank_neighbor_graph();

        assert_workspace_alignment_matches_public(&g, &layering, false);
    }

    #[test]
    fn workspace_index_span_ignores_unlayered_high_slot_nodes() {
        let mut g = Graph::new(GraphOptions::default());
        g.set_graph(GraphLabel::default());
        g.set_node(
            "layered",
            NodeLabel {
                rank: Some(0),
                order: Some(0),
                ..Default::default()
            },
        );
        for index in 0..4_096 {
            g.set_node(format!("unlayered-{index}"), NodeLabel::default());
        }
        let layering = vec![vec!["layered".to_string()]];
        let mut work_control = RecordingWorkControl::default();

        let workspace = BkWorkspace::new_controlled(&g, &layering, &mut work_control).unwrap();

        assert!(g.node_slot_count() > 4_000);
        assert_eq!(workspace.nodes.len(), 1);
        // The third setup tranche covers the graph-index lookup allocation plus the source
        // entries. It must follow the participating index span (1 + 1), not all graph slots.
        assert_eq!(work_control.charges, vec![1, 3, 2, 1]);
    }

    #[test]
    fn indexed_workspace_bounds_a_participating_high_tombstone_slot() {
        let mut g = Graph::new(GraphOptions::default());
        g.set_graph(GraphLabel::default());
        for index in 0..4_096 {
            g.set_node(format!("retired-{index}"), NodeLabel::default());
        }
        g.set_node(
            "layered",
            NodeLabel {
                rank: Some(0),
                order: Some(0),
                ..Default::default()
            },
        );
        let layered_ix = g.node_ix("layered").expect("layered node index");
        for index in 0..4_096 {
            assert!(g.remove_node(&format!("retired-{index}")));
        }
        let layering = vec![vec![layered_ix]];
        let mut work_control = RecordingWorkControl::default();

        let workspace =
            BkWorkspace::new_indexed_controlled(&g, &layering, &mut work_control).unwrap();

        assert!(layered_ix > 4_000);
        assert_eq!(g.node_count(), 1);
        assert_eq!(workspace.nodes.len(), 1);
        // Both indexed setup tranches stay proportional to the participating entry count. The
        // retained Graphlib slot index only selects the sparse lookup representation.
        assert_eq!(work_control.charges, vec![1, 2, 2, 1]);
    }

    #[test]
    fn controlled_bk_rejects_setup_before_workspace_allocation() {
        let (g, layering) = fanout_graph(&[8]);
        let source_entries = layering.iter().map(Vec::len).sum::<usize>();
        let setup_work = layering.len() + 2 * source_entries + g.edge_slot_count();
        let mut work_control = RecordingWorkControl {
            reject_units: Some(setup_work),
            ..Default::default()
        };

        assert_eq!(
            position_x_with_layering_controlled(&g, &layering, &mut work_control),
            Err(WorkError::Interrupted)
        );
        assert_eq!(work_control.charges, vec![layering.len(), setup_work]);
    }

    #[test]
    fn controlled_bk_rejects_the_successor_sort_tranche_before_positioning() {
        let degree = 31;
        let (g, layering) = fanout_graph(&[degree]);
        let successor_sort_work = local_sort_work(degree).unwrap();
        let mut work_control = RecordingWorkControl {
            reject_units: Some(successor_sort_work),
            ..Default::default()
        };

        assert_eq!(
            position_x_with_layering_controlled(&g, &layering, &mut work_control),
            Err(WorkError::Interrupted)
        );
        assert_eq!(work_control.charges.last(), Some(&successor_sort_work));
        assert_eq!(
            work_control
                .charges
                .iter()
                .filter(|&&units| units == successor_sort_work)
                .count(),
            1
        );
    }

    #[test]
    fn controlled_bk_matches_the_compatibility_entry_point() {
        let (g, layering) = fanout_graph(&[3, 5, 7]);
        let expected = position_x_with_layering(&g, &layering);
        let mut work_control = RecordingWorkControl::default();

        let actual = position_x_with_layering_controlled(&g, &layering, &mut work_control).unwrap();

        assert_eq!(actual, expected);
        assert!(!work_control.charges.is_empty());
    }

    #[test]
    fn indexed_bk_matches_the_public_string_adapter_exactly() {
        let (g, layering) = fanout_graph(&[3, 5, 7]);
        let indexed_layering = layering
            .iter()
            .map(|layer| {
                layer
                    .iter()
                    .map(|id| g.node_ix(id).expect("fixture node index"))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut string_work = RecordingWorkControl::default();
        let mut indexed_work = RecordingWorkControl::default();

        let string_positions =
            position_x_with_layering_controlled(&g, &layering, &mut string_work).unwrap();
        let indexed_positions =
            position_x_with_indexed_layering_controlled(&g, &indexed_layering, &mut indexed_work)
                .unwrap()
                .into_iter()
                .map(|(graph_ix, x)| {
                    (
                        g.node_id_by_ix(graph_ix)
                            .expect("positioned fixture node")
                            .to_owned(),
                        x,
                    )
                })
                .collect::<HashMap<_, _>>();

        assert_eq!(indexed_positions, string_positions);
        assert!(
            indexed_work.charges.iter().sum::<usize>() < string_work.charges.iter().sum::<usize>()
        );
    }

    fn fallback_graph(
        intermediate_count: usize,
    ) -> (Graph<NodeLabel, EdgeLabel, GraphLabel>, Vec<Vec<String>>) {
        let mut g = Graph::new(GraphOptions::default());
        g.set_graph(GraphLabel::default());

        let north = vec![
            "north-low".to_string(),
            "north-middle".to_string(),
            "north-high".to_string(),
        ];
        for (order, id) in north.iter().enumerate() {
            g.set_node(
                id.clone(),
                NodeLabel {
                    rank: Some(0),
                    order: Some(order),
                    dummy: Some("dummy".to_string()),
                    ..Default::default()
                },
            );
        }

        let mut south = Vec::with_capacity(intermediate_count + 2);
        south.push("border-high".to_string());
        south.extend((0..intermediate_count).map(|index| format!("dummy-{index}")));
        south.push("border-low".to_string());

        for (order, id) in south.iter().enumerate() {
            let dummy = if id.starts_with("border-") {
                "border"
            } else {
                "dummy"
            };
            g.set_node(
                id.clone(),
                NodeLabel {
                    rank: Some(1),
                    order: Some(order),
                    dummy: Some(dummy.to_string()),
                    ..Default::default()
                },
            );
        }

        g.set_edge("north-high", "border-high");
        for id in south.iter().skip(1) {
            g.set_edge("north-low", id);
        }

        (g, vec![north, south])
    }

    fn reference_type2_bounds(
        south_len: usize,
        north_len: isize,
        boundaries: &[Type2Boundary],
    ) -> Vec<Type2Bounds> {
        let mut bounds = vec![
            Type2Bounds {
                lower: isize::MIN,
                upper: isize::MAX,
            };
            south_len
        ];
        let mut previous_north = -1;
        let mut next_north: Option<isize> = None;
        let mut south_start = 0;
        let mut boundary_index = 0;

        for south_index in 0..south_len {
            if boundaries
                .get(boundary_index)
                .is_some_and(|boundary| boundary.south_index == south_index)
            {
                next_north = boundaries[boundary_index].north_order;
                for current in &mut bounds[south_start..south_index] {
                    current.lower = current.lower.max(previous_north);
                    current.upper = current.upper.min(next_north.unwrap_or(-1));
                }
                south_start = south_index;
                previous_north = next_north.unwrap_or(previous_north);
                boundary_index += 1;
            }

            for current in &mut bounds[south_start..south_len] {
                current.lower = current.lower.max(next_north.unwrap_or(-1));
                current.upper = current.upper.min(north_len);
            }
        }

        bounds
    }

    fn public_conflict_ids(conflicts: &Conflicts) -> BTreeSet<(String, String)> {
        conflicts
            .iter()
            .flat_map(|(left, rights)| {
                rights
                    .iter()
                    .map(move |right| (left.clone(), right.clone()))
            })
            .collect()
    }

    fn workspace_conflict_ids(
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        workspace: &BkWorkspace<'_>,
    ) -> BTreeSet<(String, String)> {
        workspace
            .conflicts
            .iter()
            .map(|&(left, right)| {
                let left = g
                    .node_id_by_ix(workspace.nodes[left].graph_ix)
                    .expect("left conflict id");
                let right = g
                    .node_id_by_ix(workspace.nodes[right].graph_ix)
                    .expect("right conflict id");
                if left <= right {
                    (left.to_string(), right.to_string())
                } else {
                    (right.to_string(), left.to_string())
                }
            })
            .collect()
    }

    #[test]
    fn linear_type2_bounds_match_upstream_suffix_scan_union() {
        const STATES: [Option<Option<isize>>; 5] = [
            None,
            Some(None),
            Some(Some(0)),
            Some(Some(2)),
            Some(Some(5)),
        ];

        for south_len in 0..=5 {
            let combinations = STATES.len().pow(south_len as u32);
            for mut encoded in 0..combinations {
                let mut boundaries = Vec::new();
                for south_index in 0..south_len {
                    let state = STATES[encoded % STATES.len()];
                    encoded /= STATES.len();
                    if let Some(north_order) = state {
                        boundaries.push(Type2Boundary {
                            south_index,
                            north_order,
                        });
                    }
                }
                assert_eq!(
                    type2_conflict_bounds(south_len, 3, &boundaries),
                    reference_type2_bounds(south_len, 3, &boundaries),
                    "south_len={south_len}, boundaries={boundaries:?}"
                );
            }
        }
    }

    #[test]
    fn indexed_type2_scan_matches_public_conflicts() {
        for missing_border_order in [false, true] {
            for intermediate_count in [0, 1, 8, 64] {
                let (mut g, layering) = fallback_graph(intermediate_count);
                if missing_border_order {
                    g.node_mut("north-high").expect("north-high").order = None;
                }
                let expected = find_type2_conflicts(&g, &layering);

                let mut workspace = BkWorkspace::new(&g, &layering);
                workspace.find_type2_conflicts();

                let expected = public_conflict_ids(&expected);
                assert_eq!(workspace_conflict_ids(&g, &workspace), expected);
                assert_eq!(workspace.conflicts.len(), expected.len());
            }
        }
    }

    #[test]
    fn indexed_type2_scan_matches_public_for_out_of_range_monotonic_border_orders() {
        let mut g = Graph::new(GraphOptions::default());
        g.set_graph(GraphLabel::default());
        for (id, rank, order, dummy) in [
            ("low", 0, 0, "dummy"),
            ("middle", 0, 1, "dummy"),
            ("high", 0, 5, "dummy"),
            ("border-low", 1, 0, "border"),
            ("south", 1, 1, "dummy"),
            ("border-high", 1, 2, "border"),
        ] {
            g.set_node(
                id,
                NodeLabel {
                    rank: Some(rank),
                    order: Some(order),
                    dummy: Some(dummy.to_string()),
                    ..Default::default()
                },
            );
        }
        g.set_edge("low", "border-low");
        g.set_edge("high", "south");
        g.set_edge("high", "border-high");

        let layering = vec![
            vec!["low".to_string(), "middle".to_string(), "high".to_string()],
            vec![
                "border-low".to_string(),
                "south".to_string(),
                "border-high".to_string(),
            ],
        ];
        let expected = find_type2_conflicts(&g, &layering);
        assert!(has_conflict(&expected, "high", "south"));

        let mut workspace = BkWorkspace::new(&g, &layering);
        workspace.find_type2_conflicts();

        let expected = public_conflict_ids(&expected);
        assert_eq!(workspace_conflict_ids(&g, &workspace), expected);
        assert_eq!(workspace.conflicts.len(), expected.len());
    }
}
