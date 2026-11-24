use super::{FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct StartLinkSemantics {
    pub marker: FlowEdgeMarker,
    pub stroke_kind: FlowEdgeStroke,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct EndLinkSemantics {
    pub start_marker: FlowEdgeMarker,
    pub end_marker: FlowEdgeMarker,
    pub stroke_kind: FlowEdgeStroke,
    pub visibility: FlowEdgeVisibility,
    pub length: usize,
}

#[must_use]
fn count_char(ch: char, s: &str) -> usize {
    s.chars().filter(|&c| c == ch).count()
}

#[must_use]
pub(super) fn destruct_start_link(s: &str) -> StartLinkSemantics {
    let mut line = s.trim();
    let marker = match line.as_bytes().first().copied() {
        Some(b'<') => {
            line = &line[1..];
            FlowEdgeMarker::Point
        }
        Some(b'x') => {
            line = &line[1..];
            FlowEdgeMarker::Cross
        }
        Some(b'o') => {
            line = &line[1..];
            FlowEdgeMarker::Circle
        }
        _ => FlowEdgeMarker::None,
    };

    let stroke_kind = if line.contains('=') {
        FlowEdgeStroke::Thick
    } else if line.contains('.') {
        FlowEdgeStroke::Dotted
    } else {
        FlowEdgeStroke::Normal
    };
    StartLinkSemantics {
        marker,
        stroke_kind,
    }
}

#[must_use]
pub(super) fn destruct_end_link(s: &str) -> EndLinkSemantics {
    destruct_end_link_with_source_marker(s, true)
}

#[must_use]
pub(super) fn destruct_labeled_end_link(s: &str) -> EndLinkSemantics {




    destruct_end_link_with_source_marker(s, false)
}

#[must_use]
fn destruct_end_link_with_source_marker(
    s: &str,
    recognize_source_marker: bool,
) -> EndLinkSemantics {
    let line = s.trim();

    if line.len() < 2 {
        return EndLinkSemantics {
            start_marker: FlowEdgeMarker::None,
            end_marker: FlowEdgeMarker::None,
            stroke_kind: FlowEdgeStroke::Normal,
            visibility: FlowEdgeVisibility::Visible,
            length: 1,
        };
    }

    let start_marker = if recognize_source_marker {
        match line.as_bytes().first().copied() {
            Some(b'<') => FlowEdgeMarker::Point,
            Some(b'x') => FlowEdgeMarker::Cross,
            Some(b'o') => FlowEdgeMarker::Circle,
            _ => FlowEdgeMarker::None,
        }
    } else {
        FlowEdgeMarker::None
    };
    let end_marker = match line.as_bytes().last().copied() {
        Some(b'>') => FlowEdgeMarker::Point,
        Some(b'x') => FlowEdgeMarker::Cross,
        Some(b'o') => FlowEdgeMarker::Circle,
        _ => FlowEdgeMarker::None,
    };



    let mut stroke = &line[..line.len() - 1];

    if start_marker != FlowEdgeMarker::None {
        stroke = &stroke[1..];
    }

    let visibility = if stroke.starts_with('~') {
        FlowEdgeVisibility::Invisible
    } else {
        FlowEdgeVisibility::Visible
    };
    let mut stroke_kind = if stroke.starts_with('=') {
        FlowEdgeStroke::Thick
    } else {
        FlowEdgeStroke::Normal
    };
    let mut length = stroke.len().saturating_sub(1);
    let dots = count_char('.', stroke);

    if dots > 0 {
        stroke_kind = FlowEdgeStroke::Dotted;
        length = dots;
    }

    EndLinkSemantics {
        start_marker,
        end_marker,
        stroke_kind,
        visibility,
        length,
    }
}
