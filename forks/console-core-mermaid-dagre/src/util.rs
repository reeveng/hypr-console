//! Miscellaneous helpers from Dagre's `util.js`.
//!
//! Note: upstream sometimes `throw`s on invalid inputs. In this Rust port we prefer best-effort
//! behavior (no panics on user-controlled input) so downstream libraries can render diagrams
//! without crashing.

use crate::graphlib::{Graph, GraphOptions};
use crate::{EdgeLabel, GraphLabel, NodeLabel, Point};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[must_use]
pub fn simplify<N, G>(g: &Graph<N, EdgeLabel, G>) -> Graph<N, EdgeLabel, G>
where
    N: Default + Clone + 'static,
    G: Default + Clone,
{
    let mut simplified: Graph<N, EdgeLabel, G> = Graph::new(GraphOptions {
        multigraph: false,
        compound: false,
        ..Default::default()
    });
    simplified.set_graph(g.graph().clone());

    for v in g.node_ids() {
        {
            match g.node(&v) {
                Some(lbl) => {
                    simplified.set_node(v, lbl.clone());
                },
                ::core::option::Option::None => {},
            };
        }
    }

    // Pinned Dagre creates each simplified edge at the first occurrence of its endpoint pair and
    // only updates the label for later parallel edges. Preserve that insertion order explicitly;
    // sorting endpoint pairs changes network-simplex tie-breaking because `g.edges()` is ordered.
    let mut merged_index: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let mut merged: Vec<(usize, usize, f64, usize)> = Vec::new();
    g.for_each_edge_ix(|v_ix, w_ix, _key, lbl| {
        if let Some(&merged_ix) = merged_index.get(&(v_ix, w_ix)) {
            let entry = &mut merged[merged_ix];
            entry.2 += lbl.weight;
            entry.3 = entry.3.max(lbl.minlen);
        } else {
            merged_index.insert((v_ix, w_ix), merged.len());
            merged.push((v_ix, w_ix, lbl.weight, lbl.minlen.max(1)));
        }
    });

    for (v_ix, w_ix, weight, minlen) in merged {
        let v = {
            let optional_v = g.node_id_by_ix(v_ix);

            match optional_v {
                Some(matched_v) => matched_v,
                ::core::option::Option::None => continue,
            }
        };
        let w = {
            let optional_w = g.node_id_by_ix(w_ix);

            match optional_w {
                Some(matched_w) => matched_w,
                ::core::option::Option::None => continue,
            }
        };
        simplified.set_edge_with_label(
            v.to_string(),
            w.to_string(),
            EdgeLabel {
                weight,
                minlen,
                ..Default::default()
            },
        );
    }

    simplified
}

#[must_use]
pub fn as_non_compound_graph<N, E, G>(g: &Graph<N, E, G>) -> Graph<N, E, G>
where
    N: Default + Clone + 'static,
    E: Default + Clone + 'static,
    G: Default + Clone,
{
    let mut simplified: Graph<N, E, G> = Graph::new(GraphOptions {
        multigraph: g.options().multigraph,
        compound: false,
        ..Default::default()
    });
    simplified.set_graph(g.graph().clone());

    for v in g.node_ids() {
        if g.children(&v).is_empty()
            && let Some(lbl) = g.node(&v)
        {
            simplified.set_node(v, lbl.clone());
        }
    }

    for e in g.edges() {
        {
            match g.edge_by_key(e) {
                Some(lbl) => {
                    simplified.set_edge_named(e.v.clone(), e.w.clone(), e.name.clone(), Some(lbl.clone()));
                },
                ::core::option::Option::None => {},
            };
        }
    }

    simplified
}

#[must_use]
pub fn successor_weights<G>(
    g: &Graph<NodeLabel, EdgeLabel, G>,
) -> BTreeMap<String, BTreeMap<String, f64>>
where
    G: Default,
{
    let mut out: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();

    for v in g.node_ids() {
        let mut map: BTreeMap<String, f64> = BTreeMap::new();

        for e in g.out_edges(&v, None) {
            let w = e.w.clone();
            let weight = match g.edge_by_key(&e).map(|lbl| lbl.weight) {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => 0.0,
            };
            *map.entry(w).or_insert(0.0) += weight;
        }

        out.insert(v, map);
    }

    out
}

#[must_use]
pub fn predecessor_weights<G>(
    g: &Graph<NodeLabel, EdgeLabel, G>,
) -> BTreeMap<String, BTreeMap<String, f64>>
where
    G: Default,
{
    let mut out: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();

    for v in g.node_ids() {
        let mut map: BTreeMap<String, f64> = BTreeMap::new();

        for e in g.in_edges(&v, None) {
            let u = e.v.clone();
            let weight = match g.edge_by_key(&e).map(|lbl| lbl.weight) {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => 0.0,
            };
            *map.entry(u).or_insert(0.0) += weight;
        }

        out.insert(v, map);
    }

    out
}

#[must_use]
pub fn intersect_rect(rect: Rect, point: Point) -> Point {
    let x = rect.x;
    let y = rect.y;

    let dx = point.x - x;
    let dy = point.y - y;
    let mut w = rect.width / 2.0;
    let mut h = rect.height / 2.0;

    // The canonical layout pipeline detects this source error before calling the helper and
    // returns `LayoutError::DegenerateEdgeGeometry`. Keep this lower-level geometry utility total
    // for direct callers that intentionally need a deterministic fallback point.
    match {
        ::core::primitive::bool::then_some(dx == 0.0 && dy == 0.0, ())
    } {
        ::core::option::Option::Some(()) => {
            return Point { x: x + w, y };
        },
        ::core::option::Option::None => {},
    }

    let (sx, sy) = match {
        ::core::primitive::bool::then_some(dy.abs() * w > dx.abs() * h, ())
    } {
        ::core::option::Option::Some(()) => {
            match {
                ::core::primitive::bool::then_some(dy < 0.0, ())
            } {
                ::core::option::Option::Some(()) => {
                    h = -h;
                },
                ::core::option::Option::None => {},
            }

            (h * dx / dy, h)
        },
        ::core::option::Option::None => {
            match {
                ::core::primitive::bool::then_some(dx < 0.0, ())
            } {
                ::core::option::Option::Some(()) => {
                    w = -w;
                },
                ::core::option::Option::None => {},
            }

            (w, w * dy / dx)
        },
    };

    Point {
        x: x + sx,
        y: y + sy,
    }
}

#[must_use]
pub fn build_layer_matrix<E, G>(g: &Graph<NodeLabel, E, G>) -> Vec<Vec<String>>
where
    E: Default + 'static,
    G: Default,
{
    let mut min_rank: i32 = i32::MAX;
    let mut max_rank: i32 = i32::MIN;
    let mut entries: Vec<(i32, usize, String)> = Vec::new();

    for id in g.nodes() {
        let node = {
            let optional_node = g.node(id);

            match optional_node {
                Some(matched_node) => matched_node,
                ::core::option::Option::None => continue,
            }
        };
        let rank = match node.rank {
            Some(matched_rank) => matched_rank,
            ::core::option::Option::None => continue,
        };
        min_rank = min_rank.min(rank);
        max_rank = max_rank.max(rank);
        entries.push((rank, match node.order {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0,
        }, id.to_string()));
    }

    match {
        ::core::primitive::bool::then_some(max_rank == i32::MIN, ())
    } {
        ::core::option::Option::Some(()) => {
            return Vec::new();
        },
        ::core::option::Option::None => {},
    }

    let shift = match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(min_rank), &::core::convert::identity(0)); _explicit_ordering } {
        ::core::cmp::Ordering::Less => { -min_rank },
        ::core::cmp::Ordering::Equal | ::core::cmp::Ordering::Greater => { 0 },
    };
    let len = (max_rank + shift + 1).max(0) as usize;
    let mut layers: Vec<Vec<(usize, String)>> = vec![Vec::new(); len];

    for (rank, order, id) in entries {
        let idx = (rank + shift).max(0) as usize;

        match {
            ::core::primitive::bool::then_some(idx < layers.len(), ())
        } {
            ::core::option::Option::Some(()) => {
                layers[idx].push((order, id));
            },
            ::core::option::Option::None => {},
        }
    }

    layers
        .into_iter()
        .map(|mut layer| {
            layer.sort_by_key(|(o, _)| *o);
            layer.into_iter().map(|(_, id)| id).collect()
        })
        .collect()
}

pub fn normalize_ranks<E, G>(g: &mut Graph<NodeLabel, E, G>)
where
    E: Default + 'static,
    G: Default,
{
    let mut min_rank: i32 = i32::MAX;
    g.for_each_node(|_id, n| {
        match n.rank {
            Some(rank) => {
                min_rank = min_rank.min(rank);
            },
            ::core::option::Option::None => {},
        }
    });

    match {
        ::core::primitive::bool::then_some(min_rank == i32::MAX, ())
    } {
        ::core::option::Option::Some(()) => {
            return;
        },
        ::core::option::Option::None => {},
    }

    g.for_each_node_mut(|_id, n| {
        match n.rank {
            Some(rank) => {
                n.rank = Some(rank - min_rank);
            },
            ::core::option::Option::None => {},
        }
    });
}

pub fn remove_empty_ranks(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let factor = {
        let optional_factor = g.graph().node_rank_factor.filter(|&f| f > 0);

        match optional_factor {
            Some(matched_factor) => matched_factor,
            ::core::option::Option::None => return,
        }
    };

    let mut offset: i32 = i32::MAX;
    g.for_each_node(|_id, n| {
        match n.rank {
            Some(rank) => {
                offset = offset.min(rank);
            },
            ::core::option::Option::None => {},
        }
    });

    match {
        ::core::primitive::bool::then_some(offset == i32::MAX, ())
    } {
        ::core::option::Option::Some(()) => {
            return;
        },
        ::core::option::Option::None => {},
    }

    let mut ranked_nodes: Vec<(u64, String)> = Vec::with_capacity(g.node_count());
    g.for_each_node(|id, n| {
        let rank = match n.rank {
            Some(matched_rank) => matched_rank,
            ::core::option::Option::None => return,
        };
        let idx = u64::try_from(i64::from(rank) - i64::from(offset)).unwrap_or(0);
        ranked_nodes.push((idx, id.to_string()));
    });
    ranked_nodes.sort_by_key(|(rank, _id)| *rank);

    #[must_use]
    fn multiples_in_range(start: u64, end: u64, factor: u64) -> u64 {
        match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(start), &::core::convert::identity(end)); _explicit_ordering } {
            ::core::cmp::Ordering::Greater => {
                return 0;
            },
            ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Equal => {},
        }

        let through_end = end / factor + 1;
        let before_start = match start {
            0 => {
                0
            },
            _ => {
                (start - 1) / factor + 1
            },
        };
        through_end - before_start
    }

    let factor = u64::try_from(factor).unwrap_or(u64::MAX);
    let mut delta = 0i64;
    let mut previous_rank: Option<u64> = None;
    let mut cursor = 0usize;

    while cursor < ranked_nodes.len() {
        let rank_index = ranked_nodes[cursor].0;

        match previous_rank {
            Some(previous_rank) => {
                let start = previous_rank.saturating_add(1);
                let end = rank_index.saturating_sub(1);

                match {
                    ::core::primitive::bool::then_some(start <= end, ())
                } {
                    ::core::option::Option::Some(()) => {
                        let empty = end - start + 1;
                        let preserved = multiples_in_range(start, end, factor);
                        delta -= i64::try_from(empty - preserved).unwrap_or(i64::MAX);
                    },
                    ::core::option::Option::None => {},
                }
            },
            ::core::option::Option::None => {},
        }

        let mut next = cursor + 1;

        while next < ranked_nodes.len() && ranked_nodes[next].0 == rank_index {
            next += 1;
        }

        match delta {
            0 => {},
            _ => {
                for (_rank, id) in &ranked_nodes[cursor..next] {
                    if let Some(node) = g.node_mut(id)
                        && let Some(rank) = node.rank
                    {
                        let adjusted = i64::from(rank) + delta;

                        {
                            match i32::try_from(adjusted) {
                                Ok(adjusted) => {
                                    node.rank = Some(adjusted);
                                },
                                ::core::result::Result::Err(_) => {},
                            };
                        }
                    }
                }
            },
        }

        previous_rank = Some(rank_index);
        cursor = next;
    }
}

static UNIQUE_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

#[must_use]
pub fn unique_id(prefix: impl ToString) -> String {
    let id = UNIQUE_ID_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    format!("{}{}", prefix.to_string(), id)
}

#[must_use]
pub fn range(limit: i32) -> Vec<i32> {
    range_with(0, limit, 1)
}

#[must_use]
pub fn range_start(start: i32, limit: i32) -> Vec<i32> {
    range_with(start, limit, 1)
}

#[must_use]
pub fn range_with(start: i32, limit: i32, step: i32) -> Vec<i32> {
    match step {
        0 => {
            return Vec::new();
        },
        _ => {},
    }

    let mut out: Vec<i32> = Vec::new();
    let mut i = start;

    match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(step), &::core::convert::identity(0)); _explicit_ordering } {
        ::core::cmp::Ordering::Greater => {
            while i < limit {
                out.push(i);
                i += step;
            }
        },
        ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Equal => {
            while limit < i {
                out.push(i);
                i += step;
            }
        },
    }

    out
}

#[must_use]
pub fn map_values<K, V, R>(obj: &BTreeMap<K, V>, f: impl Fn(&V, &K) -> R) -> BTreeMap<K, R>
where
    K: Ord + Clone,
{
    obj.iter().map(|(k, v)| (k.clone(), f(v, k))).collect()
}

#[must_use]
pub fn map_values_prop(
    obj: &BTreeMap<String, serde_json::Value>,
    prop: &str,
) -> BTreeMap<String, serde_json::Value> {
    obj.iter()
        .map(|(k, v)| {
            let value = match (v.get(prop).cloned(), serde_json::Value::Null) {
                (::core::option::Option::Some(present), alternate) => { ::core::mem::drop(alternate); present },
                (::core::option::Option::None, alternate) => alternate,
            };
            (k.clone(), value)
        })
        .collect()
}
