use crate::error::{Error, Result, WorkFailure};
use crate::graph::{Graph, LayoutResult, Point};
use crate::work::admit_dynamic_work;
use crate::{NoopWorkControl, WorkControl};
use console_core_perfect_hash::collections::{FixedMap as HashMap, FixedSet as HashSet};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy)]
pub struct IndexedNode {
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct IndexedEdge {
    pub a: usize,
    pub b: usize,
}

pub fn layout_indexed(nodes: &[IndexedNode], edges: &[IndexedEdge]) -> Result<Vec<Point>> {
    let mut work_control = NoopWorkControl;
    layout_indexed_with_work_control(nodes, edges, &mut work_control)
}

/// Lay out an indexed flat graph with caller-owned work accounting.
pub fn layout_indexed_with_work_control<W: WorkControl + ?Sized>(
    nodes: &[IndexedNode],
    edges: &[IndexedEdge],
    work_control: &mut W,
) -> Result<Vec<Point>> {
    match nodes {
        [] => {
            return Ok(Vec::new());
        },
        [_, ..] => {},
    }

    let projection_work = nodes
        .len()
        .checked_mul(3)
        .and_then(|units| {
            edges
                .len()
                .checked_mul(2)
                .and_then(|edge_units| units.checked_add(edge_units))
        })
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    work_control.check(projection_work)?;

    for (idx, e) in edges.iter().enumerate() {
        match {
            ::core::primitive::bool::then_some(e.a >= nodes.len() || e.b >= nodes.len(), ())
        } {
            ::core::option::Option::Some(()) => {
                return Err(Error::MissingEndpoint {
                    edge_id: format!("#{idx}"),
                });
            },
            ::core::option::Option::None => {},
        }
    }

    work_control.charge(projection_work)?;

    let mut sim = SimGraph::from_indexed(nodes, edges);
    sim.run_layout(work_control)?;

    let mut out: Vec<Point> = Vec::with_capacity(sim.nodes.len());

    for n in &sim.nodes {
        out.push(Point {
            x: n.center_x(),
            y: n.center_y(),
        });
    }

    Ok(out)
}

pub fn layout(graph: &Graph) -> Result<LayoutResult> {
    let mut work_control = NoopWorkControl;
    layout_with_work_control(graph, &mut work_control)
}

/// Lay out a flat graph with caller-owned work accounting.
pub fn layout_with_work_control<W: WorkControl + ?Sized>(
    graph: &Graph,
    work_control: &mut W,
) -> Result<LayoutResult> {
    let projection_work = graph
        .nodes
        .len()
        .checked_mul(4)
        .and_then(|units| {
            graph
                .edges
                .len()
                .checked_mul(2)
                .and_then(|edge_units| units.checked_add(edge_units))
        })
        .and_then(|units| units.checked_add(graph.compounds.len()))
        .ok_or(WorkFailure::ArithmeticOverflow)?;
    work_control.check(projection_work)?;
    graph.validate()?;
    work_control.charge(projection_work)?;
    let mut sim = SimGraph::from_graph(graph);
    sim.run_layout(work_control)?;

    let mut positions: std::collections::BTreeMap<String, Point> =
        std::collections::BTreeMap::new();

    for (input, n) in graph.nodes.iter().zip(sim.nodes) {
        let x = n.center_x();
        let y = n.center_y();
        positions.insert(input.id.clone(), Point { x, y });
    }

    Ok(LayoutResult { positions })
}

#[derive(Debug, Clone)]
struct SimNode {
    width: f64,
    height: f64,
    half_width: f64,
    half_height: f64,
    // Top-left anchored rectangle, matching upstream `layout-base` `LNode.rect`.
    left: f64,
    top: f64,
    // Incident edge indices in insertion order, matching `LNode.edges` order.
    edges: Vec<usize>,
    // Cached repulsion candidates for the FR-grid variant (`FDLayoutNode.surrounding`).
    surrounding: Vec<usize>,
    active: bool,

    // FR-grid indices computed by `update_grid` for repulsion candidate lookup.
    start_x: i32,
    finish_x: i32,
    start_y: i32,
    finish_y: i32,

    // Forces (reset each iteration), matching `FDLayoutNode` / `CoSENode`.
    spring_fx: f64,
    spring_fy: f64,
    repulsion_fx: f64,
    repulsion_fy: f64,
    gravitation_fx: f64,
    gravitation_fy: f64,
}

impl SimNode {
    fn set_center(&mut self, cx: f64, cy: f64) {
        self.left = cx - self.half_width;
        self.top = cy - self.half_height;
    }

    #[must_use]
    fn center_x(&self) -> f64 {
        self.left + self.half_width
    }

    #[must_use]
    fn center_y(&self) -> f64 {
        self.top + self.half_height
    }

    #[must_use]
    fn diagonal(&self) -> f64 {
        (self.width * self.width + self.height * self.height).sqrt()
    }

    fn move_by(&mut self, dx: f64, dy: f64) {
        self.left += dx;
        self.top += dy;
    }

    #[must_use]
    fn half_w(&self) -> f64 {
        self.half_width
    }

    #[must_use]
    fn half_h(&self) -> f64 {
        self.half_height
    }

    #[must_use]
    fn right(&self) -> f64 {
        self.left + self.width
    }

    #[must_use]
    fn bottom(&self) -> f64 {
        self.top + self.height
    }
}

#[derive(Debug, Clone, Copy)]
struct SimEdge {
    a: usize,
    b: usize,
    active: bool,
}

#[derive(Debug, Clone, Copy)]
struct RadialBranchFrame {
    node: usize,
    parent: Option<usize>,
    start_angle: f64,
    end_angle: f64,
    distance: f64,
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Bounds {
    #[must_use]
    fn from_nodes(nodes: &[SimNode], tree: &[usize]) -> Option<Self> {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for &idx in tree {
            let n = &nodes[idx];
            min_x = min_x.min(n.left);
            min_y = min_y.min(n.top);
            max_x = max_x.max(n.left + n.width);
            max_y = max_y.max(n.top + n.height);
        }

        match {
            ::core::primitive::bool::then_some(!(min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite()), ())
        } {
            ::core::option::Option::Some(()) => {
                return None;
            },
            ::core::option::Option::None => {},
        }

        Some(Self {
            min_x,
            min_y,
            max_x,
            max_y,
        })
    }
}

fn checked_grid_dimension(span: f64, range: f64) -> std::result::Result<usize, WorkFailure> {
    match {
        ::core::primitive::bool::then_some(!(span.is_finite() && range.is_finite() && range > 0.0), ())
    } {
        ::core::option::Option::Some(()) => {
            return Err(WorkFailure::ArithmeticOverflow);
        },
        ::core::option::Option::None => {},
    }

    let dimension = (span / range).ceil().max(1.0);

    match {
        ::core::primitive::bool::then_some(!dimension.is_finite() || dimension > i32::MAX as f64, ())
    } {
        ::core::option::Option::Some(()) => {
            return Err(WorkFailure::ArithmeticOverflow);
        },
        ::core::option::Option::None => {},
    }

    Ok(dimension as usize)
}

fn implicit_grid_work_units(node_count: usize) -> std::result::Result<usize, WorkFailure> {
    match node_count {
        0 => {
            return Ok(0);
        },
        _ => {},
    }

    let comparison_levels = match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(node_count), &::core::convert::identity(1)); _explicit_ordering } {
        ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Equal => {
            1
        },
        ::core::cmp::Ordering::Greater => {
            usize::BITS as usize - (node_count - 1).leading_zeros() as usize
        },
    };
    node_count
        .checked_mul(node_count)
        .and_then(|units| units.checked_mul(comparison_levels))
        .and_then(|units| units.checked_add(node_count))
        .ok_or(WorkFailure::ArithmeticOverflow)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SimGridStorageKind {
    Dense,
    Sparse,
    Implicit,
}

#[derive(Debug, Clone, Copy)]
struct SimGridPlan {
    size_x: usize,
    size_y: usize,
    total_cell_count: usize,
    cell_reference_count: usize,
    active_node_count: usize,
    storage_kind: SimGridStorageKind,
    work_units: usize,
}

impl SimGridPlan {
    fn new(
        size_x: usize,
        size_y: usize,
        cell_reference_count: usize,
        active_node_count: usize,
    ) -> std::result::Result<Self, WorkFailure> {
        let total_cell_count = size_x
            .checked_mul(size_y)
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        let candidates = [
            (
                SimGridStorageKind::Dense,
                total_cell_count.checked_add(cell_reference_count),
            ),
            (
                SimGridStorageKind::Sparse,
                cell_reference_count.checked_mul(2),
            ),
            (
                SimGridStorageKind::Implicit,
                implicit_grid_work_units(active_node_count).ok(),
            ),
        ];
        let (storage_kind, storage_work) = candidates
            .into_iter()
            .filter_map(|(kind, work)| work.map(|work| (kind, work)))
            .min_by_key(|(_, work)| *work)
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        let work_units = storage_work
            .checked_add(active_node_count)
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        Ok(Self {
            size_x,
            size_y,
            total_cell_count,
            cell_reference_count,
            active_node_count,
            storage_kind,
            work_units,
        })
    }
}

#[derive(Debug, Clone)]
enum SimGridCells {
    Dense(Vec<Vec<usize>>),
    Sparse(HashMap<(usize, usize), Vec<usize>>),
    Implicit(Vec<usize>),
}

#[derive(Debug, Clone)]
struct SimGrid {
    size_x: usize,
    size_y: usize,
    cells: SimGridCells,
}

impl Default for SimGrid {
    fn default() -> Self {
        Self {
            size_x: 0,
            size_y: 0,
            cells: SimGridCells::Dense(Vec::new()),
        }
    }
}

impl SimGrid {
    #[must_use]
    fn is_empty(&self) -> bool {
        self.size_x == 0 || self.size_y == 0
    }

    #[must_use]
    fn size_x(&self) -> usize {
        self.size_x
    }

    #[must_use]
    fn size_y(&self) -> usize {
        self.size_y
    }

    #[must_use]
    fn is_implicit(&self) -> bool {
        matches!(self.cells, SimGridCells::Implicit(_))
    }

    #[must_use]
    fn clear_work_units(&self) -> usize {
        match &self.cells {
            SimGridCells::Dense(cells) => cells.len(),
            SimGridCells::Sparse(cells) => cells.len(),
            SimGridCells::Implicit(node_order) => node_order.len(),
        }
    }

    fn clear_cells(&mut self) {
        match &mut self.cells {
            SimGridCells::Dense(cells) => {
                for cell in cells {
                    cell.clear();
                }
            }
            SimGridCells::Sparse(cells) => cells.clear(),
            SimGridCells::Implicit(node_order) => node_order.clear(),
        }
    }

    fn reset(
        &mut self,
        plan: SimGridPlan,
        _left: f64,
        _top: f64,
        _range: f64,
    ) -> std::result::Result<(), WorkFailure> {
        self.size_x = plan.size_x;
        self.size_y = plan.size_y;

        match plan.storage_kind {
            SimGridStorageKind::Dense => {
                match {
                    ::core::primitive::bool::then_some(!matches!(self.cells, SimGridCells::Dense(_)), ())
                } {
                    ::core::option::Option::Some(()) => {
                        self.cells = SimGridCells::Dense(Vec::new());
                    },
                    ::core::option::Option::None => {},
                }

                let SimGridCells::Dense(cells) = &mut self.cells else {
                    unreachable!("dense grid selected above")
                };

                match {
                    ::core::primitive::bool::then_some(plan.total_cell_count > cells.len(), ())
                } {
                    ::core::option::Option::Some(()) => {
                        cells
                            .try_reserve_exact(plan.total_cell_count - cells.len())
                            .map_err(|_| WorkFailure::ArithmeticOverflow)?;
                        cells.resize_with(plan.total_cell_count, Vec::new);
                    },
                    ::core::option::Option::None => {
                        cells.truncate(plan.total_cell_count);
                    },
                }

                for cell in cells {
                    cell.clear();
                }
            }
            SimGridStorageKind::Sparse => {
                match {
                    ::core::primitive::bool::then_some(!matches!(self.cells, SimGridCells::Sparse(_)), ())
                } {
                    ::core::option::Option::Some(()) => {
                        self.cells = SimGridCells::Sparse(HashMap::default());
                    },
                    ::core::option::Option::None => {},
                }

                let SimGridCells::Sparse(cells) = &mut self.cells else {
                    unreachable!("sparse grid selected above")
                };

                cells.clear();
                cells
                    .try_reserve(plan.cell_reference_count.min(plan.total_cell_count))
                    .map_err(|_| WorkFailure::ArithmeticOverflow)?;
            }
            SimGridStorageKind::Implicit => {
                match {
                    ::core::primitive::bool::then_some(!matches!(self.cells, SimGridCells::Implicit(_)), ())
                } {
                    ::core::option::Option::Some(()) => {
                        self.cells = SimGridCells::Implicit(Vec::new());
                    },
                    ::core::option::Option::None => {},
                }

                let SimGridCells::Implicit(node_order) = &mut self.cells else {
                    unreachable!("implicit grid selected above")
                };

                node_order.clear();
                node_order
                    .try_reserve_exact(plan.active_node_count)
                    .map_err(|_| WorkFailure::ArithmeticOverflow)?;
            }
        }

        Ok(())
    }

    #[inline]
    #[must_use]
    fn idx(&self, x: usize, y: usize) -> usize {
        (x * self.size_y) + y
    }

    #[inline]
    fn push(&mut self, x: usize, y: usize, node_idx: usize) {
        let i = self.idx(x, y);

        match &mut self.cells {
            SimGridCells::Dense(cells) => cells[i].push(node_idx),
            SimGridCells::Sparse(cells) => cells.entry((x, y)).or_default().push(node_idx),
            SimGridCells::Implicit(_) => {}
        }
    }

    fn register_implicit_node(&mut self, node_idx: usize) {
        match &mut self.cells {
            SimGridCells::Implicit(node_order) => {
                node_order.push(node_idx);
            },
            SimGridCells::Dense(_) | SimGridCells::Sparse(_) => {},
        }
    }

    #[must_use]
    fn cell_scan_work(&self, x: usize, y: usize) -> usize {
        match &self.cells {
            SimGridCells::Dense(cells) => cells[self.idx(x, y)].len(),
            SimGridCells::Sparse(cells) => match cells.get(&(x, y)).map(Vec::len) {
                ::core::option::Option::Some(option_value) => option_value,
                ::core::option::Option::None => 0,
            },
            SimGridCells::Implicit(node_order) => node_order.len(),
        }
    }

    fn fill_cell_candidates(
        &self,
        nodes: &[SimNode],
        x: usize,
        y: usize,
        candidates: &mut Vec<usize>,
    ) {
        candidates.clear();

        match &self.cells {
            SimGridCells::Dense(cells) => {
                candidates.extend_from_slice(&cells[self.idx(x, y)]);
            }
            SimGridCells::Sparse(cells) => {
                {
                    match cells.get(&(x, y)) {
                        Some(cell) => {
                            candidates.extend_from_slice(cell);
                        },
                        ::core::option::Option::None => {},
                    };
                }
            }
            SimGridCells::Implicit(node_order) => {
                let x = x as i32;
                let y = y as i32;
                candidates.extend(node_order.iter().copied().filter(|&node_idx| {
                    let node = &nodes[node_idx];
                    node.start_x <= x
                        && x <= node.finish_x
                        && node.start_y <= y
                        && y <= node.finish_y
                }));
            }
        }
    }
}

#[derive(Debug)]
struct SimGraph {
    nodes: Vec<SimNode>,
    edges: Vec<SimEdge>,
    grid: SimGrid,
    repulsion_seen: Vec<u32>,
    repulsion_seen_gen: u32,
}

impl SimGraph {
    const DEFAULT_GRAPH_MARGIN: f64 = 15.0;
    const DEFAULT_COMPONENT_SEPERATION: f64 = 60.0; // upstream typo preserved
    const DEFAULT_EDGE_LENGTH: f64 = 50.0;
    const DEFAULT_RADIAL_SEPARATION: f64 = Self::DEFAULT_EDGE_LENGTH;

    // `layout-base` `LayoutConstants.WORLD_CENTER_X/Y`.
    const WORLD_CENTER_X: f64 = 1200.0;
    const WORLD_CENTER_Y: f64 = 900.0;

    const MAX_ITERATIONS: usize = 2500;
    const CONVERGENCE_CHECK_PERIOD: usize = 100;
    const MAX_NODE_DISPLACEMENT: f64 = 300.0;
    const MIN_REPULSION_DIST: f64 = Self::DEFAULT_EDGE_LENGTH / 10.0;
    const GRID_CALCULATION_CHECK_PERIOD: usize = 10; // `FDLayoutConstants.GRID_CALCULATION_CHECK_PERIOD`

    // cytoscape-cose-bilkent default options (Mermaid uses these in `cose-bilkent/cytoscape-setup.ts`).
    const DEFAULT_SPRING_STRENGTH: f64 = 0.45; // edgeElasticity
    const DEFAULT_REPULSION_STRENGTH: f64 = 4500.0; // nodeRepulsion
    const DEFAULT_GRAVITY_STRENGTH: f64 = 0.25; // gravity
    const DEFAULT_GRAVITY_RANGE_FACTOR: f64 = 3.8; // gravityRange

    #[inline]
    #[must_use]
    fn imath_sign(value: f64) -> f64 {
        // Port of `layout-base` `IMath.sign`: returns 0 for 0.
        match {
            ::core::primitive::bool::then_some(value > 0.0, ())
        } {
            ::core::option::Option::Some(()) => {
                1.0
            },
            ::core::option::Option::None => match {
                ::core::primitive::bool::then_some(value < 0.0, ())
            } {
                ::core::option::Option::Some(()) => {
                    -1.0
                },
                ::core::option::Option::None => {
                    0.0
                },
            },
        }
    }

    #[must_use]
    fn from_indexed(nodes_in: &[IndexedNode], edges_in: &[IndexedEdge]) -> Self {
        Self::from_parts(nodes_in.iter().copied(), edges_in.iter().copied())
    }

    #[must_use]
    fn from_graph(graph: &Graph) -> Self {
        let mut id_to_idx: HashMap<&str, usize> =
            HashMap::with_capacity_and_hasher(graph.nodes.len(), Default::default());

        for (idx, n) in graph.nodes.iter().enumerate() {
            id_to_idx.insert(n.id.as_str(), idx);
        }

        Self::from_parts(
            graph.nodes.iter().map(|n| IndexedNode {
                width: n.width,
                height: n.height,
                x: n.x,
                y: n.y,
            }),
            graph.edges.iter().map(|e| IndexedEdge {
                a: id_to_idx[e.source.as_str()],
                b: id_to_idx[e.target.as_str()],
            }),
        )
    }

    // Public entry points validate endpoints before either adapter constructs the simulation.
    #[must_use]
    fn from_parts(
        nodes_in: impl ExactSizeIterator<Item = IndexedNode>,
        edges_in: impl ExactSizeIterator<Item = IndexedEdge>,
    ) -> Self {
        let node_count = nodes_in.len();
        let mut nodes: Vec<SimNode> = Vec::with_capacity(nodes_in.len());

        for n in nodes_in {
            let w = n.width.max(1.0);
            let h = n.height.max(1.0);
            let hw = w * 0.5;
            let hh = h * 0.5;
            nodes.push(SimNode {
                width: w,
                height: h,
                half_width: hw,
                half_height: hh,
                left: n.x - hw,
                top: n.y - hh,
                edges: Vec::new(),
                surrounding: Vec::new(),
                active: true,
                start_x: 0,
                finish_x: 0,
                start_y: 0,
                finish_y: 0,
                spring_fx: 0.0,
                spring_fy: 0.0,
                repulsion_fx: 0.0,
                repulsion_fy: 0.0,
                gravitation_fx: 0.0,
                gravitation_fy: 0.0,
            });
        }

        // Mirror cytoscape-cose-bilkent: retain the first edge for each unordered node pair.
        let mut seen_pairs: HashSet<(usize, usize)> =
            HashSet::with_capacity_and_hasher(edges_in.len(), Default::default());
        let mut edges: Vec<SimEdge> = Vec::with_capacity(edges_in.len());

        for e in edges_in {
            let (a, b) = (e.a, e.b);

            match {
                ::core::primitive::bool::then_some(a == b, ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            let (u, v) = match {
                ::core::primitive::bool::then_some(a < b, ())
            } {
                ::core::option::Option::Some(()) => { (a, b) },
                ::core::option::Option::None => { (b, a) },
            };

            match {
                ::core::primitive::bool::then_some(!seen_pairs.insert((u, v)), ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            let ei = edges.len();
            edges.push(SimEdge { a, b, active: true });
            nodes[a].edges.push(ei);
            nodes[b].edges.push(ei);
        }

        Self {
            nodes,
            edges,
            grid: SimGrid::default(),
            repulsion_seen: vec![0u32; node_count],
            repulsion_seen_gen: 1,
        }
    }

    fn run_layout<W: WorkControl + ?Sized>(&mut self, work_control: &mut W) -> Result<()> {
        // Keep both public surfaces on the same upstream sequence: radial forest seeding,
        // spring ticks, then layout-base's transform to positive coordinates. Non-forest graphs
        // retain their supplied initial positions before spring ticks.
        let forest = self.get_flat_forest(work_control)?;

        match {
            ::core::primitive::bool::then_some(!forest.is_empty(), ())
        } {
            ::core::option::Option::Some(()) => {
                self.position_nodes_radially(&forest, work_control)?;
            },
            ::core::option::Option::None => {},
        }

        self.run_spring_embedder(work_control)?;
        admit_dynamic_work(
            work_control,
            self.nodes
                .len()
                .checked_mul(2)
                .ok_or(WorkFailure::ArithmeticOverflow)?,
        )?;
        self.transform_to_origin();
        Ok(())
    }

    #[must_use]
    fn edge_other_end(&self, edge_idx: usize, node_idx: usize) -> usize {
        let e = self.edges[edge_idx];

        match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(e.a), &::core::convert::identity(node_idx)); _explicit_ordering } {
            ::core::cmp::Ordering::Equal => {
                e.b
            },
            ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Greater => {
                debug_assert_eq!(e.b, node_idx);
                e.a
            },
        }
    }

    fn for_each_active_neighbor(&self, node_idx: usize, mut f: impl FnMut(usize)) {
        for &ei in &self.nodes[node_idx].edges {
            match {
                ::core::primitive::bool::then_some(!self.edges[ei].active, ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            let other = self.edge_other_end(ei, node_idx);

            match {
                ::core::primitive::bool::then_some(!self.nodes[other].active, ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            f(other);
        }
    }

    #[must_use]
    fn active_edge_between(&self, a: usize, b: usize) -> Option<usize> {
        for &ei in &self.nodes[a].edges {
            match {
                ::core::primitive::bool::then_some(!self.edges[ei].active, ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(self.edge_other_end(ei, a)), &::core::convert::identity(b)); _explicit_ordering } {
                ::core::cmp::Ordering::Equal => {
                    return Some(ei);
                },
                ::core::cmp::Ordering::Less | ::core::cmp::Ordering::Greater => {},
            }
        }

        None
    }

    /// Port of `layout-base` `Layout.getFlatForest()` for flat graphs.
    fn get_flat_forest<W: WorkControl + ?Sized>(
        &self,
        work_control: &mut W,
    ) -> std::result::Result<Vec<Vec<usize>>, WorkFailure> {
        let mut flat_forest: Vec<Vec<usize>> = Vec::new();
        let mut is_forest = true;

        let scratch_work = self
            .nodes
            .len()
            .checked_mul(4)
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        admit_dynamic_work(work_control, scratch_work)?;

        // Root graph nodes in insertion order.
        let all_nodes: Vec<usize> = (0..self.nodes.len())
            .filter(|&idx| self.nodes[idx].active)
            .collect();

        // Graph is always flat in our current model (no compound nodes).

        // BFS for each component; reject if any component is not a tree.
        let mut to_be_visited: VecDeque<usize> = VecDeque::new();
        let mut parents: Vec<Option<usize>> = vec![None; self.nodes.len()];
        let mut parents_touched: Vec<usize> = Vec::new();
        let mut visited: Vec<bool> = vec![false; self.nodes.len()];
        let mut unprocessed_nodes: Vec<usize> = all_nodes;

        while !unprocessed_nodes.is_empty() && is_forest {
            to_be_visited.push_back(unprocessed_nodes[0]);

            let mut visited_order: Vec<usize> = Vec::new();

            'inner_while_2: while let Some(current_node) = to_be_visited.pop_front() {
                let traversal_work = self.nodes[current_node]
                    .edges
                    .len()
                    .checked_add(1)
                    .ok_or(WorkFailure::ArithmeticOverflow)?;
                admit_dynamic_work(work_control, traversal_work)?;

                match {
                    ::core::primitive::bool::then_some(!visited[current_node], ())
                } {
                    ::core::option::Option::Some(()) => {
                        visited[current_node] = true;
                        visited_order.push(current_node);
                    },
                    ::core::option::Option::None => {},
                }

                // Traverse all neighbors of this node, in edge insertion order.
                'inner_for_3: for &ei in &self.nodes[current_node].edges {
                    match {
                        ::core::primitive::bool::then_some(!self.edges[ei].active, ())
                    } {
                        ::core::option::Option::Some(()) => {
                            continue 'inner_for_3;
                        },
                        ::core::option::Option::None => {},
                    }

                    let current_neighbor = self.edge_other_end(ei, current_node);

                    match {
                        ::core::primitive::bool::then_some(!self.nodes[current_neighbor].active, ())
                    } {
                        ::core::option::Option::Some(()) => {
                            continue 'inner_for_3;
                        },
                        ::core::option::Option::None => {},
                    }

                    // If BFS is not growing from this neighbor.
                    match {
                        ::core::primitive::bool::then_some(parents[current_node] != Some(current_neighbor), ())
                    } {
                        ::core::option::Option::Some(()) => {
                            match {
                                ::core::primitive::bool::then_some(!visited[current_neighbor], ())
                            } {
                                ::core::option::Option::Some(()) => {
                                    to_be_visited.push_back(current_neighbor);

                                    match {
                                        ::core::primitive::bool::then_some(parents[current_neighbor].is_none(), ())
                                    } {
                                        ::core::option::Option::Some(()) => {
                                            parents_touched.push(current_neighbor);
                                        },
                                        ::core::option::Option::None => {},
                                    }

                                    parents[current_neighbor] = Some(current_node);
                                },
                                ::core::option::Option::None => {
                                    is_forest = false;
                                    break 'inner_for_3;
                                },
                            }
                        },
                        ::core::option::Option::None => {},
                    }
                }

                match {
                    ::core::primitive::bool::then_some(!is_forest, ())
                } {
                    ::core::option::Option::Some(()) => {
                        break 'inner_while_2;
                    },
                    ::core::option::Option::None => {},
                }
            }

            match {
                ::core::primitive::bool::then_some(!is_forest, ())
            } {
                ::core::option::Option::Some(()) => {
                    flat_forest.clear();
                },
                ::core::option::Option::None => {
                    let component_work = visited_order
                        .len()
                        .checked_mul(2)
                        .and_then(|units| units.checked_add(unprocessed_nodes.len()))
                        .and_then(|units| units.checked_add(parents_touched.len()))
                        .ok_or(WorkFailure::ArithmeticOverflow)?;
                    admit_dynamic_work(work_control, component_work)?;
                    // JS Set preserves insertion order; `visited_order` mimics `[...visited]`.
                    flat_forest.push(visited_order.clone());

                    // Remove all visited nodes from unProcessedNodes.
                    unprocessed_nodes.retain(|&n| !visited[n]);

                    // Clear per-component state (only touched indices).
                    for &idx in &visited_order {
                        visited[idx] = false;
                    }

                    for idx in parents_touched.drain(..) {
                        parents[idx] = None;
                    }

                    to_be_visited.clear();
                },
            }
        }

        Ok(flat_forest)
    }

    #[must_use]
    fn active_degree(&self, node_idx: usize) -> usize {
        match {
            ::core::primitive::bool::then_some(!self.nodes[node_idx].active, ())
        } {
            ::core::option::Option::Some(()) => {
                return 0;
            },
            ::core::option::Option::None => {},
        }

        let mut d = 0usize;

        for &ei in &self.nodes[node_idx].edges {
            match {
                ::core::primitive::bool::then_some(!self.edges[ei].active, ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            let other = self.edge_other_end(ei, node_idx);

            match {
                ::core::primitive::bool::then_some(self.nodes[other].active, ())
            } {
                ::core::option::Option::Some(()) => {
                    d += 1;
                },
                ::core::option::Option::None => {},
            }
        }

        d
    }

    fn update_grid<W: WorkControl + ?Sized>(
        &mut self,
        repulsion_range: f64,
        work_control: &mut W,
    ) -> std::result::Result<(), WorkFailure> {
        admit_dynamic_work(work_control, self.nodes.len())?;

        let mut min_left = f64::INFINITY;
        let mut min_top = f64::INFINITY;
        let mut max_right = f64::NEG_INFINITY;
        let mut max_bottom = f64::NEG_INFINITY;
        let mut active_node_count = 0usize;

        for n in &self.nodes {
            match {
                ::core::primitive::bool::then_some(!n.active, ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            active_node_count = active_node_count
                .checked_add(1)
                .ok_or(WorkFailure::ArithmeticOverflow)?;
            min_left = min_left.min(n.left);
            min_top = min_top.min(n.top);
            max_right = max_right.max(n.right());
            max_bottom = max_bottom.max(n.bottom());
        }

        match active_node_count {
            0 => {
                admit_dynamic_work(work_control, self.grid.clear_work_units())?;
                self.grid.clear_cells();
                return Ok(());
            },
            _ => {},
        }

        match {
            ::core::primitive::bool::then_some(!(min_left.is_finite()
            && min_top.is_finite()
            && max_right.is_finite()
            && max_bottom.is_finite()), ())
        } {
            ::core::option::Option::Some(()) => {
                admit_dynamic_work(work_control, self.grid.clear_work_units())?;
                self.grid.clear_cells();
                return Ok(());
            },
            ::core::option::Option::None => {},
        }

        // Match `layout-base` grid semantics:
        // - grid extents are based on the root graph bounds, which include `DEFAULT_GRAPH_MARGIN`
        //   (see `LGraph.updateBounds()` and `FDLayout.updateGrid()`).
        let left_with_margin = min_left - Self::DEFAULT_GRAPH_MARGIN;
        let top_with_margin = min_top - Self::DEFAULT_GRAPH_MARGIN;
        let right_with_margin = max_right + Self::DEFAULT_GRAPH_MARGIN;
        let bottom_with_margin = max_bottom + Self::DEFAULT_GRAPH_MARGIN;

        let size_x = checked_grid_dimension(right_with_margin - left_with_margin, repulsion_range)?;
        let size_y = checked_grid_dimension(bottom_with_margin - top_with_margin, repulsion_range)?;

        let clamp_x = |v: i32| v.clamp(0, (size_x as i32) - 1);
        let clamp_y = |v: i32| v.clamp(0, (size_y as i32) - 1);
        let node_grid_bounds = |n: &SimNode| {
            let start_x = ((n.left - left_with_margin) / repulsion_range).floor() as i32;
            let finish_x = ((n.right() - left_with_margin) / repulsion_range).floor() as i32;
            let start_y = ((n.top - top_with_margin) / repulsion_range).floor() as i32;
            let finish_y = ((n.bottom() - top_with_margin) / repulsion_range).floor() as i32;
            (
                clamp_x(start_x),
                clamp_x(finish_x),
                clamp_y(start_y),
                clamp_y(finish_y),
            )
        };

        admit_dynamic_work(work_control, self.nodes.len())?;
        let mut cell_reference_count = 0usize;

        for n in &self.nodes {
            match {
                ::core::primitive::bool::then_some(!n.active, ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            let (start_x, finish_x, start_y, finish_y) = node_grid_bounds(n);
            let width = usize::try_from(finish_x - start_x)
                .map_err(|_| WorkFailure::ArithmeticOverflow)?
                .checked_add(1)
                .ok_or(WorkFailure::ArithmeticOverflow)?;
            let height = usize::try_from(finish_y - start_y)
                .map_err(|_| WorkFailure::ArithmeticOverflow)?
                .checked_add(1)
                .ok_or(WorkFailure::ArithmeticOverflow)?;
            cell_reference_count = cell_reference_count
                .checked_add(
                    width
                        .checked_mul(height)
                        .ok_or(WorkFailure::ArithmeticOverflow)?,
                )
                .ok_or(WorkFailure::ArithmeticOverflow)?;
        }

        let plan = SimGridPlan::new(size_x, size_y, cell_reference_count, active_node_count)?;
        admit_dynamic_work(work_control, plan.work_units)?;
        self.grid
            .reset(plan, left_with_margin, top_with_margin, repulsion_range)?;
        let implicit = self.grid.is_implicit();

        for (idx, n) in self.nodes.iter_mut().enumerate() {
            match {
                ::core::primitive::bool::then_some(!n.active, ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            // `FDLayout.addNodeToGrid(v, left, top)` where `(left,top)` are root graph bounds
            // (already including `DEFAULT_GRAPH_MARGIN`).
            let (start_x, finish_x, start_y, finish_y) = node_grid_bounds(n);
            n.start_x = start_x;
            n.finish_x = finish_x;
            n.start_y = start_y;
            n.finish_y = finish_y;

            match ::core::primitive::bool::then_some(implicit, ()) {
                ::core::option::Option::Some(()) => {
                    self.grid.register_implicit_node(idx);
                },
                ::core::option::Option::None => {
                    for gx in (n.start_x as usize)..=(n.finish_x as usize) {
                        for gy in (n.start_y as usize)..=(n.finish_y as usize) {
                            self.grid.push(gx, gy, idx);
                        }
                    }
                },
            }
        }

        Ok(())
    }

    /// Port of `layout-base` `Layout.findCenterOfTree(nodes)`.
    /// Note: this intentionally preserves the upstream loop's in-place removal behavior.
    fn find_center_of_tree<W: WorkControl + ?Sized>(
        &self,
        nodes: &[usize],
        work_control: &mut W,
    ) -> std::result::Result<usize, WorkFailure> {
        let setup_work = nodes
            .len()
            .checked_mul(2)
            .and_then(|units| {
                self.nodes
                    .len()
                    .checked_mul(2)
                    .and_then(|scratch| units.checked_add(scratch))
            })
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        admit_dynamic_work(work_control, setup_work)?;
        let mut list: Vec<usize> = nodes.to_vec();
        let mut removed: Vec<bool> = vec![false; self.nodes.len()];
        let mut remaining_degrees: Vec<usize> = vec![0; self.nodes.len()];
        let mut found_center = list.len() == 1 || list.len() == 2;
        let mut center_node = list[0];

        let degree_scan_work = list.iter().try_fold(0usize, |work, &node| {
            work.checked_add(self.nodes[node].edges.len())
                .and_then(|work| work.checked_add(1))
                .ok_or(WorkFailure::ArithmeticOverflow)
        })?;
        admit_dynamic_work(work_control, degree_scan_work)?;

        for &node in &list {
            let degree = self.active_degree(node);
            remaining_degrees[node] = degree;

            match degree {
                1 => {
                    removed[node] = true;
                },
                _ => {},
            }
        }

        let mut temp_list: Vec<usize> = Vec::new();

        for &node in &list {
            match {
                ::core::primitive::bool::then_some(remaining_degrees[node] == 1, ())
            } {
                ::core::option::Option::Some(()) => {
                    temp_list.push(node);
                },
                ::core::option::Option::None => {},
            }
        }

        while !found_center {
            // Upstream bug-for-bug parity:
            // `Layout.findCenterOfTree()` creates `tempList2 = [...tempList]` but then iterates
            // over `list` (not `tempList2`) while removing from `list` in-place:
            //
            //   for (i=0; i<list.length; i++) { node=list[i]; list.splice(indexOf(node), 1); ... }
            //
            // This has the side-effect of skipping every other element. We replicate the exact
            // "remove then i++" semantics by using a `while` loop and `Vec::remove(i)`.
            temp_list.clear();
            let mut i = 0usize;

            while i < list.len() {
                let node = list[i];
                let removal_work = list
                    .len()
                    .checked_sub(i)
                    .and_then(|units| units.checked_add(self.nodes[node].edges.len()))
                    .ok_or(WorkFailure::ArithmeticOverflow)?;
                admit_dynamic_work(work_control, removal_work)?;
                list.remove(i);

                self.for_each_active_neighbor(node, |neighbour| {
                    match {
                        ::core::primitive::bool::then_some(removed[neighbour], ())
                    } {
                        ::core::option::Option::Some(()) => {
                            return;
                        },
                        ::core::option::Option::None => {},
                    }

                    let other_degree = remaining_degrees[neighbour];
                    let new_degree = other_degree.saturating_sub(1);

                    match new_degree {
                        1 => {
                            temp_list.push(neighbour);
                        },
                        _ => {},
                    }

                    remaining_degrees[neighbour] = new_degree;
                });

                i += 1;
            }

            admit_dynamic_work(work_control, temp_list.len())?;

            for &v in &temp_list {
                removed[v] = true;
            }

            match {
                ::core::primitive::bool::then_some(list.len() == 1 || list.len() == 2, ())
            } {
                ::core::option::Option::Some(()) => {
                    found_center = true;
                    center_node = list[0];
                },
                ::core::option::Option::None => {},
            }
        }

        Ok(center_node)
    }

    #[must_use]
    fn max_diagonal_in_tree(&self, tree: &[usize]) -> f64 {
        let mut max_diag = f64::NEG_INFINITY;

        for &idx in tree {
            max_diag = max_diag.max(self.nodes[idx].diagonal());
        }

        match {
            ::core::primitive::bool::then_some(!max_diag.is_finite(), ())
        } {
            ::core::option::Option::Some(()) => { 0.0 },
            ::core::option::Option::None => { max_diag },
        }
    }

    fn branch_radial_layout<W: WorkControl + ?Sized>(
        &mut self,
        root: RadialBranchFrame,
        radial_separation: f64,
        work_control: &mut W,
    ) -> std::result::Result<(), WorkFailure> {
        let mut stack = vec![root];

        while let Some(frame) = stack.pop() {
            let branch_work = self.nodes[frame.node]
                .edges
                .len()
                .checked_mul(2)
                .and_then(|units| units.checked_add(1))
                .ok_or(WorkFailure::ArithmeticOverflow)?;
            admit_dynamic_work(work_control, branch_work)?;
            // First, position this node by finding its angle.
            let mut half_interval = ((frame.end_angle - frame.start_angle) + 1.0) / 2.0;

            match {
                ::core::primitive::bool::then_some(half_interval < 0.0, ())
            } {
                ::core::option::Option::Some(()) => {
                    half_interval += 180.0;
                },
                ::core::option::Option::None => {},
            }

            let node_angle = (half_interval + frame.start_angle).rem_euclid(360.0);
            let teta = (node_angle * std::f64::consts::TAU) / 360.0;
            // Host libm implementations can differ by a few ULPs. The iterative layout amplifies
            // those differences, so keep its radial seed on one pure Rust implementation.
            let x_ = frame.distance * libm::cos(teta);
            let y_ = frame.distance * libm::sin(teta);
            self.nodes[frame.node].set_center(x_, y_);

            let neighbor_edges: Vec<usize> = self.nodes[frame.node].edges.clone();
            let inc_edges_count = neighbor_edges.len();
            let edge_to_parent = frame
                .parent
                .and_then(|parent| self.active_edge_between(frame.node, parent));
            let mut child_count = inc_edges_count;

            match edge_to_parent {
                ::core::option::Option::Some(_) => {
                    child_count = child_count.saturating_sub(1);
                },
                ::core::option::Option::None => {},
            }

            let start_index =
                if let Some(parent_edge) = edge_to_parent.filter(|_| inc_edges_count > 0) {
                    ((match neighbor_edges
                        .iter()
                        .position(|&edge| edge == parent_edge) {
                        ::core::option::Option::Some(option_value) => option_value,
                        ::core::option::Option::None => 0,
                    })
                        + 1)
                        % inc_edges_count
                } else {
                    0
                };

            let step_angle = match child_count {
                0 => {
                    0.0
                },
                _ => {
                    (frame.end_angle - frame.start_angle).abs() / (child_count as f64)
                },
            };

            match {
                ::core::primitive::bool::then_some(child_count == 0 || inc_edges_count == 0, ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            let mut child_frames = Vec::with_capacity(child_count);
            let mut branch_count = 0usize;
            let mut i = start_index;

            'inner_while_2: while branch_count != child_count {
                let current_neighbor = self.edge_other_end(neighbor_edges[i], frame.node);

                match {
                    ::core::primitive::bool::then_some(Some(current_neighbor) == frame.parent, ())
                } {
                    ::core::option::Option::Some(()) => {
                        i = (i + 1) % inc_edges_count;
                        continue 'inner_while_2;
                    },
                    ::core::option::Option::None => {},
                }

                let child_start_angle =
                    (frame.start_angle + (branch_count as f64) * step_angle).rem_euclid(360.0);
                let child_end_angle = (child_start_angle + step_angle).rem_euclid(360.0);
                child_frames.push(RadialBranchFrame {
                    node: current_neighbor,
                    parent: Some(frame.node),
                    start_angle: child_start_angle,
                    end_angle: child_end_angle,
                    distance: frame.distance + radial_separation,
                });

                branch_count += 1;
                i = (i + 1) % inc_edges_count;
            }

            for child_frame in child_frames.into_iter().rev() {
                stack.push(child_frame);
            }
        }

        Ok(())
    }

    fn radial_layout<W: WorkControl + ?Sized>(
        &mut self,
        tree: &[usize],
        center_node: usize,
        starting_x: f64,
        starting_y: f64,
        work_control: &mut W,
    ) -> std::result::Result<(f64, f64), WorkFailure> {
        admit_dynamic_work(
            work_control,
            tree.len()
                .checked_mul(3)
                .ok_or(WorkFailure::ArithmeticOverflow)?,
        )?;
        let radial_sep = self
            .max_diagonal_in_tree(tree)
            .max(Self::DEFAULT_RADIAL_SEPARATION);

        self.branch_radial_layout(
            RadialBranchFrame {
                node: center_node,
                parent: None,
                start_angle: 0.0,
                end_angle: 359.0,
                distance: 0.0,
            },
            radial_sep,
            work_control,
        )?;

        let bounds = {
            let optional_bounds = Bounds::from_nodes(&self.nodes, tree);

            match optional_bounds {
                Some(matched_bounds) => matched_bounds,
                ::core::option::Option::None => return Ok((starting_x, starting_y)),
            }
        };

        // `Transform` with extents 1.0 is a pure translation: worldOrg + (x - deviceOrg).
        let dx = starting_x - bounds.min_x;
        let dy = starting_y - bounds.min_y;

        for &idx in tree {
            self.nodes[idx].left += dx;
            self.nodes[idx].top += dy;
        }

        Ok((bounds.max_x + dx, bounds.max_y + dy))
    }

    fn position_nodes_radially<W: WorkControl + ?Sized>(
        &mut self,
        forest: &[Vec<usize>],
        work_control: &mut W,
    ) -> std::result::Result<(), WorkFailure> {
        // Tile the trees to a grid row by row; first tree starts at (0,0).
        let number_of_columns = (forest.len() as f64).sqrt().ceil().max(1.0) as usize;
        let mut height = 0.0;
        let mut current_y = 0.0;
        let mut current_x = 0.0;
        let mut point = (0.0, 0.0);

        for (i, tree) in forest.iter().enumerate() {
            match {
                ::core::primitive::bool::then_some(i % number_of_columns == 0, ())
            } {
                ::core::option::Option::Some(()) => {
                    current_x = 0.0;
                    current_y = height;

                    match i {
                        0 => {},
                        _ => {
                            current_y += Self::DEFAULT_COMPONENT_SEPERATION;
                        },
                    }

                    height = 0.0;
                },
                ::core::option::Option::None => {},
            }

            let center_node = self.find_center_of_tree(tree, work_control)?;
            point = self.radial_layout(tree, center_node, current_x, current_y, work_control)?;

            match {
                ::core::primitive::bool::then_some(point.1 > height, ())
            } {
                ::core::option::Option::Some(()) => {
                    height = point.1.floor();
                },
                ::core::option::Option::None => {},
            }

            current_x = (point.0 + Self::DEFAULT_COMPONENT_SEPERATION).floor();
        }

        // Match upstream `positionNodesRadially` final world-centering pass:
        // `this.transform(new PointD(WORLD_CENTER_X - point.x/2, WORLD_CENTER_Y - point.y/2))`
        // (layout-base). This is *not* equivalent to adding `(WORLD_CENTER - point/2)` directly:
        // `Layout.transform(...)` also subtracts the current root graph left/top (with margins),
        // and those exact floating-point cancellations affect downstream `===` checks.
        let world_org_x = Self::WORLD_CENTER_X - point.0 / 2.0;
        let world_org_y = Self::WORLD_CENTER_Y - point.1 / 2.0;

        admit_dynamic_work(
            work_control,
            self.nodes
                .len()
                .checked_mul(2)
                .ok_or(WorkFailure::ArithmeticOverflow)?,
        )?;
        let mut min_left = f64::INFINITY;
        let mut min_top = f64::INFINITY;

        for n in &self.nodes {
            match {
                ::core::primitive::bool::then_some(!n.active, ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            min_left = min_left.min(n.left);
            min_top = min_top.min(n.top);
        }

        match {
            ::core::primitive::bool::then_some(min_left.is_finite() && min_top.is_finite(), ())
        } {
            ::core::option::Option::Some(()) => {
                let device_org_x = min_left - Self::DEFAULT_GRAPH_MARGIN;
                let device_org_y = min_top - Self::DEFAULT_GRAPH_MARGIN;
                let dx = world_org_x - device_org_x;
                let dy = world_org_y - device_org_y;

                for n in &mut self.nodes {
                    n.move_by(dx, dy);
                }
            },
            ::core::option::Option::None => {},
        }

        Ok(())
    }

    fn run_spring_embedder<W: WorkControl + ?Sized>(
        &mut self,
        work_control: &mut W,
    ) -> std::result::Result<(), WorkFailure> {
        match {
            ::core::primitive::bool::then_some(self.nodes.is_empty(), ())
        } {
            ::core::option::Option::Some(()) => {
                return Ok(());
            },
            ::core::option::Option::None => {},
        }

        // Mermaid's Cytoscape COSE-Bilkent applies gravitational forces only when the graph is
        // disconnected (`calculateNodesToApplyGravitationTo()` collects nodes from non-connected
        // graphs). For a connected mindmap tree this list is empty, so gravity is a no-op.
        let nodes_with_gravity = self.nodes_to_apply_gravitation(work_control)?;

        fn nodes2_mut(nodes: &mut [SimNode], a: usize, b: usize) -> (&mut SimNode, &mut SimNode) {
            debug_assert!(a != b);

            match { let _explicit_ordering = ::core::cmp::Ord::cmp(&::core::convert::identity(a), &::core::convert::identity(b)); _explicit_ordering } {
                ::core::cmp::Ordering::Less => {
                    let (left, right) = nodes.split_at_mut(b);
                    (&mut left[a], &mut right[0])
                },
                ::core::cmp::Ordering::Equal | ::core::cmp::Ordering::Greater => {
                    let (left, right) = nodes.split_at_mut(a);
                    (&mut right[0], &mut left[b])
                },
            }
        }

        // These are instance fields in upstream `FDLayout`/`CoSELayout`.
        let ideal_edge_length = Self::DEFAULT_EDGE_LENGTH.max(10.0);
        let spring_constant = Self::DEFAULT_SPRING_STRENGTH;
        let repulsion_constant = Self::DEFAULT_REPULSION_STRENGTH;
        let gravity_constant = Self::DEFAULT_GRAVITY_STRENGTH;
        let gravity_range_factor = Self::DEFAULT_GRAVITY_RANGE_FACTOR;
        let repulsion_range = 2.0 * ideal_edge_length;

        let active_node_count = self.nodes.iter().filter(|n| n.active).count().max(1);
        let active_n = active_node_count as f64;
        let displacement_threshold_per_node = (3.0 * Self::DEFAULT_EDGE_LENGTH) / 100.0;
        let total_displacement_threshold = displacement_threshold_per_node * active_n;

        // Non-incremental mode: coolingFactor starts at 1.0 for small graphs.
        let initial_cooling_factor = 1.0;
        let mut cooling_factor = initial_cooling_factor;
        let max_iterations = Self::MAX_ITERATIONS.max(
            active_node_count
                .checked_mul(5)
                .ok_or(WorkFailure::ArithmeticOverflow)?,
        );
        let max_cooling_cycle = (max_iterations as f64) / (Self::CONVERGENCE_CHECK_PERIOD as f64);
        let final_temperature = (Self::CONVERGENCE_CHECK_PERIOD as f64) / (max_iterations as f64);
        let mut cooling_cycle = 0.0f64;
        // Mermaid (via `rendering-util/layout-algorithms/cose-bilkent/cytoscape-setup.ts`) uses
        // `quality: 'proof'` for COSE-Bilkent.
        let layout_quality = 2i32;

        let mut total_iterations = 0usize;
        let mut old_total_displacement = 0.0f64;
        let mut last_total_displacement = 0.0f64;

        admit_dynamic_work(
            work_control,
            self.nodes
                .len()
                .checked_mul(2)
                .ok_or(WorkFailure::ArithmeticOverflow)?,
        )?;
        let mut processed_repulsion: Vec<bool> = vec![false; self.nodes.len()];
        let mut cell_candidates = Vec::new();
        cell_candidates
            .try_reserve_exact(self.nodes.len())
            .map_err(|_| WorkFailure::ArithmeticOverflow)?;

        loop {
            total_iterations += 1;

            match {
                ::core::primitive::bool::then_some(total_iterations == max_iterations, ())
            } {
                ::core::option::Option::Some(()) => {
                    break;
                },
                ::core::option::Option::None => {},
            }

            match {
                ::core::primitive::bool::then_some(total_iterations.is_multiple_of(Self::CONVERGENCE_CHECK_PERIOD), ())
            } {
                ::core::option::Option::Some(()) => {
                    let oscilating = total_iterations > (max_iterations / 3)
                        && (last_total_displacement - old_total_displacement).abs() < 2.0;
                    let converged = last_total_displacement < total_displacement_threshold;

                    old_total_displacement = last_total_displacement;

                    match {
                        ::core::primitive::bool::then_some(converged || oscilating, ())
                    } {
                        ::core::option::Option::Some(()) => {
                            break;
                        },
                        ::core::option::Option::None => {},
                    }

                    cooling_cycle += 1.0;

                    // cooling schedule 3 (see upstream comment in `CoSELayout.tick`)
                    let numerator = (100.0 * (initial_cooling_factor - final_temperature)).ln();
                    let denominator = max_cooling_cycle.ln().max(1e-9);
                    let power = numerator / denominator;
                    let cooling_adjuster = match layout_quality {
                        0 => cooling_cycle,
                        1 => cooling_cycle / 3.0,
                        _ => 1.0,
                    };
                    let schedule = cooling_cycle.powf(power) / 100.0 * cooling_adjuster;
                    cooling_factor = (initial_cooling_factor - schedule).max(final_temperature);
                },
                ::core::option::Option::None => {},
            }

            let iteration_work = self
                .nodes
                .len()
                .checked_mul(match {
                    ::core::primitive::bool::then_some(nodes_with_gravity.is_empty(), ())
                } {
                    ::core::option::Option::Some(()) => { 2 },
                    ::core::option::Option::None => { 3 },
                })
                .and_then(|units| units.checked_add(self.edges.len()))
                .and_then(|units| units.checked_add(nodes_with_gravity.len()))
                .ok_or(WorkFailure::ArithmeticOverflow)?;
            admit_dynamic_work(work_control, iteration_work)?;
            let mut total_displacement = 0.0f64;

            // Spring forces
            'over_e_2: for e in &self.edges {
                match {
                    ::core::primitive::bool::then_some(!e.active, ())
                } {
                    ::core::option::Option::Some(()) => {
                        continue 'over_e_2;
                    },
                    ::core::option::Option::None => {},
                }

                let (a, b) = (e.a, e.b);

                match {
                    ::core::primitive::bool::then_some(!(self.nodes[a].active && self.nodes[b].active), ())
                } {
                    ::core::option::Option::Some(()) => {
                        continue 'over_e_2;
                    },
                    ::core::option::Option::None => {},
                }

                // Upstream `FDLayout.calcSpringForce` uses clipping points on the node rectangles
                // (via `IGeometry.getIntersection`) so the "ideal edge length" applies between
                // node borders rather than between node centers.
                let (target_x, target_y, source_x, source_y, overlapped) =
                    rect_intersection_points(&self.nodes[b], &self.nodes[a]);

                match ::core::primitive::bool::then_some(overlapped, ()) {
                    ::core::option::Option::Some(()) => {
                        continue 'over_e_2;
                    },
                    ::core::option::Option::None => {},
                }

                let mut lx = target_x - source_x;
                let mut ly = target_y - source_y;

                // Mirror `LEdge.updateLength(...)` from `layout-base`: very small components are
                // snapped to their sign (or 0 if the component is 0).
                match {
                    ::core::primitive::bool::then_some(lx.abs() < 1.0, ())
                } {
                    ::core::option::Option::Some(()) => {
                        lx = Self::imath_sign(lx);
                    },
                    ::core::option::Option::None => {},
                }

                match {
                    ::core::primitive::bool::then_some(ly.abs() < 1.0, ())
                } {
                    ::core::option::Option::Some(()) => {
                        ly = Self::imath_sign(ly);
                    },
                    ::core::option::Option::None => {},
                }

                let len = (lx * lx + ly * ly).sqrt();

                match {
                    ::core::primitive::bool::then_some(len == 0.0, ())
                } {
                    ::core::option::Option::Some(()) => {
                        continue 'over_e_2;
                    },
                    ::core::option::Option::None => {},
                }

                let spring_force = spring_constant * (len - ideal_edge_length);
                let sfx = spring_force * (lx / len);
                let sfy = spring_force * (ly / len);
                let (na, nb) = nodes2_mut(&mut self.nodes, a, b);
                na.spring_fx += sfx;
                na.spring_fy += sfy;
                nb.spring_fx -= sfx;
                nb.spring_fy -= sfy;
            }

            // Repulsion forces (FR-grid variant).
            //
            // Mirrors `FDLayout.calcRepulsionForces` + `calculateRepulsionForceOfANode`:
            // - rebuild the grid every `GRID_CALCULATION_CHECK_PERIOD` iterations (when allowed)
            // - cache `node.surrounding` between grid rebuilds
            // - candidate filtering uses *border distances* against `repulsionRange`
            let rebuild_surrounding = total_iterations % Self::GRID_CALCULATION_CHECK_PERIOD == 1;

            match ::core::primitive::bool::then_some(rebuild_surrounding, ()) {
                ::core::option::Option::Some(()) => {
                    self.update_grid(repulsion_range, work_control)?;
                },
                ::core::option::Option::None => {},
            }

            processed_repulsion.fill(false);

            match {
                ::core::primitive::bool::then_some(!self.grid.is_empty(), ())
            } {
                ::core::option::Option::Some(()) => {
                    let size_x_i32 = self.grid.size_x().min(i32::MAX as usize) as i32;
                    let size_y_i32 = self.grid.size_y().min(i32::MAX as usize) as i32;

                    match ::core::primitive::bool::then_some(rebuild_surrounding, ()) {
                        ::core::option::Option::Some(()) => {
                            let mut refresh_work = 0usize;

                            for node in self.nodes.iter().filter(|node| node.active) {
                                let gx0 = (node.start_x - 1).max(0) as usize;
                                let gy0 = (node.start_y - 1).max(0) as usize;
                                let gx1 = (node.finish_x + 1).min(size_x_i32.saturating_sub(1)) as usize;
                                let gy1 = (node.finish_y + 1).min(size_y_i32.saturating_sub(1)) as usize;
                                let scan_cell_count = gx1
                                    .checked_sub(gx0)
                                    .and_then(|width| width.checked_add(1))
                                    .and_then(|width| {
                                        gy1.checked_sub(gy0)
                                            .and_then(|height| height.checked_add(1))
                                            .and_then(|height| width.checked_mul(height))
                                    })
                                    .ok_or(WorkFailure::ArithmeticOverflow)?;
                                refresh_work = refresh_work
                                    .checked_add(scan_cell_count)
                                    .ok_or(WorkFailure::ArithmeticOverflow)?;

                                for gx in gx0..=gx1 {
                                    for gy in gy0..=gy1 {
                                        refresh_work = refresh_work
                                            .checked_add(self.grid.cell_scan_work(gx, gy))
                                            .ok_or(WorkFailure::ArithmeticOverflow)?;
                                    }
                                }
                            }

                            admit_dynamic_work(work_control, refresh_work)?;

                            // Build all cached surrounding lists before applying forces. The source
                            // filters candidates with `processedNodeSet`, but forces do not move nodes
                            // until the later move phase, so this preserves both candidate order and
                            // floating-point accumulation order while reducing control callbacks.
                            'over_a_2: for a in 0..self.nodes.len() {
                                match {
                                    ::core::primitive::bool::then_some(!self.nodes[a].active, ())
                                } {
                                    ::core::option::Option::Some(()) => {
                                        continue 'over_a_2;
                                    },
                                    ::core::option::Option::None => {},
                                }

                                self.nodes[a].surrounding.clear();
                                self.repulsion_seen_gen = self.repulsion_seen_gen.wrapping_add(1);

                                match self.repulsion_seen_gen {
                                    0 => {
                                        self.repulsion_seen.fill(0);
                                        self.repulsion_seen_gen = 1;
                                    },
                                    _ => {},
                                }

                                let seen_gen = self.repulsion_seen_gen;
                                let ni = &self.nodes[a];
                                let gx0 = (ni.start_x - 1).max(0) as usize;
                                let gy0 = (ni.start_y - 1).max(0) as usize;
                                let gx1 = (ni.finish_x + 1).min(size_x_i32.saturating_sub(1)) as usize;
                                let gy1 = (ni.finish_y + 1).min(size_y_i32.saturating_sub(1)) as usize;

                                for gx in gx0..=gx1 {
                                    for gy in gy0..=gy1 {
                                        self.grid.fill_cell_candidates(
                                            &self.nodes,
                                            gx,
                                            gy,
                                            &mut cell_candidates,
                                        );

                                        'inner_for_5: for &b in &cell_candidates {
                                            match {
                                                ::core::primitive::bool::then_some(processed_repulsion[b]
                                                || b == a
                                                || !self.nodes[b].active
                                                || self.repulsion_seen[b] == seen_gen, ())
                                            } {
                                                ::core::option::Option::Some(()) => {
                                                    continue 'inner_for_5;
                                                },
                                                ::core::option::Option::None => {},
                                            }

                                            let na = &self.nodes[a];
                                            let nb = &self.nodes[b];
                                            let dist_x = (na.center_x() - nb.center_x()).abs()
                                                - (na.half_w() + nb.half_w());
                                            let dist_y = (na.center_y() - nb.center_y()).abs()
                                                - (na.half_h() + nb.half_h());

                                            match {
                                                ::core::primitive::bool::then_some(dist_x <= repulsion_range && dist_y <= repulsion_range, ())
                                            } {
                                                ::core::option::Option::Some(()) => {
                                                    self.repulsion_seen[b] = seen_gen;
                                                    self.nodes[a].surrounding.push(b);
                                                },
                                                ::core::option::Option::None => {},
                                            }
                                        }
                                    }
                                }

                                processed_repulsion[a] = true;
                            }
                        },
                        ::core::option::Option::None => {},
                    }

                    let surrounding_work = self.nodes.iter().try_fold(0usize, |work, node| {
                        work.checked_add(node.surrounding.len())
                            .ok_or(WorkFailure::ArithmeticOverflow)
                    })?;
                    admit_dynamic_work(work_control, surrounding_work)?;

                    'over_a_2: for a in 0..self.nodes.len() {
                        match {
                            ::core::primitive::bool::then_some(!self.nodes[a].active, ())
                        } {
                            ::core::option::Option::Some(()) => {
                                continue 'over_a_2;
                            },
                            ::core::option::Option::None => {},
                        }

                        let surrounding = std::mem::take(&mut self.nodes[a].surrounding);

                        for &b in &surrounding {
                            let (rfx, rfy) = self.calc_repulsion_force(a, b, repulsion_constant);
                            let (na, nb) = nodes2_mut(&mut self.nodes, a, b);
                            na.repulsion_fx += rfx;
                            na.repulsion_fy += rfy;
                            nb.repulsion_fx -= rfx;
                            nb.repulsion_fy -= rfy;
                        }

                        self.nodes[a].surrounding = surrounding;
                    }
                },
                ::core::option::Option::None => {},
            }

            // Gravitation (only for disconnected graphs).
            if !nodes_with_gravity.is_empty()
                && let Some((owner_center_x, owner_center_y, estimated_size)) =
                    self.gravitation_context(gravity_range_factor)
            {
                'inner_for_2: for &idx in &nodes_with_gravity {
                    let n = &mut self.nodes[idx];

                    match {
                        ::core::primitive::bool::then_some(!n.active, ())
                    } {
                        ::core::option::Option::Some(()) => {
                            continue 'inner_for_2;
                        },
                        ::core::option::Option::None => {},
                    }

                    let distance_x = n.center_x() - owner_center_x;
                    let distance_y = n.center_y() - owner_center_y;
                    let abs_distance_x = distance_x.abs() + n.width / 2.0;
                    let abs_distance_y = distance_y.abs() + n.height / 2.0;

                    match {
                        ::core::primitive::bool::then_some(abs_distance_x > estimated_size || abs_distance_y > estimated_size, ())
                    } {
                        ::core::option::Option::Some(()) => {
                            n.gravitation_fx = -gravity_constant * distance_x;
                            n.gravitation_fy = -gravity_constant * distance_y;
                        },
                        ::core::option::Option::None => {},
                    }
                }
            }

            // Move nodes
            'over_n_2: for n in &mut self.nodes {
                match {
                    ::core::primitive::bool::then_some(!n.active, ())
                } {
                    ::core::option::Option::Some(()) => {
                        continue 'over_n_2;
                    },
                    ::core::option::Option::None => {},
                }

                let dx = cooling_factor * (n.spring_fx + n.repulsion_fx + n.gravitation_fx);
                let dy = cooling_factor * (n.spring_fy + n.repulsion_fy + n.gravitation_fy);

                let mut mdx = dx;
                let mut mdy = dy;
                let max_d = cooling_factor * Self::MAX_NODE_DISPLACEMENT;

                match {
                    ::core::primitive::bool::then_some(mdx.abs() > max_d, ())
                } {
                    ::core::option::Option::Some(()) => {
                        mdx = max_d * Self::imath_sign(mdx);
                    },
                    ::core::option::Option::None => {},
                }

                match {
                    ::core::primitive::bool::then_some(mdy.abs() > max_d, ())
                } {
                    ::core::option::Option::Some(()) => {
                        mdy = max_d * Self::imath_sign(mdy);
                    },
                    ::core::option::Option::None => {},
                }

                n.move_by(mdx, mdy);
                total_displacement += mdx.abs() + mdy.abs();

                // Reset forces
                n.spring_fx = 0.0;
                n.spring_fy = 0.0;
                n.repulsion_fx = 0.0;
                n.repulsion_fy = 0.0;
                n.gravitation_fx = 0.0;
                n.gravitation_fy = 0.0;
            }

            last_total_displacement = total_displacement;
        }

        Ok(())
    }

    #[cfg(test)]
    fn run_single_spring_tick_flat_graph(&mut self) {
        if self.nodes.is_empty() {
            return;
        }

        fn nodes2_mut(nodes: &mut [SimNode], a: usize, b: usize) -> (&mut SimNode, &mut SimNode) {
            debug_assert!(a != b);
            if a < b {
                let (left, right) = nodes.split_at_mut(b);
                (&mut left[a], &mut right[0])
            } else {
                let (left, right) = nodes.split_at_mut(a);
                (&mut right[0], &mut left[b])
            }
        }

        let ideal_edge_length = Self::DEFAULT_EDGE_LENGTH.max(10.0);
        let spring_constant = Self::DEFAULT_SPRING_STRENGTH;
        let repulsion_constant = Self::DEFAULT_REPULSION_STRENGTH;
        let repulsion_range = 2.0 * ideal_edge_length;

        // Tick #1 in upstream always triggers a grid rebuild (`totalIterations % 10 == 1`).
        let mut work_control = NoopWorkControl;
        self.update_grid(repulsion_range, &mut work_control)
            .expect("unbounded test grid update");

        // Spring forces.
        let mut spring_debug: Vec<(usize, usize, f64, f64, f64, f64)> = Vec::new(); // (a,b,lx,ly,len,sfy)
        for e in &self.edges {
            if !e.active {
                continue;
            }
            let (a, b) = (e.a, e.b);
            if !(self.nodes[a].active && self.nodes[b].active) {
                continue;
            }
            let (target_x, target_y, source_x, source_y, overlapped) =
                rect_intersection_points(&self.nodes[b], &self.nodes[a]);
            if overlapped {
                continue;
            }
            let mut lx = target_x - source_x;
            let mut ly = target_y - source_y;
            if lx.abs() < 1.0 {
                lx = Self::imath_sign(lx);
            }
            if ly.abs() < 1.0 {
                ly = Self::imath_sign(ly);
            }
            let len = (lx * lx + ly * ly).sqrt();
            if len == 0.0 {
                continue;
            }
            let spring_force = spring_constant * (len - ideal_edge_length);
            let sfx = spring_force * (lx / len);
            let sfy = spring_force * (ly / len);
            spring_debug.push((a, b, lx, ly, len, sfy));
            let (na, nb) = nodes2_mut(&mut self.nodes, a, b);
            na.spring_fx += sfx;
            na.spring_fy += sfy;
            nb.spring_fx -= sfx;
            nb.spring_fy -= sfy;
        }

        // Repulsion forces (FR-grid, tick #1: rebuild surrounding).
        let mut processed_repulsion: Vec<bool> = vec![false; self.nodes.len()];
        processed_repulsion.fill(false);
        let mut cell_candidates = Vec::with_capacity(self.nodes.len());
        if !self.grid.is_empty() {
            let size_x_i32 = self.grid.size_x().min(i32::MAX as usize) as i32;
            let size_y_i32 = self.grid.size_y().min(i32::MAX as usize) as i32;

            for a in 0..self.nodes.len() {
                if !self.nodes[a].active {
                    continue;
                }

                self.nodes[a].surrounding.clear();

                self.repulsion_seen_gen = self.repulsion_seen_gen.wrapping_add(1);
                if self.repulsion_seen_gen == 0 {
                    self.repulsion_seen.fill(0);
                    self.repulsion_seen_gen = 1;
                }
                let seen_gen = self.repulsion_seen_gen;

                let ni = &self.nodes[a];
                let gx0 = (ni.start_x - 1).max(0) as usize;
                let gy0 = (ni.start_y - 1).max(0) as usize;
                let gx1 = (ni.finish_x + 1).min(size_x_i32.saturating_sub(1)) as usize;
                let gy1 = (ni.finish_y + 1).min(size_y_i32.saturating_sub(1)) as usize;

                for gx in gx0..=gx1 {
                    for gy in gy0..=gy1 {
                        self.grid
                            .fill_cell_candidates(&self.nodes, gx, gy, &mut cell_candidates);
                        for &b in &cell_candidates {
                            if processed_repulsion[b] {
                                continue;
                            }
                            if b == a || !self.nodes[b].active {
                                continue;
                            }
                            if self.repulsion_seen[b] == seen_gen {
                                continue;
                            }

                            let na = &self.nodes[a];
                            let nb = &self.nodes[b];
                            let dist_x =
                                (na.center_x() - nb.center_x()).abs() - (na.half_w() + nb.half_w());
                            let dist_y =
                                (na.center_y() - nb.center_y()).abs() - (na.half_h() + nb.half_h());
                            if dist_x <= repulsion_range && dist_y <= repulsion_range {
                                self.repulsion_seen[b] = seen_gen;
                                self.nodes[a].surrounding.push(b);
                            }
                        }
                    }
                }

                let surrounding = self.nodes[a].surrounding.clone();
                for b in surrounding {
                    let (rfx, rfy) = self.calc_repulsion_force(a, b, repulsion_constant);
                    let (na, nb) = nodes2_mut(&mut self.nodes, a, b);
                    na.repulsion_fx += rfx;
                    na.repulsion_fy += rfy;
                    nb.repulsion_fx -= rfx;
                    nb.repulsion_fy -= rfy;
                }

                processed_repulsion[a] = true;
            }
        }

        // For horizontal arrangements, y-forces should remain exactly zero.
        for (i, n) in self.nodes.iter().enumerate() {
            if !(n.spring_fy == 0.0 && n.repulsion_fy == 0.0 && n.gravitation_fy == 0.0) {
                panic!(
                    "unexpected y force before move: node[{i}] spring_fy={} repulsion_fy={} gravitation_fy={} spring_debug={:?}",
                    n.spring_fy, n.repulsion_fy, n.gravitation_fy, spring_debug
                );
            }
        }

        // Move nodes (coolingFactor=1.0 on tick #1 for small, non-incremental graphs).
        let cooling_factor = 1.0;
        for n in &mut self.nodes {
            if !n.active {
                continue;
            }
            let dx = cooling_factor * (n.spring_fx + n.repulsion_fx + n.gravitation_fx);
            let dy = cooling_factor * (n.spring_fy + n.repulsion_fy + n.gravitation_fy);

            let mut mdx = dx;
            let mut mdy = dy;
            let max_d = cooling_factor * Self::MAX_NODE_DISPLACEMENT;
            if mdx.abs() > max_d {
                mdx = max_d * Self::imath_sign(mdx);
            }
            if mdy.abs() > max_d {
                mdy = max_d * Self::imath_sign(mdy);
            }

            n.move_by(mdx, mdy);

            n.spring_fx = 0.0;
            n.spring_fy = 0.0;
            n.repulsion_fx = 0.0;
            n.repulsion_fy = 0.0;
            n.gravitation_fx = 0.0;
            n.gravitation_fy = 0.0;
        }
    }

    #[must_use]
    fn calc_repulsion_force(&self, a: usize, b: usize, repulsion_constant: f64) -> (f64, f64) {
        let na = &self.nodes[a];
        let nb = &self.nodes[b];

        match {
            ::core::primitive::bool::then_some(rects_intersect(na, nb), ())
        } {
            ::core::option::Option::Some(()) => {
                let (ox, oy) = calc_separation_amount(na, nb, Self::DEFAULT_EDGE_LENGTH / 2.0);
                let repulsion_fx = 2.0 * ox;
                let repulsion_fy = 2.0 * oy;
                // `childrenConstant = 1*1/(1+1) = 0.5` for flat leaf nodes.
                (-0.5 * repulsion_fx, -0.5 * repulsion_fy)
            },
            ::core::option::Option::None => {
                // Use clipping points (approx) to account for node dimensions.
                // Avoid the redundant overlap check inside `rect_intersection_points`.
                let (ax, ay, bx, by) = rect_intersection_points_no_overlap_check(na, nb);
                let mut dx = bx - ax;
                let mut dy = by - ay;

                match {
                    ::core::primitive::bool::then_some(dx.abs() < Self::MIN_REPULSION_DIST, ())
                } {
                    ::core::option::Option::Some(()) => {
                        dx = Self::imath_sign(dx) * Self::MIN_REPULSION_DIST;
                    },
                    ::core::option::Option::None => {},
                }

                match {
                    ::core::primitive::bool::then_some(dy.abs() < Self::MIN_REPULSION_DIST, ())
                } {
                    ::core::option::Option::Some(()) => {
                        dy = Self::imath_sign(dy) * Self::MIN_REPULSION_DIST;
                    },
                    ::core::option::Option::None => {},
                }

                let dist_sq = dx * dx + dy * dy;
                let dist = dist_sq.sqrt();

                match {
                    ::core::primitive::bool::then_some(dist_sq == 0.0 || dist == 0.0, ())
                } {
                    ::core::option::Option::Some(()) => {
                        return (0.0, 0.0);
                    },
                    ::core::option::Option::None => {},
                }

                let repulsion_force = repulsion_constant / dist_sq;
                let rfx = repulsion_force * dx / dist;
                let rfy = repulsion_force * dy / dist;
                (-rfx, -rfy)
            },
        }
    }

    /// Port of `Layout.transform(newLeftTop)` for the root graph and `newLeftTop = (0,0)`.
    /// This moves the layout into a positive coordinate space with a fixed margin (15px).
    fn transform_to_origin(&mut self) {
        match {
            ::core::primitive::bool::then_some(self.nodes.is_empty(), ())
        } {
            ::core::option::Option::Some(()) => {
                return;
            },
            ::core::option::Option::None => {},
        }

        let mut min_left = f64::INFINITY;
        let mut min_top = f64::INFINITY;

        for n in &self.nodes {
            match {
                ::core::primitive::bool::then_some(!n.active, ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            min_left = min_left.min(n.left);
            min_top = min_top.min(n.top);
        }

        match {
            ::core::primitive::bool::then_some(!(min_left.is_finite() && min_top.is_finite()), ())
        } {
            ::core::option::Option::Some(()) => {
                return;
            },
            ::core::option::Option::None => {},
        }

        let left_top_x = min_left - Self::DEFAULT_GRAPH_MARGIN;
        let left_top_y = min_top - Self::DEFAULT_GRAPH_MARGIN;

        // Translate so `left_top` becomes (0,0).
        let dx = -left_top_x;
        let dy = -left_top_y;

        for n in &mut self.nodes {
            match {
                ::core::primitive::bool::then_some(!n.active, ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            n.left += dx;
            n.top += dy;
        }
    }

    fn nodes_to_apply_gravitation<W: WorkControl + ?Sized>(
        &self,
        work_control: &mut W,
    ) -> std::result::Result<Vec<usize>, WorkFailure> {
        let traversal_work = self
            .nodes
            .len()
            .checked_mul(3)
            .and_then(|units| {
                self.edges
                    .len()
                    .checked_mul(2)
                    .and_then(|edge_units| units.checked_add(edge_units))
            })
            .ok_or(WorkFailure::ArithmeticOverflow)?;
        admit_dynamic_work(work_control, traversal_work)?;
        // Port of COSE `calculateNodesToApplyGravitationTo()` for a flat graph: apply gravity to
        // all nodes only if the graph is disconnected.
        let mut first_active: Option<usize> = None;

        for (i, n) in self.nodes.iter().enumerate() {
            match {
                ::core::primitive::bool::then_some(n.active, ())
            } {
                ::core::option::Option::Some(()) => {
                    first_active = Some(i);
                    break;
                },
                ::core::option::Option::None => {},
            }
        }

        let start = match first_active {
            Some(matched_start) => matched_start,
            ::core::option::Option::None => return Ok(Vec::new()),
        };

        let mut stack: Vec<usize> = vec![start];
        let mut seen: Vec<bool> = vec![false; self.nodes.len()];
        let mut seen_count: usize = 1;
        seen[start] = true;

        while let Some(u) = stack.pop() {
            'inner_for_2: for &ei in &self.nodes[u].edges {
                match {
                    ::core::primitive::bool::then_some(!self.edges[ei].active, ())
                } {
                    ::core::option::Option::Some(()) => {
                        continue 'inner_for_2;
                    },
                    ::core::option::Option::None => {},
                }

                let v = self.edge_other_end(ei, u);

                match {
                    ::core::primitive::bool::then_some(!self.nodes[v].active, ())
                } {
                    ::core::option::Option::Some(()) => {
                        continue 'inner_for_2;
                    },
                    ::core::option::Option::None => {},
                }

                match {
                    ::core::primitive::bool::then_some(!seen[v], ())
                } {
                    ::core::option::Option::Some(()) => {
                        seen[v] = true;
                        seen_count += 1;
                        stack.push(v);
                    },
                    ::core::option::Option::None => {},
                }
            }
        }

        let active_count = self.nodes.iter().filter(|n| n.active).count();

        match {
            ::core::primitive::bool::then_some(seen_count == active_count, ())
        } {
            ::core::option::Option::Some(()) => {
                Ok(Vec::new())
            },
            ::core::option::Option::None => {
                Ok((0..self.nodes.len())
                    .filter(|&i| self.nodes[i].active)
                    .collect())
            },
        }
    }

    #[must_use]
    fn gravitation_context(&self, gravity_range_factor: f64) -> Option<(f64, f64, f64)> {
        // Port of `FDLayout.calcGravitationalForce` context:
        // - owner center = bbox center of the root graph
        // - estimatedSize = root.estimatedSize * gravityRangeFactor
        let mut min_left = f64::INFINITY;
        let mut max_right = f64::NEG_INFINITY;
        let mut min_top = f64::INFINITY;
        let mut max_bottom = f64::NEG_INFINITY;

        let mut size_sum = 0.0f64;
        let mut active_n = 0usize;

        for n in &self.nodes {
            match {
                ::core::primitive::bool::then_some(!n.active, ())
            } {
                ::core::option::Option::Some(()) => {
                    continue;
                },
                ::core::option::Option::None => {},
            }

            active_n += 1;
            min_left = min_left.min(n.left);
            max_right = max_right.max(n.right());
            min_top = min_top.min(n.top);
            max_bottom = max_bottom.max(n.bottom());
            size_sum += (n.width + n.height) / 2.0;
        }

        match {
            ::core::primitive::bool::then_some(active_n == 0
            || !(min_left.is_finite()
                && max_right.is_finite()
                && min_top.is_finite()
                && max_bottom.is_finite()), ())
        } {
            ::core::option::Option::Some(()) => {
                return None;
            },
            ::core::option::Option::None => {},
        }

        let owner_center_x = (max_right + min_left) / 2.0;
        let owner_center_y = (max_bottom + min_top) / 2.0;

        let estimated_size_base = match {
            ::core::primitive::bool::then_some(size_sum == 0.0, ())
        } {
            ::core::option::Option::Some(()) => {
                // `LayoutConstants.EMPTY_COMPOUND_NODE_SIZE`
                40.0
            },
            ::core::option::Option::None => {
                size_sum / (active_n as f64).sqrt()
            },
        };
        let estimated_size = estimated_size_base * gravity_range_factor;

        match {
            ::core::primitive::bool::then_some(!estimated_size.is_finite(), ())
        } {
            ::core::option::Option::Some(()) => {
                return None;
            },
            ::core::option::Option::None => {},
        }

        Some((owner_center_x, owner_center_y, estimated_size))
    }
}

#[must_use]
fn rects_intersect(a: &SimNode, b: &SimNode) -> bool {
    // Match `layout-base` `RectangleD.intersects` semantics:
    // - touching borders counts as intersection (uses `<`, not `<=`, in early-exit checks)
    match {
        ::core::primitive::bool::then_some(a.right() < b.left, ())
    } {
        ::core::option::Option::Some(()) => {
            return false;
        },
        ::core::option::Option::None => {},
    }

    match {
        ::core::primitive::bool::then_some(a.bottom() < b.top, ())
    } {
        ::core::option::Option::Some(()) => {
            return false;
        },
        ::core::option::Option::None => {},
    }

    match {
        ::core::primitive::bool::then_some(b.right() < a.left, ())
    } {
        ::core::option::Option::Some(()) => {
            return false;
        },
        ::core::option::Option::None => {},
    }

    match {
        ::core::primitive::bool::then_some(b.bottom() < a.top, ())
    } {
        ::core::option::Option::Some(()) => {
            return false;
        },
        ::core::option::Option::None => {},
    }

    true
}

/// Port of `layout-base` `IGeometry.getIntersection2(rectA, rectB, result)`.
///
/// Returns `(ax, ay, bx, by, overlapped)` where `(ax,ay)` is rectA's clip point and `(bx,by)` is
/// rectB's clip point on the line segment between their centers.
#[must_use]
fn rect_intersection_points(a: &SimNode, b: &SimNode) -> (f64, f64, f64, f64, bool) {
    let p1x = a.center_x();
    let p1y = a.center_y();
    let p2x = b.center_x();
    let p2y = b.center_y();

    match {
        ::core::primitive::bool::then_some(rects_intersect(a, b), ())
    } {
        ::core::option::Option::Some(()) => {
            return (p1x, p1y, p2x, p2y, true);
        },
        ::core::option::Option::None => {},
    }

    // NOTE: This intentionally mirrors the upstream `IGeometry.getIntersection2` implementation
    // from `layout-base` (including its branching structure) rather than a mathematically
    // equivalent closed-form intersection. Downstream convergence is sensitive to these
    // conditionals due to floating-point comparisons.

    // rectA corners
    let top_left_ax = a.left;
    let top_left_ay = a.top;
    let top_right_ax = a.right();
    let bottom_left_ax = a.left;
    let bottom_left_ay = a.bottom();
    let bottom_right_ax = a.right();
    let half_width_a = a.width / 2.0;
    let half_height_a = a.height / 2.0;

    // rectB corners
    let top_left_bx = b.left;
    let top_left_by = b.top;
    let top_right_bx = b.right();
    let bottom_left_bx = b.left;
    let bottom_left_by = b.bottom();
    let bottom_right_bx = b.right();
    let half_width_b = b.width / 2.0;
    let half_height_b = b.height / 2.0;

    // Line is vertical.
    match {
        ::core::primitive::bool::then_some(p1x == p2x, ())
    } {
        ::core::option::Option::Some(()) => {
            match {
                ::core::primitive::bool::then_some(p1y > p2y, ())
            } {
                ::core::option::Option::Some(()) => {
                    return (p1x, top_left_ay, p2x, bottom_left_by, false);
                },
                ::core::option::Option::None => match {
                    ::core::primitive::bool::then_some(p1y < p2y, ())
                } {
                    ::core::option::Option::Some(()) => {
                        return (p1x, bottom_left_ay, p2x, top_left_by, false);
                    },
                    ::core::option::Option::None => {
                        return (p1x, p1y, p2x, p2y, false);
                    },
                },
            }
        },
        ::core::option::Option::None => {},
    }

    // Line is horizontal.
    match {
        ::core::primitive::bool::then_some(p1y == p2y, ())
    } {
        ::core::option::Option::Some(()) => {
            match {
                ::core::primitive::bool::then_some(p1x > p2x, ())
            } {
                ::core::option::Option::Some(()) => {
                    return (top_left_ax, p1y, top_right_bx, p2y, false);
                },
                ::core::option::Option::None => match {
                    ::core::primitive::bool::then_some(p1x < p2x, ())
                } {
                    ::core::option::Option::Some(()) => {
                        return (top_right_ax, p1y, top_left_bx, p2y, false);
                    },
                    ::core::option::Option::None => {
                        return (p1x, p1y, p2x, p2y, false);
                    },
                },
            }
        },
        ::core::option::Option::None => {},
    }

    #[inline]
    #[must_use]
    fn get_cardinal_direction(slope: f64, slope_prime: f64, line: i32) -> i32 {
        match {
            ::core::primitive::bool::then_some(slope > slope_prime, ())
        } {
            ::core::option::Option::Some(()) => {
                line
            },
            ::core::option::Option::None => {
                1 + (line % 4)
            },
        }
    }

    let slope_a = a.height / a.width;
    let slope_b = b.height / b.width;
    let slope_prime = (p2y - p1y) / (p2x - p1x);

    let mut ax = 0.0;
    let mut ay = 0.0;
    let mut bx = 0.0;
    let mut by = 0.0;
    let mut clip_a_found = false;
    let mut clip_b_found = false;

    // Determine whether clipping point is the corner of rectA.
    match {
        ::core::primitive::bool::then_some(-slope_a == slope_prime, ())
    } {
        ::core::option::Option::Some(()) => {
            match {
                ::core::primitive::bool::then_some(p1x > p2x, ())
            } {
                ::core::option::Option::Some(()) => {
                    ax = bottom_left_ax;
                    ay = bottom_left_ay;
                    clip_a_found = true;
                },
                ::core::option::Option::None => {
                    ax = top_right_ax;
                    ay = top_left_ay;
                    clip_a_found = true;
                },
            }
        },
        ::core::option::Option::None => match {
            ::core::primitive::bool::then_some(slope_a == slope_prime, ())
        } {
            ::core::option::Option::Some(()) => {
                match {
                    ::core::primitive::bool::then_some(p1x > p2x, ())
                } {
                    ::core::option::Option::Some(()) => {
                        ax = top_left_ax;
                        ay = top_left_ay;
                        clip_a_found = true;
                    },
                    ::core::option::Option::None => {
                        ax = bottom_right_ax;
                        ay = bottom_left_ay;
                        clip_a_found = true;
                    },
                }
            },
            ::core::option::Option::None => {},
        },
    }

    // Determine whether clipping point is the corner of rectB.
    match {
        ::core::primitive::bool::then_some(-slope_b == slope_prime, ())
    } {
        ::core::option::Option::Some(()) => {
            match {
                ::core::primitive::bool::then_some(p2x > p1x, ())
            } {
                ::core::option::Option::Some(()) => {
                    bx = bottom_left_bx;
                    by = bottom_left_by;
                    clip_b_found = true;
                },
                ::core::option::Option::None => {
                    bx = top_right_bx;
                    by = top_left_by;
                    clip_b_found = true;
                },
            }
        },
        ::core::option::Option::None => match {
            ::core::primitive::bool::then_some(slope_b == slope_prime, ())
        } {
            ::core::option::Option::Some(()) => {
                match {
                    ::core::primitive::bool::then_some(p2x > p1x, ())
                } {
                    ::core::option::Option::Some(()) => {
                        bx = top_left_bx;
                        by = top_left_by;
                        clip_b_found = true;
                    },
                    ::core::option::Option::None => {
                        bx = bottom_right_bx;
                        by = bottom_left_by;
                        clip_b_found = true;
                    },
                }
            },
            ::core::option::Option::None => {},
        },
    }

    match {
        ::core::primitive::bool::then_some(clip_a_found && clip_b_found, ())
    } {
        ::core::option::Option::Some(()) => {
            return (ax, ay, bx, by, false);
        },
        ::core::option::Option::None => {},
    }

    let (card_a, card_b) = match {
        ::core::primitive::bool::then_some(p1x > p2x, ())
    } {
        ::core::option::Option::Some(()) => {
            match {
                ::core::primitive::bool::then_some(p1y > p2y, ())
            } {
                ::core::option::Option::Some(()) => {
                    (
                        get_cardinal_direction(slope_a, slope_prime, 4),
                        get_cardinal_direction(slope_b, slope_prime, 2),
                    )
                },
                ::core::option::Option::None => {
                    (
                        get_cardinal_direction(-slope_a, slope_prime, 3),
                        get_cardinal_direction(-slope_b, slope_prime, 1),
                    )
                },
            }
        },
        ::core::option::Option::None => match {
            ::core::primitive::bool::then_some(p1y > p2y, ())
        } {
            ::core::option::Option::Some(()) => {
                (
                    get_cardinal_direction(-slope_a, slope_prime, 1),
                    get_cardinal_direction(-slope_b, slope_prime, 3),
                )
            },
            ::core::option::Option::None => {
                (
                    get_cardinal_direction(slope_a, slope_prime, 2),
                    get_cardinal_direction(slope_b, slope_prime, 4),
                )
            },
        },
    };

    match {
        ::core::primitive::bool::then_some(!clip_a_found, ())
    } {
        ::core::option::Option::Some(()) => {
            match card_a {
                1 => {
                    ay = top_left_ay;
                    ax = p1x + (-half_height_a) / slope_prime;
                }
                2 => {
                    ax = bottom_right_ax;
                    ay = p1y + half_width_a * slope_prime;
                }
                3 => {
                    ay = bottom_left_ay;
                    ax = p1x + half_height_a / slope_prime;
                }
                _ => {
                    ax = bottom_left_ax;
                    ay = p1y + (-half_width_a) * slope_prime;
                }
            }
        },
        ::core::option::Option::None => {},
    }

    match {
        ::core::primitive::bool::then_some(!clip_b_found, ())
    } {
        ::core::option::Option::Some(()) => {
            match card_b {
                1 => {
                    by = top_left_by;
                    bx = p2x + (-half_height_b) / slope_prime;
                }
                2 => {
                    bx = bottom_right_bx;
                    by = p2y + half_width_b * slope_prime;
                }
                3 => {
                    by = bottom_left_by;
                    bx = p2x + half_height_b / slope_prime;
                }
                _ => {
                    bx = bottom_left_bx;
                    by = p2y + (-half_width_b) * slope_prime;
                }
            }
        },
        ::core::option::Option::None => {},
    }

    (ax, ay, bx, by, false)
}

#[inline]
#[must_use]
fn rect_intersection_points_no_overlap_check(a: &SimNode, b: &SimNode) -> (f64, f64, f64, f64) {
    // Fast path for callers that already know `rects_intersect(a, b) == false`.
    let p1x = a.center_x();
    let p1y = a.center_y();
    let p2x = b.center_x();
    let p2y = b.center_y();

    // rectA corners
    let top_left_ax = a.left;
    let top_left_ay = a.top;
    let top_right_ax = a.right();
    let bottom_left_ax = a.left;
    let bottom_left_ay = a.bottom();
    let bottom_right_ax = a.right();
    let half_width_a = a.width / 2.0;
    let half_height_a = a.height / 2.0;

    // rectB corners
    let top_left_bx = b.left;
    let top_left_by = b.top;
    let top_right_bx = b.right();
    let bottom_left_bx = b.left;
    let bottom_left_by = b.bottom();
    let bottom_right_bx = b.right();
    let half_width_b = b.width / 2.0;
    let half_height_b = b.height / 2.0;

    match {
        ::core::primitive::bool::then_some(p1x == p2x, ())
    } {
        ::core::option::Option::Some(()) => {
            match {
                ::core::primitive::bool::then_some(p1y > p2y, ())
            } {
                ::core::option::Option::Some(()) => {
                    return (p1x, top_left_ay, p2x, bottom_left_by);
                },
                ::core::option::Option::None => match {
                    ::core::primitive::bool::then_some(p1y < p2y, ())
                } {
                    ::core::option::Option::Some(()) => {
                        return (p1x, bottom_left_ay, p2x, top_left_by);
                    },
                    ::core::option::Option::None => {
                        return (p1x, p1y, p2x, p2y);
                    },
                },
            }
        },
        ::core::option::Option::None => {},
    }

    match {
        ::core::primitive::bool::then_some(p1y == p2y, ())
    } {
        ::core::option::Option::Some(()) => {
            match {
                ::core::primitive::bool::then_some(p1x > p2x, ())
            } {
                ::core::option::Option::Some(()) => {
                    return (top_left_ax, p1y, top_right_bx, p2y);
                },
                ::core::option::Option::None => match {
                    ::core::primitive::bool::then_some(p1x < p2x, ())
                } {
                    ::core::option::Option::Some(()) => {
                        return (top_right_ax, p1y, top_left_bx, p2y);
                    },
                    ::core::option::Option::None => {
                        return (p1x, p1y, p2x, p2y);
                    },
                },
            }
        },
        ::core::option::Option::None => {},
    }

    #[inline]
    #[must_use]
    fn get_cardinal_direction(slope: f64, slope_prime: f64, line: i32) -> i32 {
        match {
            ::core::primitive::bool::then_some(slope > slope_prime, ())
        } {
            ::core::option::Option::Some(()) => {
                line
            },
            ::core::option::Option::None => {
                1 + (line % 4)
            },
        }
    }

    let slope_a = a.height / a.width;
    let slope_b = b.height / b.width;
    let slope_prime = (p2y - p1y) / (p2x - p1x);

    let mut ax = 0.0;
    let mut ay = 0.0;
    let mut bx = 0.0;
    let mut by = 0.0;
    let mut clip_a_found = false;
    let mut clip_b_found = false;

    match {
        ::core::primitive::bool::then_some(-slope_a == slope_prime, ())
    } {
        ::core::option::Option::Some(()) => {
            match {
                ::core::primitive::bool::then_some(p1x > p2x, ())
            } {
                ::core::option::Option::Some(()) => {
                    ax = bottom_left_ax;
                    ay = bottom_left_ay;
                    clip_a_found = true;
                },
                ::core::option::Option::None => {
                    ax = top_right_ax;
                    ay = top_left_ay;
                    clip_a_found = true;
                },
            }
        },
        ::core::option::Option::None => match {
            ::core::primitive::bool::then_some(slope_a == slope_prime, ())
        } {
            ::core::option::Option::Some(()) => {
                match {
                    ::core::primitive::bool::then_some(p1x > p2x, ())
                } {
                    ::core::option::Option::Some(()) => {
                        ax = top_left_ax;
                        ay = top_left_ay;
                        clip_a_found = true;
                    },
                    ::core::option::Option::None => {
                        ax = bottom_right_ax;
                        ay = bottom_left_ay;
                        clip_a_found = true;
                    },
                }
            },
            ::core::option::Option::None => {},
        },
    }

    match {
        ::core::primitive::bool::then_some(-slope_b == slope_prime, ())
    } {
        ::core::option::Option::Some(()) => {
            match {
                ::core::primitive::bool::then_some(p2x > p1x, ())
            } {
                ::core::option::Option::Some(()) => {
                    bx = bottom_left_bx;
                    by = bottom_left_by;
                    clip_b_found = true;
                },
                ::core::option::Option::None => {
                    bx = top_right_bx;
                    by = top_left_by;
                    clip_b_found = true;
                },
            }
        },
        ::core::option::Option::None => match {
            ::core::primitive::bool::then_some(slope_b == slope_prime, ())
        } {
            ::core::option::Option::Some(()) => {
                match {
                    ::core::primitive::bool::then_some(p2x > p1x, ())
                } {
                    ::core::option::Option::Some(()) => {
                        bx = top_left_bx;
                        by = top_left_by;
                        clip_b_found = true;
                    },
                    ::core::option::Option::None => {
                        bx = bottom_right_bx;
                        by = bottom_left_by;
                        clip_b_found = true;
                    },
                }
            },
            ::core::option::Option::None => {},
        },
    }

    match {
        ::core::primitive::bool::then_some(!(clip_a_found && clip_b_found), ())
    } {
        ::core::option::Option::Some(()) => {
            let (card_a, card_b) = match {
                ::core::primitive::bool::then_some(p1x > p2x, ())
            } {
                ::core::option::Option::Some(()) => {
                    match {
                        ::core::primitive::bool::then_some(p1y > p2y, ())
                    } {
                        ::core::option::Option::Some(()) => {
                            (
                                get_cardinal_direction(slope_a, slope_prime, 4),
                                get_cardinal_direction(slope_b, slope_prime, 2),
                            )
                        },
                        ::core::option::Option::None => {
                            (
                                get_cardinal_direction(-slope_a, slope_prime, 3),
                                get_cardinal_direction(-slope_b, slope_prime, 1),
                            )
                        },
                    }
                },
                ::core::option::Option::None => match {
                    ::core::primitive::bool::then_some(p1y > p2y, ())
                } {
                    ::core::option::Option::Some(()) => {
                        (
                            get_cardinal_direction(-slope_a, slope_prime, 1),
                            get_cardinal_direction(-slope_b, slope_prime, 3),
                        )
                    },
                    ::core::option::Option::None => {
                        (
                            get_cardinal_direction(slope_a, slope_prime, 2),
                            get_cardinal_direction(slope_b, slope_prime, 4),
                        )
                    },
                },
            };

            match {
                ::core::primitive::bool::then_some(!clip_a_found, ())
            } {
                ::core::option::Option::Some(()) => {
                    match card_a {
                        1 => {
                            ay = top_left_ay;
                            ax = p1x + (-half_height_a) / slope_prime;
                        }
                        2 => {
                            ax = bottom_right_ax;
                            ay = p1y + half_width_a * slope_prime;
                        }
                        3 => {
                            ay = bottom_left_ay;
                            ax = p1x + half_height_a / slope_prime;
                        }
                        _ => {
                            ax = bottom_left_ax;
                            ay = p1y + (-half_width_a) * slope_prime;
                        }
                    }
                },
                ::core::option::Option::None => {},
            }

            match {
                ::core::primitive::bool::then_some(!clip_b_found, ())
            } {
                ::core::option::Option::Some(()) => {
                    match card_b {
                        1 => {
                            by = top_left_by;
                            bx = p2x + (-half_height_b) / slope_prime;
                        }
                        2 => {
                            bx = bottom_right_bx;
                            by = p2y + half_width_b * slope_prime;
                        }
                        3 => {
                            by = bottom_left_by;
                            bx = p2x + half_height_b / slope_prime;
                        }
                        _ => {
                            bx = bottom_left_bx;
                            by = p2y + (-half_width_b) * slope_prime;
                        }
                    }
                },
                ::core::option::Option::None => {},
            }
        },
        ::core::option::Option::None => {},
    }

    (ax, ay, bx, by)
}

#[must_use]
fn calc_separation_amount(a: &SimNode, b: &SimNode, separation_buffer: f64) -> (f64, f64) {
    debug_assert!(rects_intersect(a, b));

    let (dir_x, dir_y) = decide_directions_for_overlapping_nodes(a, b);

    let mut overlap_x = a.right().min(b.right()) - a.left.max(b.left);
    let mut overlap_y = a.bottom().min(b.bottom()) - a.top.max(b.top);

    match {
        ::core::primitive::bool::then_some((a.left <= b.left) && (a.right() >= b.right()), ())
    } {
        ::core::option::Option::Some(()) => {
            overlap_x += (b.left - a.left).min(a.right() - b.right());
        },
        ::core::option::Option::None => match {
            ::core::primitive::bool::then_some((b.left <= a.left) && (b.right() >= a.right()), ())
        } {
            ::core::option::Option::Some(()) => {
                overlap_x += (a.left - b.left).min(b.right() - a.right());
            },
            ::core::option::Option::None => {},
        },
    }

    match {
        ::core::primitive::bool::then_some((a.top <= b.top) && (a.bottom() >= b.bottom()), ())
    } {
        ::core::option::Option::Some(()) => {
            overlap_y += (b.top - a.top).min(a.bottom() - b.bottom());
        },
        ::core::option::Option::None => match {
            ::core::primitive::bool::then_some((b.top <= a.top) && (b.bottom() >= a.bottom()), ())
        } {
            ::core::option::Option::Some(()) => {
                overlap_y += (a.top - b.top).min(b.bottom() - a.bottom());
            },
            ::core::option::Option::None => {},
        },
    }

    let mut slope = ((b.center_y() - a.center_y()) / (b.center_x() - a.center_x())).abs();

    match {
        ::core::primitive::bool::then_some((b.center_y() == a.center_y()) && (b.center_x() == a.center_x()), ())
    } {
        ::core::option::Option::Some(()) => {
            slope = 1.0;
        },
        ::core::option::Option::None => {},
    }

    let mut move_by_y = slope * overlap_x;
    let mut move_by_x = overlap_y / slope;

    match {
        ::core::primitive::bool::then_some(overlap_x < move_by_x, ())
    } {
        ::core::option::Option::Some(()) => {
            move_by_x = overlap_x;
        },
        ::core::option::Option::None => {
            move_by_y = overlap_y;
        },
    }

    let dx = -(dir_x as f64) * ((move_by_x / 2.0) + separation_buffer);
    let dy = -(dir_y as f64) * ((move_by_y / 2.0) + separation_buffer);
    (dx, dy)
}

#[must_use]
fn decide_directions_for_overlapping_nodes(a: &SimNode, b: &SimNode) -> (i32, i32) {
    let dir_x = match {
        ::core::primitive::bool::then_some(a.center_x() < b.center_x(), ())
    } {
        ::core::option::Option::Some(()) => { -1 },
        ::core::option::Option::None => { 1 },
    };
    let dir_y = match {
        ::core::primitive::bool::then_some(a.center_y() < b.center_y(), ())
    } {
        ::core::option::Option::Some(()) => { -1 },
        ::core::option::Option::None => { 1 },
    };
    (dir_x, dir_y)
}

#[cfg(test)]
mod tests {
    use super::{
        IndexedEdge, IndexedNode, NoopWorkControl, SimGraph, SimGrid, SimGridCells, SimGridPlan,
        SimGridStorageKind, layout, layout_indexed, layout_indexed_with_work_control,
        layout_with_work_control,
    };
    use crate::{BoundsExtras, Compound, Edge, Error, Graph, Node, WorkControl, WorkFailure};

    #[derive(Debug, Default)]
    struct RecordingWorkControl {
        limit: Option<usize>,
        used: usize,
        checks: usize,
        charges: usize,
    }

    impl RecordingWorkControl {
        fn unbounded() -> Self {
            Self::default()
        }

        fn limited(limit: usize) -> Self {
            Self {
                limit: Some(limit),
                ..Self::default()
            }
        }

        fn admit(&self, units: usize) -> std::result::Result<usize, WorkFailure> {
            let next = self
                .used
                .checked_add(units)
                .ok_or(WorkFailure::ArithmeticOverflow)?;
            if self.limit.is_some_and(|limit| next > limit) {
                return Err(WorkFailure::Interrupted);
            }
            Ok(next)
        }
    }

    impl WorkControl for RecordingWorkControl {
        fn check(&mut self, units: usize) -> std::result::Result<(), WorkFailure> {
            self.checks += 1;
            self.admit(units).map(|_| ())
        }

        fn charge(&mut self, units: usize) -> std::result::Result<(), WorkFailure> {
            self.charges += 1;
            self.used = self.admit(units)?;
            Ok(())
        }
    }

    fn basic_nodes_and_edges() -> (Vec<IndexedNode>, Vec<IndexedEdge>) {
        let nodes = vec![
            IndexedNode {
                width: 69.734375,
                height: 34.0,
                x: 0.0,
                y: 0.0,
            },
            IndexedNode {
                width: 48.40625,
                height: 34.0,
                x: 0.0,
                y: 0.0,
            },
            IndexedNode {
                width: 48.921875,
                height: 34.0,
                x: 0.0,
                y: 0.0,
            },
        ];
        let edges = vec![IndexedEdge { a: 0, b: 1 }, IndexedEdge { a: 0, b: 2 }];
        (nodes, edges)
    }

    fn mixed_size_tree_nodes_and_edges() -> (Vec<IndexedNode>, Vec<IndexedEdge>) {
        // Mirrors the range of rectangular and circular label boxes exercised by complex
        // Mindmap fixtures. The work-controlled entry point must remain bit-identical even when
        // node sizes change the radial seed positions and grid occupancy.
        let nodes = vec![
            IndexedNode {
                width: 240.0,
                height: 92.0,
                x: 0.0,
                y: 0.0,
            },
            IndexedNode {
                width: 67.203,
                height: 67.203,
                x: 0.0,
                y: 0.0,
            },
            IndexedNode {
                width: 169.922,
                height: 44.0,
                x: 0.0,
                y: 0.0,
            },
            IndexedNode {
                width: 107.203,
                height: 44.0,
                x: 0.0,
                y: 0.0,
            },
            IndexedNode {
                width: 67.203,
                height: 67.203,
                x: 0.0,
                y: 0.0,
            },
            IndexedNode {
                width: 81.125,
                height: 81.125,
                x: 0.0,
                y: 0.0,
            },
            IndexedNode {
                width: 167.844,
                height: 44.0,
                x: 0.0,
                y: 0.0,
            },
        ];
        let edges = vec![
            IndexedEdge { a: 0, b: 1 },
            IndexedEdge { a: 1, b: 2 },
            IndexedEdge { a: 1, b: 3 },
            IndexedEdge { a: 0, b: 4 },
            IndexedEdge { a: 4, b: 5 },
            IndexedEdge { a: 4, b: 6 },
        ];
        (nodes, edges)
    }

    fn assert_points_identical(actual: &[crate::Point], expected: &[crate::Point]) {
        assert_eq!(actual.len(), expected.len());
        for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            assert_eq!(
                (actual.x.to_bits(), actual.y.to_bits()),
                (expected.x.to_bits(), expected.y.to_bits()),
                "point {index} changed"
            );
        }
    }

    fn graph_from_indexed(nodes: &[IndexedNode], edges: &[IndexedEdge]) -> Graph {
        Graph {
            nodes: nodes
                .iter()
                .enumerate()
                .map(|(index, node)| Node {
                    id: format!("node-{index}"),
                    parent: None,
                    width: node.width,
                    height: node.height,
                    x: node.x,
                    y: node.y,
                    bounds_extras: BoundsExtras::default(),
                })
                .collect(),
            edges: edges
                .iter()
                .enumerate()
                .map(|(index, edge)| Edge {
                    id: format!("edge-{index}"),
                    source: format!("node-{}", edge.a),
                    target: format!("node-{}", edge.b),
                    label_width: None,
                    label_height: None,
                    source_anchor: None,
                    target_anchor: None,
                    ideal_length: 0.0,
                    elasticity: 0.0,
                })
                .collect(),
            compounds: Vec::new(),
        }
    }

    fn assert_close(a: f64, b: f64) {
        let eps = 1e-3;
        assert!((a - b).abs() <= eps, "expected {a} ~= {b} (eps={eps})");
    }

    #[test]
    fn radial_trig_matches_v8_reference_values() {
        let radial_angle = (9.475_f64 * std::f64::consts::TAU) / 360.0;
        assert_eq!(libm::sin(radial_angle).to_bits(), 0x3fc5_122d_8320_944e);
        assert_eq!(libm::cos(radial_angle).to_bits(), 0x3fef_903d_a710_dece);
    }

    #[test]
    fn basic_three_node_tree_matches_upstream_positions() {
        // Oracle: cytoscape-cose-bilkent@4.1.0 + cytoscape@3.33.3 (Mermaid 11.16.0),
        // with the same node dimensions.
        //
        // This corresponds to `fixtures/upstream-svgs/mindmap/basic.svg`.
        let (nodes, edges) = basic_nodes_and_edges();

        let out = layout_indexed(&nodes, &edges).expect("layout");

        assert_eq!(out.len(), 3);
        assert_close(out[0].x, 152.283539);
        assert_close(out[0].y, 32.0);
        assert_close(out[1].x, 264.848328);
        assert_close(out[1].y, 32.0);
        assert_close(out[2].x, 39.460938);
        assert_close(out[2].y, 32.0);
    }

    #[test]
    fn graph_and_indexed_layouts_preserve_positions_and_work_budgets() {
        let (mut cycle_nodes, _) = basic_nodes_and_edges();
        cycle_nodes[0].width = 0.0;
        cycle_nodes[1].height = -1.0;
        cycle_nodes[2].x = 100.0;
        cycle_nodes[2].y = -30.0;
        cycle_nodes.push(IndexedNode {
            width: 10.0,
            height: 10.0,
            x: -200.0,
            y: 90.0,
        });
        let cycle_edges = vec![
            IndexedEdge { a: 2, b: 0 },
            IndexedEdge { a: 0, b: 1 },
            IndexedEdge { a: 1, b: 2 },
            IndexedEdge { a: 0, b: 2 },
            IndexedEdge { a: 2, b: 0 },
            IndexedEdge { a: 1, b: 1 },
        ];

        for (nodes, edges) in [
            (Vec::new(), Vec::new()),
            mixed_size_tree_nodes_and_edges(),
            (cycle_nodes, cycle_edges),
        ] {
            let mut graph = graph_from_indexed(&nodes, &edges);
            if let Some(node) = graph.nodes.first_mut() {
                node.parent = Some("group".to_string());
                graph.compounds.push(Compound {
                    id: "group".to_string(),
                    parent: None,
                });
            }
            let mut indexed_work = RecordingWorkControl::unbounded();
            let expected = layout_indexed_with_work_control(&nodes, &edges, &mut indexed_work)
                .expect("indexed layout");
            let mut graph_work = RecordingWorkControl::unbounded();
            let actual = layout_with_work_control(&graph, &mut graph_work).expect("graph layout");
            let actual_points = graph
                .nodes
                .iter()
                .map(|node| actual.positions[&node.id])
                .collect::<Vec<_>>();
            assert_points_identical(&actual_points, &expected);
            assert_eq!(actual.positions.len(), nodes.len());
            assert_eq!(
                graph_work.used,
                indexed_work.used + nodes.len() + graph.compounds.len()
            );

            let mut exact = RecordingWorkControl::limited(graph_work.used);
            let exact_result =
                layout_with_work_control(&graph, &mut exact).expect("exact graph budget");
            let exact_points = graph
                .nodes
                .iter()
                .map(|node| exact_result.positions[&node.id])
                .collect::<Vec<_>>();
            assert_points_identical(&exact_points, &expected);
            if graph_work.used > 0 {
                let mut below = RecordingWorkControl::limited(graph_work.used - 1);
                assert!(matches!(
                    layout_with_work_control(&graph, &mut below),
                    Err(Error::WorkFailure(WorkFailure::Interrupted))
                ));
            }
        }
    }

    #[test]
    fn graph_layout_preserves_last_duplicate_id_for_endpoints_and_results() {
        let (nodes, _) = basic_nodes_and_edges();
        let mut graph = graph_from_indexed(&nodes, &[IndexedEdge { a: 0, b: 1 }]);
        graph.nodes[2].id = graph.nodes[0].id.clone();
        let expected = layout_indexed(&nodes, &[IndexedEdge { a: 2, b: 1 }])
            .expect("last matching node owns the edge");

        let actual = layout(&graph).expect("duplicate ids retain their existing semantics");

        assert_eq!(actual.positions.len(), 2);
        assert_points_identical(
            &[
                actual.positions[&graph.nodes[1].id],
                actual.positions[&graph.nodes[2].id],
            ],
            &[expected[1], expected[2]],
        );
    }

    #[test]
    fn controlled_layout_preserves_output_and_has_an_exact_budget_boundary() {
        let (nodes, edges) = mixed_size_tree_nodes_and_edges();
        let compatibility = layout_indexed(&nodes, &edges).expect("compatibility layout");

        let mut recording = RecordingWorkControl::unbounded();
        let controlled = layout_indexed_with_work_control(&nodes, &edges, &mut recording)
            .expect("recorded layout");
        assert_points_identical(&controlled, &compatibility);
        assert!(recording.used > 0);
        assert!(recording.checks > 0);
        assert!(recording.charges > 0);

        let exact = recording.used;
        let mut exact_control = RecordingWorkControl::limited(exact);
        let exact_output = layout_indexed_with_work_control(&nodes, &edges, &mut exact_control)
            .expect("exact budget should admit the complete layout");
        assert_eq!(exact_control.used, exact);
        assert_points_identical(&exact_output, &compatibility);

        let mut short_control = RecordingWorkControl::limited(exact - 1);
        let error = layout_indexed_with_work_control(&nodes, &edges, &mut short_control)
            .expect_err("exact minus one must reject a complete work tranche");
        assert!(matches!(
            error,
            Error::WorkFailure(WorkFailure::Interrupted)
        ));
        assert!(short_control.used < exact);
    }

    #[test]
    fn controlled_layout_has_explicit_resource_first_endpoint_semantics() {
        let invalid_edges = [IndexedEdge { a: 0, b: 1 }];

        let mut empty_control = RecordingWorkControl::limited(0);
        assert!(
            layout_indexed_with_work_control(&[], &invalid_edges, &mut empty_control)
                .expect("empty compatibility case")
                .is_empty()
        );
        assert_eq!(empty_control.used, 0);

        let nodes = [IndexedNode {
            width: 10.0,
            height: 10.0,
            x: 0.0,
            y: 0.0,
        }];
        assert!(matches!(
            layout_indexed(&nodes, &invalid_edges),
            Err(Error::MissingEndpoint { ref edge_id }) if edge_id == "#0"
        ));

        let mut rejecting_control = RecordingWorkControl::limited(0);
        assert!(matches!(
            layout_indexed_with_work_control(&nodes, &invalid_edges, &mut rejecting_control),
            Err(Error::WorkFailure(WorkFailure::Interrupted))
        ));
        assert_eq!(rejecting_control.used, 0);
        assert_eq!(rejecting_control.charges, 0);
    }

    #[test]
    fn grid_planner_selects_the_smallest_bounded_representation() {
        assert_eq!(
            SimGridPlan::new(1, 1, 16, 16)
                .expect("dense plan")
                .storage_kind,
            SimGridStorageKind::Dense
        );
        assert_eq!(
            SimGridPlan::new(1_000, 1_000, 10, 100)
                .expect("sparse plan")
                .storage_kind,
            SimGridStorageKind::Sparse
        );
        assert_eq!(
            SimGridPlan::new(1_000, 1_000, 1_000_000, 2)
                .expect("implicit plan")
                .storage_kind,
            SimGridStorageKind::Implicit
        );
        assert!(matches!(
            SimGridPlan::new(usize::MAX, 2, 0, 0),
            Err(WorkFailure::ArithmeticOverflow)
        ));
    }

    #[test]
    fn dense_sparse_and_implicit_grids_preserve_candidate_order() {
        let nodes = vec![
            IndexedNode {
                width: 10.0,
                height: 10.0,
                x: 0.0,
                y: 0.0,
            };
            3
        ];
        let mut sim = SimGraph::from_indexed(&nodes, &[]);
        let bounds = [(0, 1), (1, 2), (0, 2)];
        for (node, (start_x, finish_x)) in sim.nodes.iter_mut().zip(bounds) {
            node.start_x = start_x;
            node.finish_x = finish_x;
            node.start_y = 0;
            node.finish_y = 0;
        }

        let mut outputs = Vec::new();
        for storage_kind in [
            SimGridStorageKind::Dense,
            SimGridStorageKind::Sparse,
            SimGridStorageKind::Implicit,
        ] {
            let mut grid = SimGrid::default();
            grid.reset(
                SimGridPlan {
                    size_x: 3,
                    size_y: 1,
                    total_cell_count: 3,
                    cell_reference_count: 7,
                    active_node_count: 3,
                    storage_kind,
                    work_units: 0,
                },
                0.0,
                0.0,
                1.0,
            )
            .expect("grid reset");
            if storage_kind == SimGridStorageKind::Implicit {
                for node_idx in 0..sim.nodes.len() {
                    grid.register_implicit_node(node_idx);
                }
            } else {
                for (node_idx, node) in sim.nodes.iter().enumerate() {
                    for x in node.start_x as usize..=node.finish_x as usize {
                        grid.push(x, 0, node_idx);
                    }
                }
            }

            let mut by_cell = Vec::new();
            let mut candidates = Vec::new();
            for x in 0..3 {
                grid.fill_cell_candidates(&sim.nodes, x, 0, &mut candidates);
                by_cell.push(candidates.clone());
            }
            outputs.push(by_cell);
        }

        assert_eq!(outputs[0], vec![vec![0, 2], vec![0, 1, 2], vec![1, 2]]);
        assert_eq!(outputs[1], outputs[0]);
        assert_eq!(outputs[2], outputs[0]);
    }

    #[test]
    fn grid_admission_rejection_preserves_previous_grid_and_node_bounds() {
        let nodes = vec![
            IndexedNode {
                width: 10.0,
                height: 10.0,
                x: 0.0,
                y: 0.0,
            },
            IndexedNode {
                width: 10.0,
                height: 10.0,
                x: 150.0,
                y: 0.0,
            },
        ];
        let mut sim = SimGraph::from_indexed(&nodes, &[]);
        sim.update_grid(100.0, &mut NoopWorkControl)
            .expect("initial grid");
        let previous_grid = format!("{:?}", sim.grid);
        let previous_bounds = sim
            .nodes
            .iter()
            .map(|node| (node.start_x, node.finish_x, node.start_y, node.finish_y))
            .collect::<Vec<_>>();

        sim.nodes[1].left = 1_000_000_000.0;
        let mut rejecting_control = RecordingWorkControl::limited(sim.nodes.len() * 2);
        assert!(matches!(
            sim.update_grid(100.0, &mut rejecting_control),
            Err(WorkFailure::Interrupted)
        ));
        assert_eq!(format!("{:?}", sim.grid), previous_grid);
        assert_eq!(
            sim.nodes
                .iter()
                .map(|node| (node.start_x, node.finish_x, node.start_y, node.finish_y))
                .collect::<Vec<_>>(),
            previous_bounds
        );
    }

    #[test]
    fn large_finite_coordinate_span_uses_sparse_grid_storage() {
        let nodes = vec![
            IndexedNode {
                width: 10.0,
                height: 10.0,
                x: 0.0,
                y: 0.0,
            },
            IndexedNode {
                width: 10.0,
                height: 10.0,
                x: 1_000_000_000.0,
                y: 0.0,
            },
        ];
        let mut sim = SimGraph::from_indexed(&nodes, &[]);
        sim.update_grid(100.0, &mut NoopWorkControl)
            .expect("large finite span should avoid dense allocation");
        assert!(matches!(&sim.grid.cells, SimGridCells::Sparse(_)));
    }

    #[test]
    fn layout_indexed_handles_deep_tree_radial_layout_with_small_stack() {
        const DEPTH: usize = 2_048;
        let nodes = vec![
            IndexedNode {
                width: 48.0,
                height: 24.0,
                x: 0.0,
                y: 0.0,
            };
            DEPTH
        ];
        let edges = (1..DEPTH)
            .map(|idx| IndexedEdge { a: idx - 1, b: idx })
            .collect::<Vec<_>>();

        let handle = std::thread::Builder::new()
            .name("cose-bilkent-deep-tree-radial-layout".to_string())
            .stack_size(64 * 1024)
            .spawn(move || {
                let out = layout_indexed(&nodes, &edges)
                    .expect("deep tree layout should not depend on recursive stack growth");
                assert_eq!(out.len(), DEPTH);
                assert!(
                    out.iter()
                        .all(|point| point.x.is_finite() && point.y.is_finite()),
                    "deep tree layout should emit finite positions"
                );
            })
            .expect("spawn COSE-Bilkent deep tree layout test");
        handle
            .join()
            .expect("COSE-Bilkent deep tree layout should finish without stack overflow");
    }

    #[test]
    fn basic_three_node_tree_radial_init_has_equal_y() {
        let nodes = vec![
            IndexedNode {
                width: 69.734375,
                height: 34.0,
                x: 0.0,
                y: 0.0,
            },
            IndexedNode {
                width: 48.40625,
                height: 34.0,
                x: 0.0,
                y: 0.0,
            },
            IndexedNode {
                width: 48.921875,
                height: 34.0,
                x: 0.0,
                y: 0.0,
            },
        ];
        let edges = vec![IndexedEdge { a: 0, b: 1 }, IndexedEdge { a: 0, b: 2 }];

        let mut sim = SimGraph::from_indexed(&nodes, &edges);
        let mut work_control = NoopWorkControl;
        let forest = sim
            .get_flat_forest(&mut work_control)
            .expect("unbounded forest");
        assert_eq!(forest.len(), 1);
        sim.position_nodes_radially(&forest, &mut work_control)
            .expect("unbounded radial layout");

        let y0 = sim.nodes[0].center_y();
        for (i, n) in sim.nodes.iter().enumerate() {
            assert!(
                n.center_y() == y0,
                "radial init center_y mismatch: node[{i}] y={} vs y0={}",
                n.center_y(),
                y0
            );
        }
    }

    #[test]
    fn basic_three_node_tree_tick1_keeps_equal_y() {
        let nodes = vec![
            IndexedNode {
                width: 69.734375,
                height: 34.0,
                x: 0.0,
                y: 0.0,
            },
            IndexedNode {
                width: 48.40625,
                height: 34.0,
                x: 0.0,
                y: 0.0,
            },
            IndexedNode {
                width: 48.921875,
                height: 34.0,
                x: 0.0,
                y: 0.0,
            },
        ];
        let edges = vec![IndexedEdge { a: 0, b: 1 }, IndexedEdge { a: 0, b: 2 }];

        let mut sim = SimGraph::from_indexed(&nodes, &edges);
        assert_eq!(sim.edges.len(), 2);
        assert_eq!(sim.edges[0].a, 0);
        assert_eq!(sim.edges[0].b, 1);
        assert_eq!(sim.edges[1].a, 0);
        assert_eq!(sim.edges[1].b, 2);
        let mut work_control = NoopWorkControl;
        let forest = sim
            .get_flat_forest(&mut work_control)
            .expect("unbounded forest");
        sim.position_nodes_radially(&forest, &mut work_control)
            .expect("unbounded radial layout");

        let y0 = sim.nodes[0].center_y();
        for n in &sim.nodes {
            assert_eq!(n.center_y(), y0);
        }
        // Mirror the spring embedder's tick#1 grid rebuild (should not affect geometry).
        sim.update_grid(
            2.0 * super::SimGraph::DEFAULT_EDGE_LENGTH,
            &mut work_control,
        )
        .expect("unbounded grid update");
        // Sanity: for a horizontal arrangement, clipping points should preserve equal y.
        {
            let (t1x, t1y, s1x, s1y, ov1) =
                super::rect_intersection_points(&sim.nodes[1], &sim.nodes[0]);
            assert!(!ov1);
            assert_eq!(
                t1y, s1y,
                "edge(0->1) clip y differs: t=({t1x},{t1y}) s=({s1x},{s1y})"
            );
            let (t2x, t2y, s2x, s2y, ov2) =
                super::rect_intersection_points(&sim.nodes[2], &sim.nodes[0]);
            assert!(!ov2);
            assert_eq!(
                t2y, s2y,
                "edge(0->2) clip y differs: t=({t2x},{t2y}) s=({s2x},{s2y})"
            );
        }
        sim.run_single_spring_tick_flat_graph();

        for (i, n) in sim.nodes.iter().enumerate() {
            assert!(
                n.center_y() == y0,
                "tick1 center_y mismatch: node[{i}] y={} vs y0={}",
                n.center_y(),
                y0
            );
        }
    }

    #[test]
    fn find_center_of_tree_matches_layout_base_buggy_semantics() {
        // Star-shaped tree: node 0 connected to all others.
        // With the upstream `findCenterOfTree()` bug (removing from `list` while iterating over it),
        // the result depends on insertion order and ends up not being the actual tree center.
        let n = 21usize;
        let nodes: Vec<IndexedNode> = (0..n)
            .map(|_| IndexedNode {
                width: 10.0,
                height: 10.0,
                x: 0.0,
                y: 0.0,
            })
            .collect();
        let edges: Vec<IndexedEdge> = (1..n).map(|i| IndexedEdge { a: 0, b: i }).collect();

        let sim = SimGraph::from_indexed(&nodes, &edges);
        let list: Vec<usize> = (0..n).collect();
        let mut work_control = NoopWorkControl;

        assert_eq!(
            sim.find_center_of_tree(&list, &mut work_control)
                .expect("unbounded center search"),
            7
        );
    }
}
