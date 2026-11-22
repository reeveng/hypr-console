use super::OrderNodeRange;
use crate::graphlib::Graph;
use console_core_perfect_hash::collections::FixedSet as HashSet;

#[must_use]
pub fn init_order<N, E, G>(g: &Graph<N, E, G>) -> Vec<Vec<String>>
where
    N: Default + OrderNodeRange + 'static,
    E: Default + 'static,
    G: Default,
{
    let mut visited: HashSet<String> = HashSet::default();

    let simple_nodes: Vec<String> = g
        .nodes()
        .filter(|v| g.children_iter(v).next().is_none())
        .map(|v| v.to_string())
        .collect();

    let mut max_rank: i32 = i32::MIN;

    for v in &simple_nodes {
        let rank = {
            let optional_rank = g.node(v).and_then(|n| n.rank());

            match optional_rank {
                Some(matched_rank) => matched_rank,
                ::core::option::Option::None => continue,
            }
        };
        max_rank = max_rank.max(rank);
    }

    match {
        ::core::primitive::bool::then_some(max_rank == i32::MIN, ())
    } {
        ::core::option::Option::Some(()) => {
            return Vec::new();
        },
        ::core::option::Option::None => {},
    }

    let mut layers: Vec<Vec<String>> = vec![Vec::new(); (max_rank + 1).max(0) as usize];

    let mut ordered_vs = simple_nodes;

    // `simple_nodes` already follows pinned Graphlib's JavaScript object-key enumeration.
    // Rust's stable `sort_by_key` therefore preserves the official relative order within a rank
    // without a second ID vector or an auxiliary String-keyed position map.
    ordered_vs.sort_by_key(|v| match g.node(v).and_then(|n| n.rank()) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => i32::MAX,
    });

    for v in ordered_vs {
        let mut stack = vec![v];

        'inner_while_2: while let Some(v) = stack.pop() {
            match {
                ::core::primitive::bool::then_some(!visited.insert(v.clone()), ())
            } {
                ::core::option::Option::Some(()) => {
                    continue 'inner_while_2;
                },
                ::core::option::Option::None => {},
            }

            let rank = {
                let optional_rank_2 = g.node(&v).and_then(|n| n.rank());

                match optional_rank_2 {
                    Some(matched_rank_2) => matched_rank_2,
                    ::core::option::Option::None => continue 'inner_while_2,
                }
            };
            let idx = rank.max(0) as usize;

            match layers.get_mut(idx) {
                Some(layer) => {
                    layer.push(v.clone());
                },
                ::core::option::Option::None => {},
            }

            let successors = g.successors(&v);

            for w in successors.into_iter().rev() {
                stack.push(w.to_string());
            }
        }
    }

    layers
}
