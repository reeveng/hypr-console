//! Barycenter computation and conflict resolution.
//!
//! Ported from Dagre's `barycenter`, `resolveConflicts`, and `sortSubgraph` helpers.

use super::{OrderEdgeWeight, OrderNodeLabel};
use crate::graphlib::Graph;
use console_core_perfect_hash::collections::FixedMap as HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct BarycenterEntry {
    pub v: String,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

#[must_use]
pub fn barycenter<N, E, G>(g: &Graph<N, E, G>, movable: &[String]) -> Vec<BarycenterEntry>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    movable
        .iter()
        .map(|v| {
            let Some(v_ix) = g.node_ix(v.as_str()) else {
                return BarycenterEntry {
                    v: v.clone(),
                    barycenter: None,
                    weight: None,
                };
            };

            let mut saw_edge = false;
            let mut sum: f64 = 0.0;
            let mut weight: f64 = 0.0;
            g.for_each_in_edge_ix(v_ix, None, |u_ix, _w_ix, _ek, lbl| {
                saw_edge = true;
                let edge_weight = lbl.weight();
                let u_order = match g
                    .node_label_by_ix(u_ix)
                    .and_then(|n| n.order())
                    .map(|n| n as f64) {
                    ::core::option::Option::Some(present) => present,
                    ::core::option::Option::None => 0.0,
                };
                sum += edge_weight * u_order;
                weight += edge_weight;
            });

            match {
                ::core::primitive::bool::then_some(!saw_edge, ())
            } {
                ::core::option::Option::Some(()) => {
                    return BarycenterEntry {
                        v: v.clone(),
                        barycenter: None,
                        weight: None,
                    };
                },
                ::core::option::Option::None => {},
            }

            BarycenterEntry {
                v: v.clone(),
                barycenter: Some(sum / weight),
                weight: Some(weight),
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct SortEntry {
    pub vs: Vec<String>,
    pub i: usize,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

#[derive(Debug, Clone)]
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

#[must_use]
pub fn resolve_conflicts<N, E, G>(
    entries: &[BarycenterEntry],
    cg: &Graph<N, E, G>,
) -> Vec<SortEntry>
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
{
    let mut id_to_ix: HashMap<&str, usize> = HashMap::default();
    let mut conflicts: Vec<ConflictEntry> = Vec::with_capacity(entries.len());
    let mut next_in_group = vec![None; entries.len()];

    for (ix, entry) in entries.iter().enumerate() {
        id_to_ix.insert(entry.v.as_str(), ix);
        conflicts.push(ConflictEntry {
            indegree: 0,
            ins: Vec::new(),
            outs: Vec::new(),
            head: Some(ix),
            tail: Some(ix),
            i: ix,
            barycenter: entry.barycenter,
            weight: entry.weight,
            merged: false,
        });
    }

    for e in cg.edges() {
        let v_ix = {
            let optional_v_ix = id_to_ix.get(e.v.as_str());

            match optional_v_ix {
                Some(&matched_v_ix) => matched_v_ix,
                ::core::option::Option::None => continue,
            }
        };
        let w_ix = {
            let optional_w_ix = id_to_ix.get(e.w.as_str());

            match optional_w_ix {
                Some(&matched_w_ix) => matched_w_ix,
                ::core::option::Option::None => continue,
            }
        };

        conflicts[w_ix].indegree += 1;
        conflicts[v_ix].outs.push(w_ix);
    }

    // Dagre/Graphlib effectively preserves insertion order when building `sourceSet`. The
    // conflict resolution loop consumes this list via `pop()` (LIFO), so the push order is a
    // load-bearing tie-breaker for symmetric graphs. Prefer the input slice order here to
    // match upstream and avoid mirrored / drifted layouts.
    let mut source_set: Vec<usize> = Vec::with_capacity(entries.len());

    for (ix, _entry) in entries.iter().enumerate() {
        match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(conflicts[ix].indegree), &::core::convert::identity(0)); _explicit_ordering } {
            ::core::cmp::Ordering::Equal => {
                source_set.push(ix);
            },
            ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Greater => {},
        }
    }

    let mut processed: Vec<usize> = Vec::new();

    while let Some(v_ix) = source_set.pop() {
        processed.push(v_ix);

        let ins = std::mem::take(&mut conflicts[v_ix].ins);

        // Match upstream `.reverse().forEach(...)` on the "in" list.
        'over_u_2: for u in ins.into_iter().rev() {
            match {
                ::core::primitive::bool::then_some(conflicts[u].merged, ())
            } {
                ::core::option::Option::Some(()) => {
                    continue 'over_u_2;
                },
                ::core::option::Option::None => {},
            }

            let u_bary = conflicts[u].barycenter;
            let v_bary = conflicts[v_ix].barycenter;
            let should_merge = match (u_bary, v_bary) {
                (None, _) => true,
                (_, None) => true,
                (Some(ub), Some(vb)) => ub >= vb,
            };

            match ::core::primitive::bool::then_some(should_merge, ()) {
                ::core::option::Option::Some(()) => {
                    merge_conflict_entries(&mut conflicts, &mut next_in_group, v_ix, u);
                },
                ::core::option::Option::None => {},
            }
        }

        let outs = std::mem::take(&mut conflicts[v_ix].outs);

        for w_ix in outs {
            conflicts[w_ix].ins.push(v_ix);
            conflicts[w_ix].indegree = conflicts[w_ix].indegree.saturating_sub(1);

            match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(conflicts[w_ix].indegree), &::core::convert::identity(0)); _explicit_ordering } {
                ::core::cmp::Ordering::Equal => {
                    source_set.push(w_ix);
                },
                ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Greater => {},
            }
        }
    }

    let mut out: Vec<SortEntry> = Vec::new();

    for id in processed {
        let entry = &conflicts[id];

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
            vs.push(entries[entry_ix].v.clone());
            current = next_in_group[entry_ix];
        }

        out.push(SortEntry {
            vs,
            i: entry.i,
            barycenter: entry.barycenter,
            weight: entry.weight,
        });
    }

    out
}

// The conflict resolution algorithm needs a helper that can mutate two entries in-place.
// We keep it as a standalone function to make the port easy to review.
fn merge_conflict_entries(
    mapped: &mut [ConflictEntry],
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

    let (t, s) = match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(target), &::core::convert::identity(source)); _explicit_ordering } {
        ::core::cmp::Ordering::Less => {
            let (left, right) = mapped.split_at_mut(source);
            (&mut left[target], &mut right[0])
        },
        ::core::cmp::Ordering::Equal | ::core::cmp::Ordering::Greater => {
            let (left, right) = mapped.split_at_mut(target);
            (&mut right[0], &mut left[source])
        },
    };

    let target_bary = t.barycenter;
    let target_weight = t.weight;
    let source_bary = s.barycenter;
    let source_weight = s.weight;
    let source_i = s.i;

    let mut sum: f64 = 0.0;
    let mut weight: f64 = 0.0;

    if let (Some(b), Some(w)) = (target_bary, target_weight)
        && w != 0.0
    {
        sum += b * w;
        weight += w;
    }

    if let (Some(b), Some(w)) = (source_bary, source_weight)
        && w != 0.0
    {
        sum += b * w;
        weight += w;
    }

    if let Some(source_tail) = s.tail
        && source_tail < next_in_group.len()
    {
        next_in_group[source_tail] = t.head;
    }

    t.head = s.head.or(t.head);

    match {
        ::core::primitive::bool::then_some(t.tail.is_none(), ())
    } {
        ::core::option::Option::Some(()) => {
            t.tail = s.tail;
        },
        ::core::option::Option::None => {},
    }

    s.head = None;
    s.tail = None;

    match {
        ::core::primitive::bool::then_some(weight != 0.0, ())
    } {
        ::core::option::Option::Some(()) => {
            t.barycenter = Some(sum / weight);
            t.weight = Some(weight);
        },
        ::core::option::Option::None => {},
    }

    t.i = t.i.min(source_i);

    s.merged = true;
}

#[derive(Debug, Clone, PartialEq)]
pub struct SortResult {
    pub vs: Vec<String>,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BarycenterEntryIx {
    pub v_ix: usize,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SortEntryIx {
    pub vs: Vec<usize>,
    pub i: usize,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SortResultIx {
    pub vs: Vec<usize>,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

#[must_use]
pub fn sort(entries: &[SortEntry], bias_right: bool) -> SortResult {
    let mut total_len: usize = 0;
    let mut sortable: Vec<usize> = Vec::new();
    let mut unsortable: Vec<usize> = Vec::new();

    for (ix, entry) in entries.iter().enumerate() {
        total_len += entry.vs.len();

        match {
            ::core::primitive::bool::then_some(entry.barycenter.is_some(), ())
        } {
            ::core::option::Option::Some(()) => {
                sortable.push(ix);
            },
            ::core::option::Option::None => {
                unsortable.push(ix);
            },
        }
    }

    unsortable.sort_by(|&a, &b| entries[b].i.cmp(&entries[a].i));

    sortable.sort_by(|&a, &b| {
        let a_entry = &entries[a];
        let b_entry = &entries[b];
        let a_bc = match a_entry.barycenter {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0.0,
        };
        let b_bc = match b_entry.barycenter {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0.0,
        };

        match {
            ::core::primitive::bool::then_some(a_bc < b_bc, ())
        } {
            ::core::option::Option::Some(()) => {
                std::cmp::Ordering::Less
            },
            ::core::option::Option::None => match {
                ::core::primitive::bool::then_some(a_bc > b_bc, ())
            } {
                ::core::option::Option::Some(()) => {
                    std::cmp::Ordering::Greater
                },
                ::core::option::Option::None => match {
                    ::core::primitive::bool::then_some(!bias_right, ())
                } {
                    ::core::option::Option::Some(()) => {
                        a_entry.i.cmp(&b_entry.i)
                    },
                    ::core::option::Option::None => {
                        b_entry.i.cmp(&a_entry.i)
                    },
                },
            },
        }
    });

    let mut out: Vec<String> = Vec::with_capacity(total_len);
    let mut sum: f64 = 0.0;
    let mut weight: f64 = 0.0;
    let mut vs_index: usize = 0;

    fn consume_unsortable(
        out: &mut Vec<String>,
        entries: &[SortEntry],
        unsortable: &mut Vec<usize>,
        mut index: usize,
    ) -> usize {
        while let Some(&last_ix) = unsortable.last() {
            let last = &entries[last_ix];

            match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(last.i), &::core::convert::identity(index)); _explicit_ordering } {
                ::core::cmp::Ordering::Greater => {
                    break;
                },
                ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Equal => {},
            }

            let last_ix = {
                let optional_last_ix = unsortable.pop();

                match optional_last_ix {
                    Some(matched_last_ix) => matched_last_ix,
                    ::core::option::Option::None => break,
                }
            };
            out.extend(entries[last_ix].vs.iter().cloned());
            index += 1;
        }

        index
    }

    vs_index = consume_unsortable(&mut out, entries, &mut unsortable, vs_index);

    for entry_ix in sortable {
        let entry = &entries[entry_ix];
        vs_index += entry.vs.len();
        out.extend(entry.vs.iter().cloned());

        match (entry.barycenter, entry.weight) {
            (Some(bc), Some(w)) => {
                sum += bc * w;
                weight += w;
            },
            (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {},
        }

        vs_index = consume_unsortable(&mut out, entries, &mut unsortable, vs_index);
    }

    match {
        ::core::primitive::bool::then_some(weight != 0.0, ())
    } {
        ::core::option::Option::Some(()) => {
            SortResult {
                vs: out,
                barycenter: Some(sum / weight),
                weight: Some(weight),
            }
        },
        ::core::option::Option::None => {
            SortResult {
                vs: out,
                barycenter: None,
                weight: None,
            }
        },
    }
}

#[must_use]
pub(crate) fn sort_ix(entries: &[SortEntryIx], bias_right: bool) -> SortResultIx {
    let mut total_len: usize = 0;
    let mut sortable: Vec<usize> = Vec::new();
    let mut unsortable: Vec<usize> = Vec::new();

    for (ix, entry) in entries.iter().enumerate() {
        total_len += entry.vs.len();

        match {
            ::core::primitive::bool::then_some(entry.barycenter.is_some(), ())
        } {
            ::core::option::Option::Some(()) => {
                sortable.push(ix);
            },
            ::core::option::Option::None => {
                unsortable.push(ix);
            },
        }
    }

    unsortable.sort_by(|&a, &b| entries[b].i.cmp(&entries[a].i));

    sortable.sort_by(|&a, &b| {
        let a_entry = &entries[a];
        let b_entry = &entries[b];
        let a_bc = match a_entry.barycenter {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0.0,
        };
        let b_bc = match b_entry.barycenter {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => 0.0,
        };

        match {
            ::core::primitive::bool::then_some(a_bc < b_bc, ())
        } {
            ::core::option::Option::Some(()) => {
                std::cmp::Ordering::Less
            },
            ::core::option::Option::None => match {
                ::core::primitive::bool::then_some(a_bc > b_bc, ())
            } {
                ::core::option::Option::Some(()) => {
                    std::cmp::Ordering::Greater
                },
                ::core::option::Option::None => match {
                    ::core::primitive::bool::then_some(!bias_right, ())
                } {
                    ::core::option::Option::Some(()) => {
                        a_entry.i.cmp(&b_entry.i)
                    },
                    ::core::option::Option::None => {
                        b_entry.i.cmp(&a_entry.i)
                    },
                },
            },
        }
    });

    let mut out: Vec<usize> = Vec::with_capacity(total_len);
    let mut sum: f64 = 0.0;
    let mut weight: f64 = 0.0;
    let mut vs_index: usize = 0;

    fn consume_unsortable(
        out: &mut Vec<usize>,
        entries: &[SortEntryIx],
        unsortable: &mut Vec<usize>,
        mut index: usize,
    ) -> usize {
        while let Some(&last_ix) = unsortable.last() {
            let last = &entries[last_ix];

            match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(last.i), &::core::convert::identity(index)); _explicit_ordering } {
                ::core::cmp::Ordering::Greater => {
                    break;
                },
                ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Equal => {},
            }

            let last_ix = {
                let optional_last_ix = unsortable.pop();

                match optional_last_ix {
                    Some(matched_last_ix) => matched_last_ix,
                    ::core::option::Option::None => break,
                }
            };
            out.extend(entries[last_ix].vs.iter().copied());
            index += 1;
        }

        index
    }

    vs_index = consume_unsortable(&mut out, entries, &mut unsortable, vs_index);

    for entry_ix in sortable {
        let entry = &entries[entry_ix];
        vs_index += entry.vs.len();
        out.extend(entry.vs.iter().copied());

        match (entry.barycenter, entry.weight) {
            (Some(bc), Some(w)) => {
                sum += bc * w;
                weight += w;
            },
            (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {},
        }

        vs_index = consume_unsortable(&mut out, entries, &mut unsortable, vs_index);
    }

    match {
        ::core::primitive::bool::then_some(weight != 0.0, ())
    } {
        ::core::option::Option::Some(()) => {
            SortResultIx {
                vs: out,
                barycenter: Some(sum / weight),
                weight: Some(weight),
            }
        },
        ::core::option::Option::None => {
            SortResultIx {
                vs: out,
                barycenter: None,
                weight: None,
            }
        },
    }
}

#[must_use]
pub fn sort_subgraph<N, E, G, CN, CE, CG>(
    g: &Graph<N, E, G>,
    v: &str,
    cg: &Graph<CN, CE, CG>,
    bias_right: bool,
) -> SortResult
where
    N: Default + OrderNodeLabel + Clone + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
    CN: Default + 'static,
    CE: Default + 'static,
    CG: Default,
{
    struct SortSubgraphFrame {
        v: String,
        barycenters: Vec<BarycenterEntry>,
        border_left: Option<String>,
        border_right: Option<String>,
    }

    enum SortSubgraphStep {
        Enter(String),
        Exit(SortSubgraphFrame),
    }

    let root_id = v.to_string();
    let mut root_result: Option<SortResult> = None;
    let mut results: HashMap<String, SortResult> = HashMap::default();
    let mut stack = vec![SortSubgraphStep::Enter(root_id.clone())];

    while let Some(step) = stack.pop() {
        match step {
            SortSubgraphStep::Enter(v) => {
                let mut movable: Vec<String> =
                    g.children(&v).into_iter().map(|s| s.to_string()).collect();

                let (border_left, border_right) = g.node(&v).map_or((None, None), |node| {
                    (
                        node.border_left().map(|s| s.to_string()),
                        node.border_right().map(|s| s.to_string()),
                    )
                });

                match (border_left.as_deref(), border_right.as_deref()) {
                    (Some(bl), Some(br)) => {
                        movable.retain(|w| w != bl && w != br);
                    },
                    (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {},
                }

                let barycenters = barycenter(g, &movable);
                let child_ids = barycenters
                    .iter()
                    .filter(|entry| !g.children(&entry.v).is_empty())
                    .map(|entry| entry.v.clone())
                    .collect::<Vec<_>>();

                stack.push(SortSubgraphStep::Exit(SortSubgraphFrame {
                    v,
                    barycenters,
                    border_left,
                    border_right,
                }));

                for child in child_ids.into_iter().rev() {
                    stack.push(SortSubgraphStep::Enter(child));
                }
            }
            SortSubgraphStep::Exit(mut frame) => {
                let mut subgraphs: HashMap<String, SortResult> = HashMap::default();

                'over_entry_2: for entry in &mut frame.barycenters {
                    let subgraph_result = {
                        let optional_subgraph_result = results.get(&entry.v).cloned();

                        match optional_subgraph_result {
                            Some(matched_subgraph_result) => matched_subgraph_result,
                            ::core::option::Option::None => continue 'over_entry_2,
                        }
                    };

                    match {
                        ::core::primitive::bool::then_some(subgraph_result.barycenter.is_some(), ())
                    } {
                        ::core::option::Option::Some(()) => {
                            merge_barycenters(entry, &subgraph_result);
                        },
                        ::core::option::Option::None => {},
                    }

                    subgraphs.insert(entry.v.clone(), subgraph_result);
                }

                let mut entries = resolve_conflicts(&frame.barycenters, cg);
                expand_subgraphs(&mut entries, &subgraphs);
                let mut result = sort(&entries, bias_right);

                match (frame.border_left.as_deref(), frame.border_right.as_deref()) {
                    (Some(bl), Some(br)) => {
                        let mut out: Vec<String> = Vec::with_capacity(result.vs.len() + 2);
                        out.push(bl.to_string());
                        out.extend(result.vs);
                        out.push(br.to_string());
                        result.vs = out;

                        {
                            match (g.first_predecessor(bl), g.first_predecessor(br)) {
                                (Some(bl_pred), Some(br_pred)) => {
                                    let bl_order = (match g.node(bl_pred).and_then(|n| n.order()) {
                                        ::core::option::Option::Some(present) => present,
                                        ::core::option::Option::None => 0,
                                    }) as f64;
                                    let br_order = (match g.node(br_pred).and_then(|n| n.order()) {
                                        ::core::option::Option::Some(present) => present,
                                        ::core::option::Option::None => 0,
                                    }) as f64;

                                    let bc = match result.barycenter {
                                        ::core::option::Option::Some(present) => present,
                                        ::core::option::Option::None => 0.0,
                                    };
                                    let w = match result.weight {
                                        ::core::option::Option::Some(present) => present,
                                        ::core::option::Option::None => 0.0,
                                    };
                                    let denom = w + 2.0;
                                    result.barycenter = Some((bc * w + bl_order + br_order) / denom);
                                    result.weight = Some(denom);
                                },
                                (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {},
                            };
                        }
                    },
                    (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {},
                }

                match {
                    ::core::primitive::bool::then_some(frame.v == root_id, ())
                } {
                    ::core::option::Option::Some(()) => {
                        root_result = Some(result.clone());
                    },
                    ::core::option::Option::None => {},
                }

                results.insert(frame.v, result);
            }
        }
    }

    match (root_result, SortResult {
            vs: Vec::new(),
            barycenter: None,
            weight: None,
        }) {
        (::core::option::Option::Some(present), alternate) => { ::core::mem::drop(alternate); present },
        (::core::option::Option::None, alternate) => alternate,
    }
}

fn expand_subgraphs(entries: &mut [SortEntry], subgraphs: &HashMap<String, SortResult>) {
    for entry in entries {
        let mut out: Vec<String> = Vec::new();

        for v in &entry.vs {
            if let Some(sg) = subgraphs.get(v) {
                out.extend(sg.vs.iter().cloned());
            } else {
                out.push(v.clone());
            }
        }

        entry.vs = out;
    }
}

fn merge_barycenters(target: &mut BarycenterEntry, other: &SortResult) {
    let other_bc = match other.barycenter {
        Some(matched_other_bc) => matched_other_bc,
        ::core::option::Option::None => return,
    };
    let other_w = match other.weight {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0.0,
    };

    match (target.barycenter, target.weight) {
        (Some(bc), Some(w)) => {
            let denom = w + other_w;
            target.barycenter = Some((bc * w + other_bc * other_w) / denom);
            target.weight = Some(denom);
        },
        (::core::option::Option::None, _) | (_, ::core::option::Option::None) => {
            target.barycenter = Some(other_bc);
            target.weight = Some(other_w);
        },
    }
}
