use crate::text::{TextMetrics, TextStyle};
use indexmap::IndexMap;
use std::borrow::Cow;

#[must_use]
fn parse_style_decl(s: &str) -> Option<(&str, &str, bool)> {
    let parsed = crate::mermaid_style::parse_style_declaration(s)?;
    Some((parsed.property_source(), parsed.value(), parsed.important()))
}

#[must_use]
pub(crate) fn flowchart_split_mermaid_style_decls(s: &str) -> impl Iterator<Item = &str> {
    #[must_use]
    fn looks_like_key_start(s: &str) -> bool {
        let s = s.trim_start();

        let k = {
            let optional_k = s.split_once(':');

            match optional_k {
                Some((matched_k, _)) => matched_k,
                ::core::option::Option::None => return false,
            }
        };

        let k = k.trim();
        !k.is_empty()
            && k.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    }

    let mut parts: Vec<&str> = Vec::new();
    let mut start = 0usize;

    for (i, ch) in s.char_indices() {
        match ch {
            ',' => {},
            _ => {
                continue;
            },
        }

        if looks_like_key_start(&s[i + 1..]) {
            let p = s[start..i].trim();

            if !p.is_empty() {
                parts.push(p);
            }

            start = i + 1;
        }
    }

    let tail = s[start..].trim();

    if !tail.is_empty() {
        parts.push(tail);
    }

    parts.into_iter()
}

#[must_use]
pub(crate) fn flowchart_is_source_spelled_label_style_key(key: &str) -> bool {
    matches!(
        key.trim(),
        "color"
            | "font-size"
            | "font-family"
            | "font-weight"
            | "font-style"
            | "text-decoration"
            | "text-align"
            | "text-transform"
            | "line-height"
            | "letter-spacing"
            | "word-spacing"
            | "text-shadow"
            | "text-overflow"
            | "white-space"
            | "word-wrap"
            | "word-break"
            | "overflow-wrap"
            | "hyphens"
    )
}

#[derive(Debug)]
pub(crate) struct FlowchartTextStyleResolution<'a> {
    pub(crate) style: Cow<'a, TextStyle>,
    pub(crate) terminal_foreground: Option<FlowchartTerminalForeground>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowchartTerminalForegroundProvenance {
    ThemeNode,
    ThemeTitle,
    AssignedClass,
    InlineStyle,
    #[cfg(test)]
    EdgeLabelStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FlowchartTerminalForeground {
    value: String,
    provenance: FlowchartTerminalForegroundProvenance,
    important: bool,
}

impl FlowchartTerminalForeground {
    #[must_use]
    pub(crate) fn new(value: &str, provenance: FlowchartTerminalForegroundProvenance) -> Self {
        Self {
            value: value.trim().to_string(),
            provenance,
            important: false,
        }
    }

    fn observe(
        target: &mut Option<Self>,
        value: &str,
        provenance: FlowchartTerminalForegroundProvenance,
        important: bool,
    ) {
        if target
            .as_ref()
            .is_some_and(|current| current.important && !important)
        {
            return;
        }

        *target = Some(Self {
            value: value.trim().to_string(),
            provenance,
            important,
        });
    }

    #[must_use]
    pub(crate) fn value(&self) -> &str {
        &self.value
    }

    #[cfg(test)]
    pub(crate) const fn provenance(&self) -> FlowchartTerminalForegroundProvenance {
        self.provenance
    }
}

impl AsRef<TextStyle> for FlowchartTextStyleResolution<'_> {
    fn as_ref(&self) -> &TextStyle {
        self.style.as_ref()
    }
}

impl<'a> FlowchartTextStyleResolution<'a> {
    #[must_use]
    pub(crate) fn borrowed(style: &'a TextStyle) -> Self {
        Self {
            style: Cow::Borrowed(style),
            terminal_foreground: None,
        }
    }

    #[must_use]
    pub(crate) fn terminal_foreground(&self) -> Option<&FlowchartTerminalForeground> {
        self.terminal_foreground.as_ref()
    }
}

impl std::ops::Deref for FlowchartTextStyleResolution<'_> {
    type Target = TextStyle;

    fn deref(&self) -> &Self::Target {
        self.style.as_ref()
    }
}

struct FlowchartTextStyleResolver<'a, 's> {
    resolution: FlowchartTextStyleResolution<'a>,
    base: &'a TextStyle,
    typography: [Option<(&'s str, bool)>; 4],
}

impl<'a, 's> FlowchartTextStyleResolver<'a, 's> {
    #[must_use]
    fn new(base: &'a TextStyle) -> Self {
        Self {
            resolution: FlowchartTextStyleResolution::borrowed(base),
            base,
            typography: Default::default(),
        }
    }

    fn observe(
        &mut self,
        key: &str,
        value: &'s str,
        provenance: FlowchartTerminalForegroundProvenance,
        important: bool,
    ) {
        if !flowchart_is_source_spelled_label_style_key(key) {
            return;
        }

        let index = match key {
            "font-family" => Some(0),
            "font-size" => Some(1),
            "font-weight" => Some(2),
            "font-style" => Some(3),
            _ => None,
        };

        if let Some(index_2) = index
            && self.typography[index_2]
                .as_ref()
                .is_none_or(|(_, previous)| important || !previous)
        {

            self.typography[index_2] = Some((value, important));
        }

        if key.trim() == "color" {
            FlowchartTerminalForeground::observe(
                &mut self.resolution.terminal_foreground,
                value,
                provenance,
                important,
            );
        }
    }

    #[must_use]
    fn finish(mut self) -> FlowchartTextStyleResolution<'a> {
        let mut cascade = crate::mermaid_style::TextStyleCascade::new(
            self.base,
            crate::mermaid_style::CssFontSizeContext::uniform(self.base.font_size),
        );

        for (property, winner) in ["font-family", "font-size", "font-weight", "font-style"]
            .into_iter()
            .zip(self.typography.iter())
        {
            match winner {
                Some((value, important)) => {
                    cascade.observe(property, value, *important);
                },
                ::core::option::Option::None => {},
            }
        }

        self.resolution.style = cascade.finish();
        self.resolution
    }
}

#[must_use]
fn flowchart_text_style_resolver_for_class_names<'a, 'b, 's>(
    base: &'a TextStyle,
    class_defs: &'s IndexMap<String, Vec<String>>,
    class_names: impl IntoIterator<Item = &'b str>,
    inline_styles: &'s [String],
) -> FlowchartTextStyleResolver<'a, 's> {
    let mut resolver = FlowchartTextStyleResolver::new(base);

    for class in class_names {
        let decls = {
            let optional_decls = class_defs.get(class);

            match optional_decls {
                Some(matched_decls) => matched_decls,
                ::core::option::Option::None => continue,
            }
        };

        for d in decls {
            'over_d_3: for d_2 in flowchart_split_mermaid_style_decls(d) {
                let (k, v, important) = {
                    let optional_k_2 = parse_style_decl(d_2);

                    match optional_k_2 {
                        Some((matched_k_2, matched_v_2, matched_important)) => (matched_k_2, matched_v_2, matched_important),
                        ::core::option::Option::None => continue 'over_d_3,
                    }
                };

                resolver.observe(
                    k,
                    v,
                    FlowchartTerminalForegroundProvenance::AssignedClass,
                    important,
                );
            }
        }
    }

    for d in inline_styles {
        'over_d_2: for d_3 in flowchart_split_mermaid_style_decls(d) {
            let (k, v, important) = {
                let optional_k_2 = parse_style_decl(d_3);

                match optional_k_2 {
                    Some((matched_k_2, matched_v_2, matched_important)) => (matched_k_2, matched_v_2, matched_important),
                    ::core::option::Option::None => continue 'over_d_2,
                }
            };

            resolver.observe(
                k,
                v,
                FlowchartTerminalForegroundProvenance::InlineStyle,
                important,
            );
        }
    }

    resolver
}

#[must_use]
pub(super) fn flowchart_declared_font_facets(
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
    inline_styles: &[String],
    node_defaults: bool,
) -> [bool; 2] {
    let base = TextStyle::default();
    let class_names = if node_defaults {
        flowchart_effective_node_class_names(class_defs, classes)
    } else {
        classes.iter().map(String::as_str).collect()
    };
    let resolver = flowchart_text_style_resolver_for_class_names(
        &base,
        class_defs,
        class_names,
        inline_styles,
    );
    [
        resolver.typography[0].is_some(),
        resolver.typography[1].is_some(),
    ]
}

#[must_use]
pub(crate) fn flowchart_effective_node_class_names<'a>(
    class_defs: &'a IndexMap<String, Vec<String>>,
    classes: &'a [String],
) -> Vec<&'a str> {
    let mut effective: Vec<&'a str> = Vec::with_capacity(classes.len() + 2);

    if class_defs.contains_key("default") {
        effective.push("default");
    }

    if class_defs.contains_key("node") {
        effective.push("node");
    }

    effective.extend(classes.iter().map(|class| class.as_str()));
    effective
}

#[must_use]
pub(crate) fn flowchart_effective_text_style_for_node_classes<'a>(
    base: &'a TextStyle,
    class_defs: &'a IndexMap<String, Vec<String>>,
    classes: &'a [String],
    inline_styles: &[String],
) -> std::borrow::Cow<'a, TextStyle> {
    flowchart_effective_text_style_for_node_classes_with_provenance(
        base,
        class_defs,
        classes,
        inline_styles,
    )
    .style
}

#[must_use]
pub(crate) fn flowchart_effective_text_style_for_node_classes_with_provenance<'a>(
    base: &'a TextStyle,
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
    inline_styles: &[String],
) -> FlowchartTextStyleResolution<'a> {
    let effective_classes = flowchart_effective_node_class_names(class_defs, classes);

    if effective_classes.is_empty() && inline_styles.is_empty() {
        return FlowchartTextStyleResolution::borrowed(base);
    }

    flowchart_text_style_resolver_for_class_names(
        base,
        class_defs,
        effective_classes,
        inline_styles,
    )
    .finish()
}

#[must_use]
pub(crate) fn flowchart_effective_text_style_for_classes<'a>(
    base: &'a TextStyle,
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &'a [String],
    inline_styles: &[String],
) -> std::borrow::Cow<'a, TextStyle> {
    flowchart_effective_text_style_for_classes_with_provenance(
        base,
        class_defs,
        classes,
        inline_styles,
    )
    .style
}

#[must_use]
pub(crate) fn flowchart_effective_text_style_for_classes_with_provenance<'a>(
    base: &'a TextStyle,
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
    inline_styles: &[String],
) -> FlowchartTextStyleResolution<'a> {
    if classes.is_empty() && inline_styles.is_empty() {
        return FlowchartTextStyleResolution::borrowed(base);
    }

    flowchart_text_style_resolver_for_class_names(
        base,
        class_defs,
        classes.iter().map(|class| class.as_str()),
        inline_styles,
    )
    .finish()
}

/// Mermaid first compiles edge classes and then applies the concatenated `linkStyle default`
/// and per-edge declarations. The resulting style is applied after SVG line wrapping, but it owns
/// the final text bbox used by the layout graph.
#[cfg(test)]
pub(crate) fn flowchart_effective_edge_label_text_style<'a>(
    base: &'a TextStyle,
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &'a [String],
    default_edge_styles: &[String],
    edge_styles: &[String],
) -> std::borrow::Cow<'a, TextStyle> {
    flowchart_effective_edge_label_text_style_with_provenance(
        base,
        class_defs,
        classes,
        default_edge_styles,
        edge_styles,
    )
    .style
}

#[cfg(test)]
pub(crate) fn flowchart_effective_edge_label_text_style_with_provenance<'a>(
    base: &'a TextStyle,
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
    default_edge_styles: &[String],
    edge_styles: &[String],
) -> FlowchartTextStyleResolution<'a> {
    if classes.is_empty() && default_edge_styles.is_empty() && edge_styles.is_empty() {
        return FlowchartTextStyleResolution::borrowed(base);
    }

    let mut resolver = flowchart_text_style_resolver_for_class_names(
        base,
        class_defs,
        classes.iter().map(|class| class.as_str()),
        &[],
    );
    for declaration in default_edge_styles.iter().chain(edge_styles) {
        for declaration in flowchart_split_mermaid_style_decls(declaration) {
            let Some((key, value, important)) = parse_style_decl(declaration) else {
                continue;
            };
            resolver.observe(
                key,
                value,
                FlowchartTerminalForegroundProvenance::EdgeLabelStyle,
                important,
            );
        }
    }
    resolver.finish()
}

/// Mermaid's Swimlane adapter moves an edge label onto a fresh `labelRect` node and copies only
/// the first entry from the already-concatenated default/edge `labelStyle` array. Classes and later
/// style entries remain on the original edge and must not affect the label node's measurement.
#[cfg(test)]
pub(crate) fn flowchart_swimlane_label_rect_text_style<'a>(
    base: &'a TextStyle,
    default_edge_styles: &[String],
    edge_styles: &[String],
) -> std::borrow::Cow<'a, TextStyle> {
    flowchart_swimlane_label_rect_text_style_with_provenance(base, default_edge_styles, edge_styles)
        .style
}

#[cfg(test)]
pub(crate) fn flowchart_swimlane_label_rect_text_style_with_provenance<'a>(
    base: &'a TextStyle,
    default_edge_styles: &[String],
    edge_styles: &[String],
) -> FlowchartTextStyleResolution<'a> {
    let Some(first_style) = default_edge_styles.first().or_else(|| edge_styles.first()) else {
        return FlowchartTextStyleResolution::borrowed(base);
    };

    let mut resolver = FlowchartTextStyleResolver::new(base);
    for declaration in flowchart_split_mermaid_style_decls(first_style) {
        let Some((key, value, important)) = parse_style_decl(declaration) else {
            continue;
        };
        resolver.observe(
            key,
            value,
            FlowchartTerminalForegroundProvenance::EdgeLabelStyle,
            important,
        );
    }
    resolver.finish()
}

#[derive(Debug, Clone, Copy, Default)]
struct CssBoxEdges {
    top: f64,
    right: f64,
    bottom: f64,
    left: f64,
}

impl CssBoxEdges {
    fn set_shorthand(&mut self, values: &[f64]) {
        match values {
            [all] => {
                self.top = *all;
                self.right = *all;
                self.bottom = *all;
                self.left = *all;
            }
            [vertical, horizontal] => {
                self.top = *vertical;
                self.right = *horizontal;
                self.bottom = *vertical;
                self.left = *horizontal;
            }
            [top, horizontal, bottom] => {
                self.top = *top;
                self.right = *horizontal;
                self.bottom = *bottom;
                self.left = *horizontal;
            }
            [top, right, bottom, left] => {
                self.top = *top;
                self.right = *right;
                self.bottom = *bottom;
                self.left = *left;
            }
            _ => {}
        }
    }

    #[must_use]
    fn horizontal(self) -> f64 {
        self.left + self.right
    }

    #[must_use]
    fn vertical(self) -> f64 {
        self.top + self.bottom
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum HtmlSpanDisplay {
    #[default]
    Inline,
    InlineBlock,
    Other,
}

#[derive(Debug, Clone, Copy)]
struct HtmlSpanBoxStyle {
    display: HtmlSpanDisplay,
    margin: CssBoxEdges,
    padding: CssBoxEdges,
    border_width: CssBoxEdges,
    border_visible: [bool; 4],
    line_height: f64,
}

impl HtmlSpanBoxStyle {
    #[must_use]
    fn new(font_size: f64) -> Self {
        Self {
            display: HtmlSpanDisplay::Inline,
            margin: CssBoxEdges::default(),
            padding: CssBoxEdges::default(),


            border_width: CssBoxEdges {
                top: 3.0,
                right: 3.0,
                bottom: 3.0,
                left: 3.0,
            },
            border_visible: [false; 4],
            line_height: font_size.max(1.0) * 1.5,
        }
    }

    #[must_use]
    fn visible_border_width(self) -> CssBoxEdges {
        CssBoxEdges {
            top: if self.border_visible[0] {
                self.border_width.top
            } else {
                0.0
            },
            right: if self.border_visible[1] {
                self.border_width.right
            } else {
                0.0
            },
            bottom: if self.border_visible[2] {
                self.border_width.bottom
            } else {
                0.0
            },
            left: if self.border_visible[3] {
                self.border_width.left
            } else {
                0.0
            },
        }
    }
}

#[must_use]
fn css_box_length_px(raw: &str, font_size: f64, allow_negative: bool) -> Option<f64> {
    let raw_2 = raw.trim().trim_end_matches("!important").trim();

    if raw_2.eq_ignore_ascii_case("auto") {
        return Some(0.0);
    }

    let lower = raw_2.to_ascii_lowercase();
    let value = if let Some(value) = lower.strip_suffix("px") {
        match value.trim().parse::<f64>() { ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__), ::core::result::Result::Err(_invalid_float) => ::core::option::Option::None }?
    } else if let Some(value) = lower.strip_suffix("rem") {
        match value.trim().parse::<f64>() { ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__), ::core::result::Result::Err(_invalid_float) => ::core::option::Option::None }? * font_size
    } else if let Some(value) = lower.strip_suffix("em") {
        match value.trim().parse::<f64>() { ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__), ::core::result::Result::Err(_invalid_float) => ::core::option::Option::None }? * font_size
    } else if lower == "0" || lower == "+0" || lower == "-0" {
        0.0
    } else {
        return None;
    };

    if !value.is_finite() || (!allow_negative && value < 0.0) {
        return None;
    }

    Some(value)
}

#[must_use]
fn css_box_shorthand_px(raw: &str, font_size: f64, allow_negative: bool) -> Option<Vec<f64>> {
    let values = raw
        .split_ascii_whitespace()
        .map(|value| css_box_length_px(value, font_size, allow_negative))
        .collect::<Option<Vec<_>>>()?;
    (1..=4).contains(&values.len()).then_some(values)
}

#[must_use]
fn css_border_width_px(raw: &str, font_size: f64) -> Option<f64> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "thin" => Some(1.0),
        "medium" => Some(3.0),
        "thick" => Some(5.0),
        _ => css_box_length_px(raw, font_size, false),
    }
}

#[must_use]
fn css_border_style_visible(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "none" | "hidden" => Some(false),
        "dotted" | "dashed" | "solid" | "double" | "groove" | "ridge" | "inset" | "outset" => {
            Some(true)
        }
        _ => None,
    }
}

#[must_use]
fn css_border_shorthand(raw: &str, font_size: f64) -> (f64, bool) {
    let mut width = 3.0;
    let mut visible = false;

    for token in raw.split_ascii_whitespace() {
        match css_border_width_px(token, font_size) {
            Some(value) => {
                width = value;
            },
            ::core::option::Option::None => {},
        }

        {
            match css_border_style_visible(token) {
                Some(value) => {
                    visible = value;
                },
                ::core::option::Option::None => {},
            };
        }
    }

    (width, visible)
}

fn set_border_width_side(edges: &mut CssBoxEdges, side: usize, width: f64) {
    match side {
        0 => edges.top = width,
        1 => edges.right = width,
        2 => edges.bottom = width,
        3 => edges.left = width,
        _ => unreachable!("CSS box side index"),
    }
}

fn apply_html_span_box_decl(
    style: &mut HtmlSpanBoxStyle,
    property: &str,
    value: &str,
    font_size: f64,
) {
    let property_2 = property.trim().to_ascii_lowercase();

    match property_2.as_str() {
        "display" => {
            style.display = match value.trim().to_ascii_lowercase().as_str() {
                "inline" => HtmlSpanDisplay::Inline,
                "inline-block" => HtmlSpanDisplay::InlineBlock,
                _ => HtmlSpanDisplay::Other,
            };
        }
        "margin" => {
            {
                match css_box_shorthand_px(value, font_size, true) {
                    Some(values) => {
                        style.margin.set_shorthand(&values);
                    },
                    ::core::option::Option::None => {},
                };
            }
        }
        "padding" => {
            {
                match css_box_shorthand_px(value, font_size, false) {
                    Some(values) => {
                        style.padding.set_shorthand(&values);
                    },
                    ::core::option::Option::None => {},
                };
            }
        }
        "margin-top" | "margin-right" | "margin-bottom" | "margin-left" | "padding-top"
        | "padding-right" | "padding-bottom" | "padding-left" => {
            let is_margin = property_2.starts_with("margin-");
            let allow_negative = is_margin;

            let length = {
                let optional_length = css_box_length_px(value, font_size, allow_negative);

                match optional_length {
                    Some(matched_length) => matched_length,
                    ::core::option::Option::None => return,
                }
            };

            let edges = if is_margin {
                &mut style.margin
            } else {
                &mut style.padding
            };

            match property_2.rsplit('-').next() {
                Some("top") => edges.top = length,
                Some("right") => edges.right = length,
                Some("bottom") => edges.bottom = length,
                Some("left") => edges.left = length,
                _ => unreachable!("matched CSS box side"),
            }
        }
        "border" => {
            let (width, visible) = css_border_shorthand(value, font_size);
            style.border_width.set_shorthand(&[width]);
            style.border_visible.fill(visible);
        }
        "border-width" => {
            let values = value
                .split_ascii_whitespace()
                .map(|token| css_border_width_px(token, font_size))
                .collect::<Option<Vec<_>>>();

            {
                match values.filter(|values_2| (1..=4).contains(&values_2.len())) {
                    Some(values_3) => {
                        style.border_width.set_shorthand(&values_3);
                    },
                    ::core::option::Option::None => {},
                };
            }
        }
        "border-style" => {
            let values = value
                .split_ascii_whitespace()
                .map(css_border_style_visible)
                .collect::<Option<Vec<_>>>();

            {
                match values.filter(|values_4| (1..=4).contains(&values_4.len())) {
                    Some(values_5) => {
                        let mut expanded = CssBoxEdges::default();
                        expanded.set_shorthand(
                            &values_5
                                .iter()
                                .map(|visible| f64::from(*visible))
                                .collect::<Vec<_>>(),
                        );
                        style.border_visible = [
                            !::core::matches!(::core::primitive::f64::classify(expanded.top), ::core::num::FpCategory::Zero),
                            !::core::matches!(::core::primitive::f64::classify(expanded.right), ::core::num::FpCategory::Zero),
                            !::core::matches!(::core::primitive::f64::classify(expanded.bottom), ::core::num::FpCategory::Zero),
                            !::core::matches!(::core::primitive::f64::classify(expanded.left), ::core::num::FpCategory::Zero),
                        ];
                    },
                    ::core::option::Option::None => {},
                };
            }
        }
        "border-top" | "border-right" | "border-bottom" | "border-left" => {
            let side = match property_2.as_str() {
                "border-top" => 0,
                "border-right" => 1,
                "border-bottom" => 2,
                "border-left" => 3,
                _ => unreachable!("matched CSS border side"),
            };
            let (width, visible) = css_border_shorthand(value, font_size);
            set_border_width_side(&mut style.border_width, side, width);
            style.border_visible[side] = visible;
        }
        "border-top-width" | "border-right-width" | "border-bottom-width" | "border-left-width" => {
            let width = {
                let optional_width = css_border_width_px(value, font_size);

                match optional_width {
                    Some(matched_width) => matched_width,
                    ::core::option::Option::None => return,
                }
            };

            let side = match property_2.as_str() {
                "border-top-width" => 0,
                "border-right-width" => 1,
                "border-bottom-width" => 2,
                "border-left-width" => 3,
                _ => unreachable!("matched CSS border width side"),
            };
            set_border_width_side(&mut style.border_width, side, width);
        }
        "border-top-style" | "border-right-style" | "border-bottom-style" | "border-left-style" => {
            let visible = {
                let optional_visible = css_border_style_visible(value);

                match optional_visible {
                    Some(matched_visible) => matched_visible,
                    ::core::option::Option::None => return,
                }
            };

            let side = match property_2.as_str() {
                "border-top-style" => 0,
                "border-right-style" => 1,
                "border-bottom-style" => 2,
                "border-left-style" => 3,
                _ => unreachable!("matched CSS border style side"),
            };
            style.border_visible[side] = visible;
        }
        "line-height" => {
            let value_2 = value.trim().trim_end_matches("!important").trim();
            let parsed = if let Some(percent) = value_2.strip_suffix('%') {
                match percent
                    .trim()
                    .parse::<f64>() { ::core::result::Result::Ok(value__) => ::core::option::Option::Some(value__), ::core::result::Result::Err(_invalid_float) => ::core::option::Option::None }
                    .map(|percent_2| font_size * percent_2 / 100.0)
            } else if let Ok(factor) = value_2.parse::<f64>() {
                Some(font_size * factor)
            } else {
                css_box_length_px(value_2, font_size, false)
            };

            {
                match parsed.filter(|line_height| *line_height > 0.0) {
                    Some(line_height) => {
                        style.line_height = line_height;
                    },
                    ::core::option::Option::None => {},
                };
            }
        }
        _ => {}
    }
}

#[must_use]
fn flowchart_html_span_box_style(
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
    font_size: f64,
) -> HtmlSpanBoxStyle {
    let effective = flowchart_effective_node_class_names(class_defs, classes);
    let mut style = HtmlSpanBoxStyle::new(font_size);




    for (class, declarations) in class_defs {
        if !effective.contains(&class.as_str()) {
            continue;
        }

        for declaration in declarations {
            'over_declaration_3: for declaration_2 in flowchart_split_mermaid_style_decls(declaration) {
                let (property, value, _important) = {
                    let optional_property = parse_style_decl(declaration_2);

                    match optional_property {
                        Some((matched_property, matched_value, matched_important)) => (matched_property, matched_value, matched_important),
                        ::core::option::Option::None => continue 'over_declaration_3,
                    }
                };

                apply_html_span_box_decl(&mut style, property, value, font_size);
            }
        }
    }

    style
}

pub(crate) fn flowchart_apply_html_node_class_box_metrics(
    metrics: &mut TextMetrics,
    raw_label: &str,
    label_type: &str,
    text_style: &TextStyle,
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
) {
    if raw_label.is_empty() || class_defs.is_empty() {
        return;
    }

    let has_block_child = if label_type == "markdown" {
        crate::text::mermaid_markdown_wants_paragraph_wrap(raw_label)
    } else {

        true
    };
    let box_style = flowchart_html_span_box_style(class_defs, classes, text_style.font_size);
    let border = box_style.visible_border_width();
    let horizontal =
        box_style.margin.horizontal() + box_style.padding.horizontal() + border.horizontal();
    let vertical = box_style.margin.vertical() + box_style.padding.vertical() + border.vertical();

    match box_style.display {
        HtmlSpanDisplay::Inline if has_block_child && horizontal.abs() > f64::EPSILON => {



            metrics.width = metrics.width.max(horizontal.abs());
            metrics.height += 2.0 * box_style.line_height;
            metrics.line_count += 2;
        }
        HtmlSpanDisplay::Inline => {
            metrics.width = (metrics.width + horizontal).max(0.0);
        }
        HtmlSpanDisplay::InlineBlock => {
            metrics.width = (metrics.width + horizontal).max(0.0);
            metrics.height = (metrics.height + vertical).max(0.0);
        }
        HtmlSpanDisplay::Other => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_style_resolution_uses_the_compiled_invalid_winner() {
        let base = TextStyle {
            font_size: 16.0,
            ..TextStyle::default()
        };
        for (source, expected) in [
            ("font-size:32px,font-size:invalid", 16.0),
            ("font-size:32px !important,font-size:invalid", 32.0),
            ("font-size:32px,font-size:invalid !important", 16.0),
        ] {
            let style = flowchart_effective_text_style_for_classes(
                &base,
                &IndexMap::new(),
                &[],
                &[source.to_owned()],
            )
            .into_owned();
            assert_eq!(style.font_size, expected, "{source}");
        }
    }

    #[test]
    fn swimlane_label_rect_uses_only_the_first_concatenated_edge_style() {
        let base = TextStyle::default();
        let default_styles = vec![
            "font-size:24px".to_string(),
            "font-weight:bold".to_string(),
            "font-size:20px".to_string(),
        ];
        let edge_styles = vec![
            "font-size:12px".to_string(),
            "font-style:italic".to_string(),
        ];

        let style = flowchart_swimlane_label_rect_text_style(&base, &default_styles, &edge_styles);

        assert_eq!(style.font_size, 24.0);
        assert_eq!(style.font_weight, None);
        assert_eq!(style.font_style, None);
    }

    #[test]
    fn swimlane_label_rect_falls_back_to_the_first_edge_style() {
        let base = TextStyle::default();
        let edge_styles = vec![
            "font-size:18px,font-style:italic".to_string(),
            "font-size:30px".to_string(),
        ];

        let style = flowchart_swimlane_label_rect_text_style(&base, &[], &edge_styles);

        assert_eq!(style.font_size, 18.0);
        assert_eq!(style.font_style.as_deref(), Some("italic"));
    }

    #[test]
    fn edge_label_text_style_applies_class_then_default_then_edge_declarations() {
        let base = TextStyle::default();
        let class_defs = IndexMap::from([(
            "accent".to_string(),
            vec!["font-size:18px,font-style:italic".to_string()],
        )]);
        let classes = vec!["accent".to_string()];
        let default_styles = vec!["font-size:24px,font-weight:bold".to_string()];
        let edge_styles = vec!["font-size:12px,font-style:normal".to_string()];

        let style = flowchart_effective_edge_label_text_style(
            &base,
            &class_defs,
            &classes,
            &default_styles,
            &edge_styles,
        );

        assert_eq!(style.font_size, 12.0);
        assert_eq!(style.font_weight.as_deref(), Some("700"));
        assert_eq!(style.font_style.as_deref(), Some("normal"));
    }

    #[test]
    fn terminal_foreground_tracks_class_inline_and_edge_label_winners() {
        let base = TextStyle::default();
        let class_defs =
            IndexMap::from([("accent".to_string(), vec!["color:#0f172a".to_string()])]);
        let classes = vec!["accent".to_string()];
        let inline = vec!["color:#f43f5e".to_string()];
        let node = flowchart_effective_text_style_for_classes_with_provenance(
            &base,
            &class_defs,
            &classes,
            &inline,
        );
        let node_foreground = node.terminal_foreground().expect("node color winner");
        assert_eq!(node_foreground.value(), "#f43f5e");
        assert_eq!(
            node_foreground.provenance(),
            FlowchartTerminalForegroundProvenance::InlineStyle
        );

        let edge = flowchart_effective_edge_label_text_style_with_provenance(
            &base,
            &class_defs,
            &classes,
            &["color:#2563eb".to_string()],
            &["color:#16a34a".to_string()],
        );
        let edge_foreground = edge.terminal_foreground().expect("edge color winner");
        assert_eq!(edge_foreground.value(), "#16a34a");
        assert_eq!(
            edge_foreground.provenance(),
            FlowchartTerminalForegroundProvenance::EdgeLabelStyle
        );

        let important_classes = IndexMap::from([(
            "important".to_string(),
            vec!["color:#7c3aed !important".to_string()],
        )]);
        let important = flowchart_effective_text_style_for_classes_with_provenance(
            &base,
            &important_classes,
            &["important".to_string()],
            &["color:#f97316".to_string()],
        );
        let important_foreground = important
            .terminal_foreground()
            .expect("important class color winner");
        assert_eq!(important_foreground.value(), "#7c3aed");
        assert_eq!(
            important_foreground.provenance(),
            FlowchartTerminalForegroundProvenance::AssignedClass
        );
    }

    #[test]
    fn text_style_resolution_keeps_important_typography_ahead_of_later_normal_declarations() {
        let base = TextStyle::default();
        let class_defs = IndexMap::from([(
            "strong".to_string(),
            vec!["font-size:30px !important,font-family:Excalifont !important".to_string()],
        )]);
        let resolution = flowchart_effective_text_style_for_classes_with_provenance(
            &base,
            &class_defs,
            &["strong".to_string()],
            &["font-size:10px,font-family:Arial".to_string()],
        );

        assert_eq!(resolution.font_size, 30.0);
        assert_eq!(resolution.font_family.as_deref(), Some("Excalifont"));
    }

    #[test]
    fn text_style_resolution_applies_source_size_when_family_matches_the_base() {
        let base = TextStyle {
            font_family: Some("Excalifont".to_string()),
            ..TextStyle::default()
        };
        let class_defs = IndexMap::from([(
            "accent".to_string(),
            vec!["font-family:Excalifont,font-size:18px".to_string()],
        )]);
        let classes = vec!["accent".to_string()];

        let resolution = flowchart_effective_text_style_for_classes_with_provenance(
            &base,
            &class_defs,
            &classes,
            &[],
        );

        assert_eq!(resolution.font_family.as_deref(), Some("Excalifont"));
        assert_eq!(resolution.font_size, 18.0);
    }

    #[test]
    fn text_style_resolution_uses_source_spelled_label_property_identity() {
        let base = TextStyle {
            font_size: 26.0,
            ..TextStyle::default()
        };
        let class_defs = IndexMap::from([(
            "accent".to_string(),
            vec!["FONT-SIZE:22px,Font-Family:Xiaolai SC".to_string()],
        )]);
        let classes = vec!["accent".to_string()];

        let resolution = flowchart_effective_text_style_for_classes_with_provenance(
            &base,
            &class_defs,
            &classes,
            &[],
        );

        assert_eq!(resolution.font_size, 26.0);
        assert_eq!(resolution.font_family, None);
    }

    #[test]
    fn text_style_resolution_uses_the_semantic_value_before_important() {
        let base = TextStyle::default();
        let class_defs = IndexMap::from([(
            "accent".to_string(),
            vec!["font-family:\"Xiaolai SC\" !important,font-weight:bolder !important".to_string()],
        )]);
        let classes = vec!["accent".to_string()];

        let resolution = flowchart_effective_text_style_for_classes_with_provenance(
            &base,
            &class_defs,
            &classes,
            &[],
        );

        assert_eq!(resolution.font_family.as_deref(), Some("\"Xiaolai SC\""));
        assert_eq!(resolution.font_weight.as_deref(), Some("700"));
    }

    #[test]
    fn text_style_resolution_resolves_winners_against_the_parent() {
        let base = TextStyle {
            font_weight: Some("700".to_owned()),
            ..TextStyle::default()
        };
        for (declarations, expected_size, expected_weight) in [
            ("font-size:2em,font-size:150%", 24.0, "700"),
            ("font-size:32px,font-size:inherit", 16.0, "700"),
            (
                "font-size:32px,font-size:unset !important,font-size:48px",
                16.0,
                "700",
            ),
            (
                "font-size:24px,font-size:invalid !important,font-size:30px",
                16.0,
                "700",
            ),
            ("font-weight:lighter,font-weight:lighter", 16.0, "400"),
            ("font-weight:900,font-weight:invalid", 16.0, "700"),
        ] {
            let style = flowchart_effective_text_style_for_classes(
                &base,
                &IndexMap::new(),
                &[],
                &[declarations.to_owned()],
            );
            assert_eq!(style.font_size, expected_size, "{declarations}");
            assert_eq!(
                style.font_weight.as_deref(),
                Some(expected_weight),
                "{declarations}"
            );
        }
    }
}
