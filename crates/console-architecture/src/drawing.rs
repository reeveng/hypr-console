//! The map, written as Graphviz.
//!
//! One node per unit, per crate that does something the map draws, per
//! program run, per pool topic, and one each for the compositor and a unix
//! socket; one edge per thing done. What a crate links is not drawn: every
//! crate links most of the core, and the edges would be the whole picture.
//! The live check reads links, because a process is everything it linked.
//!
//! Sorted everywhere, so the file changes when the desktop does and at no
//! other time.

use std::collections::{BTreeMap, BTreeSet};

use console_core_never::Never;

use crate::Architecture;
use crate::facts::{self, ANY_TOPIC, Fact, Kind};
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
    style: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct Ends<'a> {
    from: &'a str,
    to: &'a str,
}

#[derive(Debug, Clone)]
struct Node {
    id: String,
    attributes: String,
}

#[derive(Debug, Clone, Copy)]
struct Shape(&'static str);

#[derive(Debug, Default)]
struct Drawing {
    nodes: BTreeMap<String, String>,
    edges: BTreeSet<Edge>,
}

impl Drawing {
    fn node(&mut self, node: Node) -> Result<(), Never> {
        let _ = self.nodes.insert(node.id, node.attributes);

        Ok(())
    }

    fn edge(&mut self, ends: Ends<'_>, label: &str) -> Result<(), Never> {
        let _ = self.edges.insert(Edge {
            from: String::from(ends.from),
            to: String::from(ends.to),
            label: String::from(label),
            style: "solid",
        });

        Ok(())
    }

    fn ordering(&mut self, ends: Ends<'_>, label: &str) -> Result<(), Never> {
        let _ = self.edges.insert(Edge {
            from: String::from(ends.from),
            to: String::from(ends.to),
            label: String::from(label),
            style: "dashed",
        });

        Ok(())
    }

    fn written(&self) -> Result<String, Never> {
        let mut out = String::from(HEAD);

        for (id, attributes) in &self.nodes {
            out.push_str(&format!("  {id:?} [{attributes}];\n"));
        }

        for edge in &self.edges {
            out.push_str(&format!(
                "  {:?} -> {:?} [label={:?}, style={}];\n",
                edge.from, edge.to, edge.label, edge.style
            ));
        }

        out.push_str("}\n");

        Ok(out)
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
    let style = match architecture.enabled.contains(&unit.name) {
        true => "bold",
        false => "solid",
    };
    let Ok(()) = drawing.node(Node { id: id.clone(), attributes: format!("label={:?}, shape=box, style={style}", unit.name) });
    let Ok(started) = unit.started();

    match started {
        Some(Started::InternalProgram(binary)) => match binaries.get(&binary) {
            Some(package) => {
                let Ok(to) = crate_id(package);
                let Ok(()) = drawing.edge(Ends { from: &id, to: &to }, &binary);
            }
            None => {
                let to = format!("binary:{binary}");
                let Ok(()) = drawing.node(Node { id: to.clone(), attributes: format!("label={binary:?}, shape=box3d") });
                let Ok(()) = drawing.edge(Ends { from: &id, to: &to }, "starts");
            }
        },
        Some(Started::ExternalProgram(program)) => {
            let to = format!("binary:{program}");
            let Ok(()) = drawing.node(Node { id: to.clone(), attributes: format!("label={program:?}, shape=component") });
            let Ok(()) = drawing.edge(Ends { from: &id, to: &to }, "starts");
        }
        None => {}
    }

    let Ok(ordered) = unit.ordered();

    for (other, said) in ordered {
        let Ok(to) = unit_id(&other);
        let Ok(()) = drawing.ordering(Ends { from: &id, to: &to }, said);
    }

    let Ok(triggers) = unit.triggers();

    match triggers {
        Some(service) => {
            let Ok(to) = unit_id(&service);
            let Ok(()) = drawing.edge(Ends { from: &id, to: &to }, "starts");
        }
        None => {}
    }

    Ok(())
}

fn topic(drawing: &mut Drawing, named: &str) -> Result<String, Never> {
    let id = format!("topic:{named}");
    let label = match named == ANY_TOPIC {
        true => "a topic it was handed",
        false => named,
    };
    let Ok(()) = drawing.node(Node { id: id.clone(), attributes: format!("label={label:?}, shape=note") });

    Ok(id)
}

fn elsewhere(drawing: &mut Drawing, id: &str, shape: Shape) -> Result<String, Never> {
    let Ok(()) = drawing.node(Node { id: String::from(id), attributes: format!("label={id:?}, shape={}", shape.0) });

    Ok(String::from(id))
}

fn fact(drawing: &mut Drawing, fact: &Fact) -> Result<(), Never> {
    let Ok(from) = crate_id(&fact.package);
    let what = fact.what.as_str();

    let (to, label) = match fact.kind {
        Kind::Runs => {
            let to = format!("program:{what}");
            let Ok(()) = drawing.node(Node { id: to.clone(), attributes: format!("label={:?}, shape=component", what.to_lowercase()) });

            (to, "runs")
        }
        Kind::Starts => {
            let to = format!("internal:{what}");
            let Ok(()) = drawing.node(Node { id: to.clone(), attributes: format!("label={what:?}, shape=box, style=rounded") });

            (to, "starts")
        }
        Kind::Sources => {
            let Ok(to) = topic(drawing, what);

            (to, "sources")
        }
        Kind::Asks => {
            let Ok(to) = elsewhere(drawing, COMPOSITOR, Shape("cylinder"));

            (to, "asks")
        }
        Kind::Socket => match what == "Events" {
            true => {
                let Ok(to) = elsewhere(drawing, COMPOSITOR_EVENTS, Shape("cylinder"));

                (to, "names the socket")
            }
            false => {
                let Ok(to) = elsewhere(drawing, COMPOSITOR, Shape("cylinder"));

                (to, "names the socket")
            }
        },
        Kind::Connects => {
            let Ok(to) = elsewhere(drawing, UNIX_SOCKET, Shape("circle"));

            (to, "connects")
        }
        Kind::Binds => {
            let Ok(to) = elsewhere(drawing, UNIX_SOCKET, Shape("circle"));

            (to, "binds")
        }
        Kind::Builds | Kind::Links | Kind::Names | Kind::Handles | Kind::Subscribes | Kind::Timer => return Ok(()),
    };

    drawing.edge(Ends { from: &from, to: &to }, label)
}

fn packages(drawing: &mut Drawing, architecture: &Architecture) -> Result<(), Never> {
    let Ok(timers) = facts::packages(&architecture.facts, Kind::Timer);
    let Ok(subscribed) = facts::subscribed(&architecture.facts);

    for one in &architecture.facts {
        let Ok(()) = fact(drawing, one);
    }

    for (target, topics) in &subscribed {
        let Ok(from) = crate_id(&target.package);

        for named in topics {
            let Ok(to) = topic(drawing, named);
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
            Some(how) => format!("\\nkeeps a clock: {}", how.iter().cloned().collect::<Vec<_>>().join(", ")),
            None => String::new(),
        };

        match drawn.contains(&id) {
            true => {
                let Ok(()) = drawing.node(Node { id, attributes: format!("label=\"{package}{clock}\", shape=ellipse") });
            }
            false => {}
        }
    }

    Ok(())
}

pub fn drawn(architecture: &Architecture) -> Result<String, Never> {
    let mut drawing = Drawing::default();

    for one in &architecture.units {
        let Ok(()) = unit(&mut drawing, one, architecture);
    }

    let Ok(()) = packages(&mut drawing, architecture);

    drawing.written()
}
