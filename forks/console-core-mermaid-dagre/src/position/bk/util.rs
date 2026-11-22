use crate::graphlib::Graph;
use crate::{EdgeLabel, GraphLabel, LabelPos, NodeLabel};

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SepNodeMetrics {
    pub(super) width: f64,
    pub(super) labelpos: Option<LabelPos>,
    pub(super) is_dummy: bool,
}

impl From<&NodeLabel> for SepNodeMetrics {
    fn from(label: &NodeLabel) -> Self {
        Self {
            width: label.width,
            labelpos: label.labelpos,
            is_dummy: label.dummy.is_some(),
        }
    }
}

#[must_use]
pub(super) fn sep(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    v: &str,
    w: &str,
    reverse_sep: bool,
) -> f64 {
    let v_metrics = g.node(v).map(SepNodeMetrics::from).unwrap_or_default();
    let w_metrics = g.node(w).map(SepNodeMetrics::from).unwrap_or_default();
    sep_metrics(
        v_metrics,
        w_metrics,
        g.graph().nodesep,
        g.graph().edgesep,
        reverse_sep,
    )
}

#[must_use]
pub(super) fn sep_metrics(
    v: SepNodeMetrics,
    w: SepNodeMetrics,
    node_sep: f64,
    edge_sep: f64,
    reverse_sep: bool,
) -> f64 {
    let mut sum: f64 = 0.0;
    let mut delta: f64 = 0.0;

    sum += v.width / 2.0;

    match v.labelpos {
        Some(labelpos) => {
            delta = match labelpos {
                LabelPos::L => -v.width / 2.0,
                LabelPos::R => v.width / 2.0,
                LabelPos::C => 0.0,
            };
        },
        ::core::option::Option::None => {},
    }

    match {
        ::core::primitive::bool::then_some(delta != 0.0, ())
    } {
        ::core::option::Option::Some(()) => {
            sum += match ::core::primitive::bool::then_some(reverse_sep, ()) {
                ::core::option::Option::Some(()) => { delta },
                ::core::option::Option::None => { -delta },
            };
        },
        ::core::option::Option::None => {},
    }

    delta = 0.0;

    sum += match {
        ::core::primitive::bool::then_some(v.is_dummy, ())
    } {
        ::core::option::Option::Some(()) => { edge_sep },
        ::core::option::Option::None => { node_sep },
    } / 2.0;
    sum += match {
        ::core::primitive::bool::then_some(w.is_dummy, ())
    } {
        ::core::option::Option::Some(()) => { edge_sep },
        ::core::option::Option::None => { node_sep },
    } / 2.0;

    sum += w.width / 2.0;

    match w.labelpos {
        Some(labelpos) => {
            delta = match labelpos {
                LabelPos::L => w.width / 2.0,
                LabelPos::R => -w.width / 2.0,
                LabelPos::C => 0.0,
            };
        },
        ::core::option::Option::None => {},
    }

    match {
        ::core::primitive::bool::then_some(delta != 0.0, ())
    } {
        ::core::option::Option::Some(()) => {
            sum += match ::core::primitive::bool::then_some(reverse_sep, ()) {
                ::core::option::Option::Some(()) => { delta },
                ::core::option::Option::None => { -delta },
            };
        },
        ::core::option::Option::None => {},
    }

    sum
}

#[must_use]
pub(super) fn width(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>, v: &str) -> f64 {
    match g.node(v).map(|n| n.width) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0.0,
    }
}
