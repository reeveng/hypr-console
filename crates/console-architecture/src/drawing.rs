//! The map, written as Graphviz and as a page a browser opens.
//!
//! One node per unit, per crate that does something the map draws, per program
//! run, per pool event group, and one each for the compositor and a unix
//! socket; one edge per thing done. What a crate links is not drawn: every
//! crate links most of the core, and the edges would be the whole picture. The
//! live check reads links, because a process is everything it linked.
//!
//! Sorted everywhere, so the file changes when the desktop does and at no
//! other time.
//!
//! The picture `dot` lays out is the whole desktop at once, and at a couple of
//! hundred nodes it is a picture nobody can follow one line of. The page is
//! the same nodes and edges handed to Cytoscape, where a node tapped is its
//! neighbours lit and everything else faded -- which is the question somebody
//! opening a map of this desktop actually has. It was a bare JSON file first,
//! and a file of elements is something nothing on a machine opens; it was also
//! read back out of the Graphviz attribute strings, so a label with a comma in
//! it split, and a unit ordered against one outside the tree was an edge to a
//! node nobody declared, which `dot` draws on its own and Cytoscape refuses
//! the whole map over. So a node is what it is rather than a string of
//! attributes, both drawings are read off the one [`Drawing`], and every end
//! of every edge is declared.
//!
//! The page holds no colour. It imports the palette the browser's own
//! stylesheets import and asks it for the names, so `theme/palette.toml` is
//! still the one place a colour is decided.

use std::collections::{BTreeMap, BTreeSet};

use console_core_never::Never;
use console_core_words::Words;
use serde::Serialize;

use crate::Architecture;
use crate::facts::{self, ANY_EVENT_GROUP, Fact, Kind};
use crate::units::{Started, Unit};

const HEAD: &str = "\
digraph desktop {
  graph [rankdir=LR, fontname=\"sans-serif\", label=\"How the running desktop is connected. Written by `just map`; edit nothing here.\"];
  node [fontname=\"sans-serif\", fontsize=10];
  edge [fontname=\"sans-serif\", fontsize=9];
";

const COMPOSITOR: &str = "compositor";

const COMPOSITOR_EVENTS: &str = "compositor events";

const UNIX_SOCKET: &str = "a unix socket";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Edge {
    from: String,
    to: String,
    label: String,
    style: Line,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Line {
    Solid,
    Dashed,
}

impl Line {
    fn written(self) -> Result<&'static str, Never> {
        Ok(match self {
            Line::Solid => "solid",
            Line::Dashed => "dashed",
        })
    }

    fn kind(self) -> Result<&'static str, Never> {
        Ok(match self {
            Line::Solid => "dependency",
            Line::Dashed => "ordering",
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct Ends<'a> {
    from: &'a str,
    to: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Words)]
enum Of {
    #[words(shape = "box", kind = "unit")]
    Unit,
    #[words(shape = "box", kind = "unit")]
    EnabledUnit,
    #[words(shape = "box3d", kind = "binary")]
    Binary,
    #[words(shape = "component", kind = "program")]
    Program,
    #[words(shape = "box", kind = "internal")]
    Internal,
    #[words(shape = "note", kind = "event-group")]
    EventGroup,
    #[words(shape = "cylinder", kind = "compositor")]
    Compositor,
    #[words(shape = "circle", kind = "socket")]
    Socket,
    #[words(shape = "ellipse", kind = "crate")]
    Crate,
}

impl Of {
    fn style(self) -> Result<Option<&'static str>, Never> {
        Ok(match self {
            Of::EnabledUnit => Some("bold"),
            Of::Unit => Some("solid"),
            Of::Internal => Some("rounded"),
            Of::Binary | Of::Program | Of::EventGroup | Of::Compositor | Of::Socket | Of::Crate => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Look {
    label: String,
    of: Of,
}

#[derive(Debug, Clone)]
struct Node {
    id: String,
    look: Look,
}

#[derive(Debug, Default)]
struct Drawing {
    nodes: BTreeMap<String, Look>,
    edges: BTreeSet<Edge>,
}

#[derive(Serialize)]
struct Element<T> {
    data: T,
}

#[derive(Serialize)]
struct NodeData<'a> {
    id: &'a str,
    label: &'a str,
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Serialize)]
struct EdgeData<'a> {
    id: String,
    source: &'a str,
    target: &'a str,
    label: &'a str,
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Serialize)]
struct Elements<'a> {
    nodes: Vec<Element<NodeData<'a>>>,
    edges: Vec<Element<EdgeData<'a>>>,
}

impl Drawing {
    fn node(&mut self, node: Node) -> Result<(), Never> {
        let _ = self.nodes.insert(node.id, node.look);

        Ok(())
    }

    fn named_only(&mut self, node: Node) -> Result<(), Never> {
        let _ = self.nodes.entry(node.id).or_insert(node.look);

        Ok(())
    }

    fn edge(&mut self, ends: Ends<'_>, label: &str) -> Result<(), Never> {
        self.line(ends, label, Line::Solid)
    }

    fn add_ordering(&mut self, ends: Ends<'_>, label: &str) -> Result<(), Never> {
        self.line(ends, label, Line::Dashed)
    }

    fn line(&mut self, ends: Ends<'_>, label: &str, style: Line) -> Result<(), Never> {
        let _ = self.edges.insert(Edge { from: String::from(ends.from), to: String::from(ends.to), label: String::from(label), style });

        Ok(())
    }

    fn to_dot(&self) -> Result<String, Never> {
        let mut out = String::from(HEAD);

        for (id, look) in &self.nodes {
            let Ok(shape) = look.of.shape();
            let Ok(style) = look.of.style();
            let styled = match style {
                Some(style) => format!(", style={style}"),
                None => String::new(),
            };

            out.push_str(&format!("  {id:?} [label={:?}, shape={shape}{styled}];\n", look.label));
        }

        for edge in &self.edges {
            let Ok(style) = edge.style.written();

            out.push_str(&format!("  {:?} -> {:?} [label={:?}, style={style}];\n", edge.from, edge.to, edge.label));
        }

        out.push_str("}\n");

        Ok(out)
    }

    fn to_elements(&self) -> Result<String, serde_json::Error> {
        let nodes = self
            .nodes
            .iter()
            .map(|(id, look)| {
                let Ok(kind) = look.of.kind();

                Element { data: NodeData { id, label: &look.label, kind } }
            })
            .collect();
        let edges = (0_u32..)
            .zip(&self.edges)
            .map(|(numbered, edge)| {
                let Ok(kind) = edge.style.kind();

                Element { data: EdgeData { id: format!("e{numbered}"), source: &edge.from, target: &edge.to, label: &edge.label, kind } }
            })
            .collect();

        serde_json::to_string_pretty(&Elements { nodes, edges })
    }
}

fn unit_id(name: &str) -> Result<String, Never> {
    Ok(format!("unit:{name}"))
}

fn crate_id(package: &str) -> Result<String, Never> {
    Ok(format!("crate:{package}"))
}

fn packages_of_binaries(facts: &[Fact]) -> Result<BTreeMap<String, String>, Never> {
    Ok(facts
        .iter()
        .filter(|fact| fact.kind == Kind::Builds && fact.target != facts::LIBRARY)
        .map(|fact| (fact.target.clone(), fact.package.clone()))
        .collect())
}

fn unit(drawing: &mut Drawing, unit: &Unit, architecture: &Architecture) -> Result<(), Never> {
    let Ok(id) = unit_id(&unit.name);
    let Ok(binaries) = packages_of_binaries(&architecture.facts);
    let of = match architecture.enabled.contains(&unit.name) {
        true => Of::EnabledUnit,
        false => Of::Unit,
    };
    let Ok(()) = drawing.node(Node { id: id.clone(), look: Look { label: unit.name.clone(), of } });
    let Ok(started) = unit.exec_start();

    match started {
        Some(Started::InternalProgram(binary)) => match binaries.get(&binary) {
            Some(package) => {
                let Ok(to) = crate_id(package);
                let Ok(()) = drawing.edge(Ends { from: &id, to: &to }, &binary);
            }
            None => {
                let to = format!("binary:{binary}");
                let Ok(()) = drawing.node(Node { id: to.clone(), look: Look { label: binary.clone(), of: Of::Binary } });
                let Ok(()) = drawing.edge(Ends { from: &id, to: &to }, "starts");
            }
        },
        Some(Started::ExternalProgram(program)) => {
            let to = format!("binary:{program}");
            let Ok(()) = drawing.node(Node { id: to.clone(), look: Look { label: program.clone(), of: Of::Program } });
            let Ok(()) = drawing.edge(Ends { from: &id, to: &to }, "starts");
        }
        None => {}
    }

    let Ok(ordered) = unit.orderings();

    for (other, said) in ordered {
        let Ok(to) = unit_id(&other);
        let Ok(()) = drawing.named_only(Node { id: to.clone(), look: Look { label: other.clone(), of: Of::Unit } });
        let Ok(()) = drawing.add_ordering(Ends { from: &id, to: &to }, said);
    }

    let Ok(triggers) = unit.triggers();

    match triggers {
        Some(service) => {
            let Ok(to) = unit_id(&service);
            let Ok(()) = drawing.named_only(Node { id: to.clone(), look: Look { label: service.clone(), of: Of::Unit } });
            let Ok(()) = drawing.edge(Ends { from: &id, to: &to }, "starts");
        }
        None => {}
    }

    Ok(())
}

fn event_group(drawing: &mut Drawing, named: &str) -> Result<String, Never> {
    let id = format!("event-group:{named}");
    let label = match named == ANY_EVENT_GROUP {
        true => "any event group its caller names",
        false => named,
    };
    let Ok(()) = drawing.node(Node { id: id.clone(), look: Look { label: String::from(label), of: Of::EventGroup } });

    Ok(id)
}

fn elsewhere(drawing: &mut Drawing, id: &str, of: Of) -> Result<String, Never> {
    let Ok(()) = drawing.node(Node { id: String::from(id), look: Look { label: String::from(id), of } });

    Ok(String::from(id))
}

fn fact(drawing: &mut Drawing, fact: &Fact) -> Result<(), Never> {
    let Ok(from) = crate_id(&fact.package);
    let what = fact.what.as_str();

    let (to, label) = match fact.kind {
        Kind::Runs => {
            let to = format!("program:{what}");
            let Ok(()) = drawing.node(Node { id: to.clone(), look: Look { label: what.to_lowercase(), of: Of::Program } });

            (to, "runs")
        }
        Kind::Starts => {
            let to = format!("internal:{what}");
            let Ok(()) = drawing.node(Node { id: to.clone(), look: Look { label: String::from(what), of: Of::Internal } });

            (to, "starts")
        }
        Kind::Sources => {
            let Ok(to) = event_group(drawing, what);

            (to, "sources")
        }
        Kind::Asks => {
            let Ok(to) = elsewhere(drawing, COMPOSITOR, Of::Compositor);

            (to, "asks")
        }
        Kind::Socket => match what == "Events" {
            true => {
                let Ok(to) = elsewhere(drawing, COMPOSITOR_EVENTS, Of::Compositor);

                (to, "names the socket")
            }
            false => {
                let Ok(to) = elsewhere(drawing, COMPOSITOR, Of::Compositor);

                (to, "names the socket")
            }
        },
        Kind::Connects => {
            let Ok(to) = elsewhere(drawing, UNIX_SOCKET, Of::Socket);

            (to, "connects")
        }
        Kind::Binds => {
            let Ok(to) = elsewhere(drawing, UNIX_SOCKET, Of::Socket);

            (to, "binds")
        }
        Kind::Builds | Kind::Links | Kind::Names | Kind::Handles | Kind::Subscribes | Kind::Timer => return Ok(()),
    };

    drawing.edge(Ends { from: &from, to: &to }, label)
}

fn packages(drawing: &mut Drawing, architecture: &Architecture) -> Result<(), Never> {
    let Ok(timers) = facts::packages(&architecture.facts, Kind::Timer);
    let Ok(subscribed) = facts::subscriptions(&architecture.facts);

    for one in &architecture.facts {
        let Ok(()) = fact(drawing, one);
    }

    for (target, event_groups) in &subscribed {
        let Ok(from) = crate_id(&target.package);

        for named in event_groups {
            let Ok(to) = event_group(drawing, named);
            let Ok(()) = drawing.edge(Ends { from: &from, to: &to }, "subscribes");
        }
    }

    let drawn: BTreeSet<String> = drawing.edges.iter().flat_map(|edge| [edge.from.clone(), edge.to.clone()]).collect();

    let built: BTreeSet<&str> = architecture
        .facts
        .iter()
        .filter(|fact| fact.kind == Kind::Builds)
        .map(|fact| fact.package.as_str())
        .collect();

    for package in built {
        let Ok(id) = crate_id(package);
        let clock = match timers.get(package) {
            Some(how) => format!("\nkeeps a clock: {}", how.iter().cloned().collect::<Vec<_>>().join(", ")),
            None => String::new(),
        };

        match drawn.contains(&id) {
            true => {
                let Ok(()) = drawing.node(Node { id, look: Look { label: format!("{package}{clock}"), of: Of::Crate } });
            }
            false => {}
        }
    }

    Ok(())
}

fn drawing(architecture: &Architecture) -> Result<Drawing, Never> {
    let mut drawing = Drawing::default();

    for one in &architecture.units {
        let Ok(()) = unit(&mut drawing, one, architecture);
    }

    let Ok(()) = packages(&mut drawing, architecture);

    Ok(drawing)
}

pub fn draw(architecture: &Architecture) -> Result<String, Never> {
    let Ok(drawing) = drawing(architecture);

    drawing.to_dot()
}

pub fn elements(architecture: &Architecture) -> Result<String, serde_json::Error> {
    let Ok(drawing) = drawing(architecture);

    drawing.to_elements()
}

pub fn page(architecture: &Architecture) -> Result<String, serde_json::Error> {
    let elements = elements(architecture)?;
    let held = elements.replace("</", "<\\/");

    Ok(format!("{PAGE_HEAD}{held};\n{PAGE_TAIL}"))
}

const PAGE_HEAD: &str = "\
<!doctype html>
<html lang=\"en\">
<meta charset=\"utf-8\">
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">
<title>Desktop map</title>
<link rel=\"stylesheet\" href=\"../../files/home/@user@/.librewolf/console/chrome/palette.css\">
<style>
  html, body { margin: 0; height: 100%; background: var(--night); color: var(--text); font: 14px sans-serif; }
  #map { position: absolute; inset: 0; }
  #said { position: absolute; left: 16px; bottom: 16px; right: 16px; margin: 0; color: var(--soft); pointer-events: none; }
</style>
<div id=\"map\"></div>
<p id=\"said\">How the running desktop is connected. Written by `just map`; edit nothing here. Tap a node for what it touches, tap the background for all of it.</p>
<script src=\"https://cdnjs.cloudflare.com/ajax/libs/cytoscape/3.30.2/cytoscape.min.js\"></script>
<script>
const ELEMENTS = ";

const PAGE_TAIL: &str = "\
const ink = (name) => getComputedStyle(document.documentElement).getPropertyValue(`--${name}`).trim();
const painted = (kind, shape, color) => ({ selector: `node[type = \"${kind}\"]`, style: { shape, 'background-color': ink(color) } });
const map = cytoscape({
  container: document.getElementById('map'),
  elements: ELEMENTS,
  layout: { name: 'cose', animate: false, nodeRepulsion: () => 40000, idealEdgeLength: () => 80 },
  style: [
    { selector: 'node', style: { label: 'data(label)', color: ink('text'), 'font-size': 9, 'text-wrap': 'wrap', 'text-valign': 'bottom', 'text-margin-y': 3, width: 14, height: 14 } },
    painted('crate', 'ellipse', 'pink'),
    painted('unit', 'round-rectangle', 'sky'),
    painted('internal', 'round-rectangle', 'lilac'),
    painted('binary', 'rectangle', 'butter'),
    painted('program', 'hexagon', 'mint'),
    painted('event-group', 'tag', 'peach'),
    painted('compositor', 'barrel', 'mauve'),
    painted('socket', 'diamond', 'mauve'),
    { selector: 'edge', style: { width: 1, 'line-color': ink('edge'), 'target-arrow-color': ink('edge'), 'target-arrow-shape': 'triangle', 'curve-style': 'bezier', 'arrow-scale': 0.6 } },
    { selector: 'edge[type = \"ordering\"]', style: { 'line-style': 'dashed' } },
    { selector: '.faded', style: { opacity: 0.12 } },
    { selector: 'edge.lit', style: { label: 'data(label)', color: ink('soft'), 'font-size': 8, 'text-rotation': 'autorotate' } },
  ],
});
map.on('tap', 'node', (tapped) => {
  const near = tapped.target.closedNeighborhood();
  map.elements().addClass('faded').removeClass('lit');
  near.removeClass('faded');
  near.edges().addClass('lit');
});
map.on('tap', (tapped) => {
  if (tapped.target === map) map.elements().removeClass('faded').removeClass('lit');
});
</script>
</html>
";
