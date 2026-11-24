use crate::config::{config_bool, config_f64_css_px as config_f64};
use crate::model::{
    XyChartDiagramLayout, XyChartDrawableElem, XyChartPathData, XyChartRectData, XyChartTextData,
};
use crate::text::{TextMeasurer, TextStyle};
use crate::{Error, Result};
use merman_model::families::xy_chart::{
    XyChartAxisModel, XyChartDiagramRenderModel, XyChartPlotModel, XyChartPlotType,
};
use serde_json::Value;
use std::fmt::Write as _;

mod css_binding;
mod paint;
mod series;
mod theme;

pub(crate) use paint::{
    XyChartPaintPlan, XyChartPaintReceipt, XyChartPaintTerminalId, text_is_visible,
};

pub(crate) use series::{XyChartSeriesPaintPlan, XyChartSeriesPaintReceipt};
pub(crate) use theme::{XyChartTextRole, XyChartTypographyThemePlan};

#[derive(Debug, Clone)]
struct AxisThemeConfig {
    title_color: String,
    label_color: String,
    tick_color: String,
    axis_line_color: String,
}

#[derive(Debug, Clone)]
struct AxisConfig {
    show_label: bool,
    label_font_size: f64,
    label_font_weight: Option<String>,
    label_padding: f64,
    show_title: bool,
    title_font_size: f64,
    title_font_weight: Option<String>,
    title_padding: f64,
    show_tick: bool,
    tick_length: f64,
    tick_width: f64,
    show_axis_line: bool,
    axis_line_width: f64,
    label_rotation: f64,
}

#[derive(Debug, Clone)]
struct ChartConfig {
    width: f64,
    height: f64,
    plot_reserved_space_percent: f64,
    show_data_label: bool,
    show_data_label_outside_bar: bool,
    show_title: bool,
    title_font_size: f64,
    title_font_weight: Option<String>,
    title_padding: f64,
    show_legend: bool,
    legend_font_size: f64,
    legend_font_weight: Option<String>,
    legend_padding: f64,
    chart_orientation: String,
    x_axis: AxisConfig,
    y_axis: AxisConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AxisPosition {
    Left,
    Bottom,
    Top,
}

#[derive(Debug, Clone, Copy)]
struct Dimension {
    width: f64,
    height: f64,
}

type Point = crate::geom::Point;

fn finite_geometry(values: impl IntoIterator<Item = f64>) -> Result<()> {
    if values.into_iter().all(f64::is_finite) {
        Ok(())
    } else {
        Err(Error::InvalidModel {
            message: "XY Chart geometry cannot be represented with finite coordinates".to_string(),
        })
    }
}

fn reserved_plot_span(span: f64, percent: f64) -> Result<f64> {
    finite_geometry([span, percent])?;
    let product = span * percent;

    let reserved = if product.is_finite() {
        product / 100.0
    } else {
        span * (percent / 100.0)
    };
    finite_geometry([reserved])?;
    Ok(reserved.floor())
}

#[must_use]
fn linear_scale_value(value: f64, domain: (f64, f64), range: (f64, f64)) -> f64 {
    let (d0, d1) = domain;
    let (r0, r1) = range;

    if <::core::primitive::f64 as ::core::cmp::PartialOrd>::partial_cmp(&d0, &d1) == ::core::option::Option::Some(::core::cmp::Ordering::Equal) {
        return r0 + (r1 - r0) * 0.5;
    }

    let numerator = value - d0;
    let denominator = d1 - d0;
    let t = if numerator.is_finite() && denominator.is_finite() {
        numerator / denominator
    } else {
        (value * 0.5 - d0 * 0.5) / (d1 * 0.5 - d0 * 0.5)
    };
    r0 + t * (r1 - r0)
}

#[must_use]
fn pt(x: f64, y: f64) -> Point {
    crate::geom::point(x, y)
}

#[derive(Debug, Clone, Copy)]
struct BoundingRect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[must_use]
fn is_ref_only_object(v: &Value) -> bool {
    v.as_object()
        .is_some_and(|m| m.len() == 1 && m.contains_key("$ref"))
}

#[must_use]
fn default_axis_config() -> AxisConfig {
    AxisConfig {
        show_label: true,
        label_font_size: 14.0,
        label_font_weight: None,
        label_padding: 5.0,
        show_title: true,
        title_font_size: 16.0,
        title_font_weight: None,
        title_padding: 5.0,
        show_tick: true,
        tick_length: 5.0,
        tick_width: 2.0,
        show_axis_line: true,
        axis_line_width: 2.0,
        label_rotation: 0.0,
    }
}

#[must_use]
fn parse_axis_config(effective_config: &Value, axis_key: &str) -> AxisConfig {
    let base = default_axis_config();

    let Some(v) = effective_config
        .get("xyChart")
        .and_then(|c| c.get(axis_key))
    else {
        return base;
    };





    if !v.is_object() || is_ref_only_object(v) {
        return base;
    }

    AxisConfig {
        show_label: match config_bool(effective_config, &["xyChart", axis_key, "showLabel"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => base.show_label,
        },
        label_font_size: match config_f64(effective_config, &["xyChart", axis_key, "labelFontSize"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => base.label_font_size,
        },
        label_font_weight: None,
        label_padding: match config_f64(effective_config, &["xyChart", axis_key, "labelPadding"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => base.label_padding,
        },
        show_title: match config_bool(effective_config, &["xyChart", axis_key, "showTitle"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => base.show_title,
        },
        title_font_size: match config_f64(effective_config, &["xyChart", axis_key, "titleFontSize"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => base.title_font_size,
        },
        title_font_weight: None,
        title_padding: match config_f64(effective_config, &["xyChart", axis_key, "titlePadding"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => base.title_padding,
        },
        show_tick: match config_bool(effective_config, &["xyChart", axis_key, "showTick"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => base.show_tick,
        },
        tick_length: match config_f64(effective_config, &["xyChart", axis_key, "tickLength"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => base.tick_length,
        },
        tick_width: match config_f64(effective_config, &["xyChart", axis_key, "tickWidth"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => base.tick_width,
        },
        show_axis_line: match config_bool(effective_config, &["xyChart", axis_key, "showAxisLine"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => base.show_axis_line,
        },
        axis_line_width: match config_f64(effective_config, &["xyChart", axis_key, "axisLineWidth"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => base.axis_line_width,
        },
        label_rotation: match config_f64(effective_config, &["xyChart", axis_key, "labelRotation"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => base.label_rotation,
        },
    }
}

#[must_use]
fn parse_chart_config(effective_config: &Value, model: &XyChartDiagramRenderModel) -> ChartConfig {
    ChartConfig {
        width: match config_f64(effective_config, &["xyChart", "width"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => 700.0,
        },
        height: match config_f64(effective_config, &["xyChart", "height"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => 500.0,
        },
        plot_reserved_space_percent: match config_f64(
            effective_config,
            &["xyChart", "plotReservedSpacePercent"],
        ) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => 50.0,
        },
        show_data_label: match config_bool(effective_config, &["xyChart", "showDataLabel"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => false,
        },
        show_data_label_outside_bar: match config_bool(
            effective_config,
            &["xyChart", "showDataLabelOutsideBar"],
        ) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => false,
        },
        show_title: match config_bool(effective_config, &["xyChart", "showTitle"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => true,
        },
        title_font_size: match config_f64(effective_config, &["xyChart", "titleFontSize"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => 20.0,
        },
        title_font_weight: None,
        title_padding: match config_f64(effective_config, &["xyChart", "titlePadding"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => 10.0,
        },
        show_legend: match config_bool(effective_config, &["xyChart", "showLegend"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => true,
        },
        legend_font_size: match config_f64(effective_config, &["xyChart", "legendFontSize"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => 14.0,
        }
            .max(1.0),
        legend_font_weight: None,
        legend_padding: match config_f64(effective_config, &["xyChart", "legendPadding"]) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => 10.0,
        }
            .max(0.0),
        chart_orientation: match model.orientation.as_str() {
            "horizontal" => "horizontal".to_string(),
            _ => "vertical".to_string(),
        },
        x_axis: parse_axis_config(effective_config, "xAxis"),
        y_axis: parse_axis_config(effective_config, "yAxis"),
    }
}

#[must_use]
fn max_text_dimension(
    texts: &[String],
    font_size: f64,
    font_family: &str,
    font_weight: Option<&str>,
    measurer: &dyn TextMeasurer,
) -> Dimension {
    let style = TextStyle {
        font_family: Some(font_family.to_string()),
        font_size,
        font_weight: font_weight.map(str::to_owned),
        ..Default::default()
    };
    let mut max_w: f64 = 0.0;
    let mut max_h: f64 = 0.0;

    match texts {
        [] => {
            return Dimension {
                width: 0.0,
                height: 0.0,
            };
        },
        [_, ..] => {},
    }



    for t in texts {
        max_w = max_w.max(measurer.measure_svg_text_bounding_client_rect_width_px(t, &style));


        max_h = max_h.max(measurer.measure_svg_raw_text_bbox_height_px(t, &style));
    }

    Dimension {
        width: max_w,
        height: max_h,
    }
}

#[must_use]
fn single_text_height(
    text: &str,
    font_size: f64,
    font_family: &str,
    font_weight: Option<&str>,
    measurer: &dyn TextMeasurer,
) -> f64 {
    if text.trim().is_empty() {
        return 0.0;
    }

    let style = TextStyle {
        font_family: Some(font_family.to_string()),
        font_size,
        font_weight: font_weight.map(str::to_owned),
        ..Default::default()
    };
    measurer.measure_svg_raw_text_bbox_height_px(text, &style)
}

const LEGEND_MARKER_TO_FONT_RATIO: f64 = 0.75;
const LEGEND_ITEM_SPACING_TO_FONT_RATIO: f64 = 0.5;
const LEGEND_MARKER_SPACING_TO_FONT_RATIO: f64 = 0.35;

#[must_use]
fn calculate_legend_space(
    plots: &[(usize, &XyChartPlotModel)],
    chart_config: &ChartConfig,
    available_space: Dimension,
    font_family: &str,
    measurer: &dyn TextMeasurer,
) -> Dimension {
    match plots {
        [] => {
            return Dimension {
                width: 0.0,
                height: 0.0,
            };
        },
        [_, ..] => {},
    }

    let titles = plots
        .iter()
        .filter_map(|(_, plot)| plot.title.clone())
        .collect::<Vec<_>>();
    let text_dimension = max_text_dimension(
        &titles,
        chart_config.legend_font_size,
        font_family,
        chart_config.legend_font_weight.as_deref(),
        measurer,
    );
    let marker_size = chart_config.legend_font_size * LEGEND_MARKER_TO_FONT_RATIO;
    let marker_spacing = chart_config.legend_font_size * LEGEND_MARKER_SPACING_TO_FONT_RATIO;
    let item_spacing = chart_config.legend_font_size * LEGEND_ITEM_SPACING_TO_FONT_RATIO;
    let width =
        chart_config.legend_padding * 2.0 + marker_size + marker_spacing + text_dimension.width;
    let height = chart_config.legend_padding * 2.0
        + plots.len() as f64 * chart_config.legend_font_size
        + plots.len().saturating_sub(1) as f64 * item_spacing;

    if width <= available_space.width && height <= available_space.height {
        Dimension { width, height }
    } else {
        Dimension {
            width: 0.0,
            height: 0.0,
        }
    }
}

fn legend_drawable_elements(
    plots: &[(usize, &XyChartPlotModel)],
    origin: Point,
    chart_config: &ChartConfig,
    legend_text_color: &str,
    series_paint: &XyChartSeriesPaintPlan,
) -> Result<Vec<XyChartDrawableElem>> {
    match plots {
        [] => {
            return Ok(Vec::new());
        },
        [_, ..] => {},
    }

    let marker_size = chart_config.legend_font_size * LEGEND_MARKER_TO_FONT_RATIO;
    let marker_spacing = chart_config.legend_font_size * LEGEND_MARKER_SPACING_TO_FONT_RATIO;
    let item_spacing = chart_config.legend_font_size * LEGEND_ITEM_SPACING_TO_FONT_RATIO;
    let row_height = chart_config.legend_font_size + item_spacing;
    let start_x = origin.x + chart_config.legend_padding;
    let start_y = origin.y + chart_config.legend_padding;
    let mut bar_markers = Vec::new();
    let mut line_markers = Vec::new();
    let mut labels = Vec::with_capacity(plots.len());

    for (legend_index, (plot_index, plot)) in plots.iter().enumerate() {
        let fill_color = series_paint
            .fill_css(*plot_index)
            .ok_or_else(|| Error::InvalidModel {
                message: "XY Chart legend fill plan is incomplete".to_string(),
            })?;
        let stroke_color =
            series_paint
                .stroke_css(*plot_index)
                .ok_or_else(|| Error::InvalidModel {
                    message: "XY Chart legend stroke plan is incomplete".to_string(),
                })?;

        match plot.plot_type {
            XyChartPlotType::Bar => bar_markers.push(XyChartRectData {
                x: start_x,
                y: start_y + legend_index as f64 * row_height,
                width: marker_size,
                height: marker_size,
                fill: fill_color.to_string(),
                stroke_fill: stroke_color.to_string(),
                stroke_width: series_paint
                    .stroke_width(*plot_index)
                    .expect("resolved series width"),
            }),
            XyChartPlotType::Line => {
                let marker_y = start_y + legend_index as f64 * row_height + marker_size / 2.0;
                line_markers.push(XyChartPathData {
                    path: format!(
                        "M {start_x},{marker_y} L {},{marker_y}",
                        start_x + marker_size
                    ),
                    fill: (fill_color != "none").then(|| fill_color.to_string()),
                    stroke_fill: stroke_color.to_string(),
                    stroke_width: series_paint
                        .stroke_width(*plot_index)
                        .expect("resolved series width"),
                });
            }
        }

        labels.push(XyChartTextData {
            text: match plot.title.clone() {
                ::core::option::Option::Some(option_value) => option_value,
                ::core::option::Option::None => ::std::string::String::new(),
            },
            x: start_x + marker_size + marker_spacing,
            y: start_y + legend_index as f64 * row_height + marker_size / 2.0,
            fill: legend_text_color.to_string(),
            font_size: chart_config.legend_font_size,
            rotation: 0.0,
            vertical_pos: "middle".to_string(),
            horizontal_pos: "left".to_string(),
        });
    }

    Ok(vec![
        XyChartDrawableElem::Rect {
            group_texts: vec!["legend".to_string(), "markers".to_string()],
            data: bar_markers,
        },
        XyChartDrawableElem::Path {
            group_texts: vec!["legend".to_string(), "markers".to_string()],
            data: line_markers,
        },
        XyChartDrawableElem::Text {
            group_texts: vec!["legend".to_string(), "label".to_string()],
            data: labels,
        },
    ])
}

#[must_use]
fn d3_ticks(start: f64, stop: f64, count: usize) -> Vec<f64> {
    #[must_use]
    fn tick_spec(start: f64, stop: f64, count: f64) -> Option<(i64, i64, f64)> {
        if count <= 0.0 {
            return None;
        }

        let span = stop - start;
        let step = if span.is_finite() {
            span / count.max(0.0)
        } else {
            stop / count - start / count
        };

        if !step.is_finite() || ::core::matches!(::core::primitive::f64::classify(step), ::core::num::FpCategory::Zero) {
            return None;
        }

        let power = step.log10().floor();
        let error = step / 10f64.powf(power);
        let e10 = 50f64.sqrt();
        let e5 = 10f64.sqrt();
        let e2 = 2f64.sqrt();
        let factor = if error >= e10 {
            10.0
        } else if error >= e5 {
            5.0
        } else if error >= e2 {
            2.0
        } else {
            1.0
        };

        let (i1, i2, inc) = if power < 0.0 {
            let inc = 10f64.powf(-power) / factor;
            let mut i1 = (start * inc).round() as i64;
            let mut i2 = (stop * inc).round() as i64;

            if (i1 as f64) / inc < start {
                i1 += 1;
            }

            if (i2 as f64) / inc > stop {
                i2 -= 1;
            }

            (i1, i2, -inc)
        } else {
            let inc = 10f64.powf(power) * factor;
            let mut i1 = (start / inc).round() as i64;
            let mut i2 = (stop / inc).round() as i64;

            if (i1 as f64) * inc < start {
                i1 += 1;
            }

            if (i2 as f64) * inc > stop {
                i2 -= 1;
            }

            (i1, i2, inc)
        };

        if i2 < i1 && (0.5..2.0).contains(&count) {
            return tick_spec(start, stop, count * 2.0);
        }

        if !inc.is_finite() {
            return None;
        }

        if ::core::matches!(::core::primitive::f64::classify(inc), ::core::num::FpCategory::Zero) {
            return None;
        }

        Some((i1, i2, inc))
    }

    if !start.is_finite() || !stop.is_finite() {
        return Vec::new();
    }

    let count_2 = count as f64;

    if count_2 <= 0.0 {
        return Vec::new();
    }

    if <::core::primitive::f64 as ::core::cmp::PartialOrd>::partial_cmp(&start, &stop) == ::core::option::Option::Some(::core::cmp::Ordering::Equal) {
        return vec![start];
    }

    let reverse = stop < start;
    let (a, b) = if reverse {
        (stop, start)
    } else {
        (start, stop)
    };

    let (i1, i2, inc) = {
        let optional_i1 = tick_spec(a, b, count_2);

        match optional_i1 {
            Some((matched_i1, matched_i2, matched_inc)) => (matched_i1, matched_i2, matched_inc),
            ::core::option::Option::None => return Vec::new(),
        }
    };

    if i2 < i1 {
        return Vec::new();
    }

    let n = (i2 - i1 + 1).max(0) as usize;
    let mut out = Vec::with_capacity(n);

    if reverse {
        if inc < 0.0 {
            for i in 0..n {
                out.push((i2 - i as i64) as f64 / -inc);
            }
        } else {
            for i in 0..n {
                out.push((i2 - i as i64) as f64 * inc);
            }
        }
    } else if inc < 0.0 {
        for i in 0..n {
            out.push((i1 + i as i64) as f64 / -inc);
        }
    } else {
        for i in 0..n {
            out.push((i1 + i as i64) as f64 * inc);
        }
    }

    out
}

#[must_use]
fn build_tick_values(kind: &AxisKind, axis_position: AxisPosition) -> Vec<String> {
    match kind {
        AxisKind::Band { categories } => categories.clone(),
        AxisKind::Linear { domain } => {
            let (mut a, mut b) = *domain;

            if matches!(axis_position, AxisPosition::Left) {
                std::mem::swap(&mut a, &mut b);
            }

            d3_ticks(a, b, 10)
                .into_iter()
                .map(|v| format!("{v}"))
                .collect()
        }
    }
}

#[derive(Debug, Clone)]
enum AxisKind {
    Band { categories: Vec<String> },
    Linear { domain: (f64, f64) },
}

#[derive(Debug, Clone)]
struct Axis {
    kind: AxisKind,
    axis_config: AxisConfig,
    axis_theme: AxisThemeConfig,
    axis_position: AxisPosition,
    tick_values: Vec<String>,
    bounding_rect: BoundingRect,
    range: (f64, f64),
    show_title: bool,
    show_label: bool,
    show_tick: bool,
    show_axis_line: bool,
    outer_padding: f64,
    title: String,
    title_text_height: f64,
    label_dimension: Dimension,
}

impl Axis {
    #[must_use]
    fn new(
        kind: AxisKind,
        axis_config: AxisConfig,
        axis_theme: AxisThemeConfig,
        title: String,
    ) -> Self {
        Self {
            kind,
            axis_config,
            axis_theme,
            axis_position: AxisPosition::Left,
            tick_values: Vec::new(),
            bounding_rect: BoundingRect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            range: (0.0, 10.0),
            show_title: false,
            show_label: false,
            show_tick: false,
            show_axis_line: false,
            outer_padding: 0.0,
            title,
            title_text_height: 0.0,
            label_dimension: Dimension {
                width: 0.0,
                height: 0.0,
            },
        }
    }

    fn set_axis_position(&mut self, pos: AxisPosition) {
        self.axis_position = pos;
        self.tick_values = build_tick_values(&self.kind, self.axis_position);
        let range = self.range;
        self.set_range(range);
    }

    fn set_range(&mut self, range: (f64, f64)) {
        self.range = range;

        if matches!(self.axis_position, AxisPosition::Left) {
            self.bounding_rect.height = range.1 - range.0;
        } else {
            self.bounding_rect.width = range.1 - range.0;
        }
    }

    fn set_bounding_box_xy(&mut self, pt: Point) {
        self.bounding_rect.x = pt.x;
        self.bounding_rect.y = pt.y;
    }

    #[must_use]
    fn get_range(&self) -> (f64, f64) {
        (
            self.range.0 + self.outer_padding,
            self.range.1 - self.outer_padding,
        )
    }

    #[must_use]
    fn tick_values(&self) -> &[String] {
        &self.tick_values
    }

    #[must_use]
    fn tick_distance(&self) -> f64 {
        let (a, b) = self.get_range();
        let span = (a - b).abs();

        if self.tick_values.is_empty() {
            return 0.0;
        }

        span / (self.tick_values.len() as f64)
    }

    #[must_use]
    fn get_scale_value(&self, value: &str) -> f64 {
        match &self.kind {
            AxisKind::Band { categories } => {
                let (a, b) = self.get_range();
                let n = categories.len();

                match n {
                    0 => {
                        return a;
                    },
                    _ => {},
                }

                match n {
                    1 => {
                        return a + (b - a) * 0.5;
                    },
                    _ => {},
                }

                let step = (b - a) / ((n - 1) as f64);
                let idx = match categories.iter().position(|c| c == value) {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => 0,
                };
                a + step * (idx as f64)
            }
            AxisKind::Linear { domain } => {
                let v = {
                    let outcome_v = value.parse::<f64>();

                    match outcome_v {
                        Ok(matched_v) => matched_v,
                        ::core::result::Result::Err(_) => return self.get_range().0,
                    }
                };

                if v.is_nan() {
                    return f64::NAN;
                }

                let (mut d0, mut d1) = *domain;

                if matches!(self.axis_position, AxisPosition::Left) {
                    std::mem::swap(&mut d0, &mut d1);
                }

                linear_scale_value(v, (d0, d1), self.get_range())
            }
        }
    }

    #[must_use]
    fn get_scale_number(&self, value: Option<f64>) -> f64 {
        let value_2 = match value {
            Some(matched_value) => matched_value,
            ::core::option::Option::None => return self.get_scale_value("NaN"),
        };

        match &self.kind {
            AxisKind::Linear { domain } => {
                if value_2.is_nan() {
                    return f64::NAN;
                }

                let (mut d0, mut d1) = *domain;

                if matches!(self.axis_position, AxisPosition::Left) {
                    std::mem::swap(&mut d0, &mut d1);
                }

                linear_scale_value(value_2, (d0, d1), self.get_range())
            }
            AxisKind::Band { .. } => self.get_scale_value(&format!("{value}", value = value_2)),
        }
    }

    fn recalculate_outer_padding_to_draw_bar(&mut self) {
        const BAR_WIDTH_TO_TICK_WIDTH_RATIO: f64 = 0.7;
        let target = BAR_WIDTH_TO_TICK_WIDTH_RATIO * self.tick_distance();

        if target > self.outer_padding * 2.0 {
            self.outer_padding = (target / 2.0).floor();
        }
    }

    #[must_use]
    fn normalized_label_rotation_rad(&self) -> f64 {
        let rotation = self.axis_config.label_rotation;

        if (-90.0..=90.0).contains(&rotation) {
            rotation.to_radians()
        } else {
            0.0
        }
    }

    #[must_use]
    fn label_rotation_offset(&self, reference: AxisLabelDimensionRef) -> f64 {
        let rotation = self.normalized_label_rotation_rad();

        if ::core::matches!(::core::primitive::f64::classify(rotation), ::core::num::FpCategory::Zero) {
            return 0.0;
        }

        let dimension = match reference {
            AxisLabelDimensionRef::Width => self.label_dimension.width,
            AxisLabelDimensionRef::Height => self.label_dimension.height,
        };
        rotation.sin() * dimension / 2.0
    }

    fn calculate_space(
        &mut self,
        available: Dimension,
        font_family: &str,
        measurer: &dyn TextMeasurer,
    ) -> Dimension {
        self.show_title = false;
        self.show_label = false;
        self.show_tick = false;
        self.show_axis_line = false;
        self.outer_padding = 0.0;
        self.title_text_height = 0.0;
        self.label_dimension = Dimension {
            width: 0.0,
            height: 0.0,
        };

        if matches!(self.axis_position, AxisPosition::Left) {
            let mut available_width = available.width;

            if self.axis_config.show_axis_line && available_width > self.axis_config.axis_line_width
            {
                available_width -= self.axis_config.axis_line_width;
                self.show_axis_line = true;
            }

            if self.axis_config.show_label {
                let ticks = self.tick_values();
                let dim = max_text_dimension(
                    ticks,
                    self.axis_config.label_font_size,
                    font_family,
                    self.axis_config.label_font_weight.as_deref(),
                    measurer,
                );
                self.label_dimension = dim;
                let max_padding = 0.2 * available.height;
                self.outer_padding = (dim.height / 2.0).min(max_padding);
                let width_required = dim.width + self.axis_config.label_padding * 2.0;

                if width_required <= available_width {
                    available_width -= width_required;
                    self.show_label = true;
                }
            }

            if self.axis_config.show_tick && available_width >= self.axis_config.tick_length {
                self.show_tick = true;
                available_width -= self.axis_config.tick_length;
            }

            if self.axis_config.show_title && !self.title.is_empty() {
                let title_height = single_text_height(
                    &self.title,
                    self.axis_config.title_font_size,
                    font_family,
                    self.axis_config.title_font_weight.as_deref(),
                    measurer,
                );
                let width_required = title_height + self.axis_config.title_padding * 2.0;
                self.title_text_height = title_height;

                if width_required <= available_width {
                    available_width -= width_required;
                    self.show_title = true;
                }
            }

            self.bounding_rect.width = available.width - available_width;
            self.bounding_rect.height = available.height;
            Dimension {
                width: self.bounding_rect.width,
                height: self.bounding_rect.height,
            }
        } else {
            let mut available_height = available.height;

            if self.axis_config.show_axis_line
                && available_height > self.axis_config.axis_line_width
            {
                available_height -= self.axis_config.axis_line_width;
                self.show_axis_line = true;
            }

            if self.axis_config.show_label {
                let ticks = self.tick_values();
                let dim = max_text_dimension(
                    ticks,
                    self.axis_config.label_font_size,
                    font_family,
                    self.axis_config.label_font_weight.as_deref(),
                    measurer,
                );
                self.label_dimension = dim;
                let max_padding = 0.2 * available.width;
                self.outer_padding = (dim.width / 2.0).min(max_padding);
                let mut height_required = dim.height;
                let rotation = self.normalized_label_rotation_rad();

                if matches!(self.axis_position, AxisPosition::Bottom) && !::core::matches!(::core::primitive::f64::classify(rotation), ::core::num::FpCategory::Zero) {
                    height_required = height_required.max(
                        (rotation.sin() * dim.width).abs() + (rotation.cos() * dim.height).abs(),
                    );
                }

                height_required += self.axis_config.label_padding * 2.0;

                if height_required <= available_height {
                    available_height -= height_required;
                    self.show_label = true;
                }
            }

            if self.axis_config.show_tick && available_height >= self.axis_config.tick_length {
                self.show_tick = true;
                available_height -= self.axis_config.tick_length;
            }

            if self.axis_config.show_title && !self.title.is_empty() {
                let title_height = single_text_height(
                    &self.title,
                    self.axis_config.title_font_size,
                    font_family,
                    self.axis_config.title_font_weight.as_deref(),
                    measurer,
                );
                let height_required = title_height + self.axis_config.title_padding * 2.0;
                self.title_text_height = title_height;

                if height_required <= available_height {
                    available_height -= height_required;
                    self.show_title = true;
                }
            }

            self.bounding_rect.width = available.width;
            self.bounding_rect.height = available.height - available_height;
            Dimension {
                width: self.bounding_rect.width,
                height: self.bounding_rect.height,
            }
        }
    }

    #[must_use]
    fn drawable_elements(&self) -> Vec<XyChartDrawableElem> {
        match self.axis_position {
            AxisPosition::Left => self.drawable_elements_for_left_axis(),
            AxisPosition::Bottom => self.drawable_elements_for_bottom_axis(),
            AxisPosition::Top => self.drawable_elements_for_top_axis(),
        }
    }

    #[must_use]
    fn drawable_elements_for_left_axis(&self) -> Vec<XyChartDrawableElem> {
        let mut out: Vec<XyChartDrawableElem> = Vec::new();

        if self.show_axis_line {
            let x = self.bounding_rect.x + self.bounding_rect.width
                - self.axis_config.axis_line_width / 2.0;
            out.push(XyChartDrawableElem::Path {
                group_texts: vec!["left-axis".to_string(), "axisl-line".to_string()],
                data: vec![XyChartPathData {
                    path: format!(
                        "M {x},{} L {x},{} ",
                        self.bounding_rect.y,
                        self.bounding_rect.y + self.bounding_rect.height
                    ),
                    fill: None,
                    stroke_fill: self.axis_theme.axis_line_color.clone(),
                    stroke_width: self.axis_config.axis_line_width,
                }],
            });
        }

        if self.show_label {
            let x = self.bounding_rect.x + self.bounding_rect.width
                - (if self.show_label {
                    self.axis_config.label_padding
                } else {
                    0.0
                })
                - (if self.show_tick {
                    self.axis_config.tick_length
                } else {
                    0.0
                })
                - (if self.show_axis_line {
                    self.axis_config.axis_line_width
                } else {
                    0.0
                });
            let ticks = self.tick_values();
            out.push(XyChartDrawableElem::Text {
                group_texts: vec!["left-axis".to_string(), "label".to_string()],
                data: ticks
                    .iter()
                    .map(|t| XyChartTextData {
                        text: t.clone(),
                        x,
                        y: self.get_scale_value(t),
                        fill: self.axis_theme.label_color.clone(),
                        font_size: self.axis_config.label_font_size,
                        rotation: 0.0,
                        vertical_pos: "middle".to_string(),
                        horizontal_pos: "right".to_string(),
                    })
                    .collect(),
            });
        }

        if self.show_tick {
            let x = self.bounding_rect.x + self.bounding_rect.width
                - (if self.show_axis_line {
                    self.axis_config.axis_line_width
                } else {
                    0.0
                });
            let ticks = self.tick_values();
            out.push(XyChartDrawableElem::Path {
                group_texts: vec!["left-axis".to_string(), "ticks".to_string()],
                data: ticks
                    .iter()
                    .map(|t| {
                        let y = self.get_scale_value(t);
                        XyChartPathData {
                            path: format!("M {x},{y} L {},{y}", x - self.axis_config.tick_length),
                            fill: None,
                            stroke_fill: self.axis_theme.tick_color.clone(),
                            stroke_width: self.axis_config.tick_width,
                        }
                    })
                    .collect(),
            });
        }

        if self.show_title {
            out.push(XyChartDrawableElem::Text {
                group_texts: vec!["left-axis".to_string(), "title".to_string()],
                data: vec![XyChartTextData {
                    text: self.title.clone(),
                    x: self.bounding_rect.x + self.axis_config.title_padding,
                    y: self.bounding_rect.y + self.bounding_rect.height / 2.0,
                    fill: self.axis_theme.title_color.clone(),
                    font_size: self.axis_config.title_font_size,
                    rotation: 270.0,
                    vertical_pos: "top".to_string(),
                    horizontal_pos: "center".to_string(),
                }],
            });
        }

        out
    }

    #[must_use]
    fn drawable_elements_for_bottom_axis(&self) -> Vec<XyChartDrawableElem> {
        let mut out: Vec<XyChartDrawableElem> = Vec::new();

        if self.show_axis_line {
            let y = self.bounding_rect.y + self.axis_config.axis_line_width / 2.0;
            out.push(XyChartDrawableElem::Path {
                group_texts: vec!["bottom-axis".to_string(), "axis-line".to_string()],
                data: vec![XyChartPathData {
                    path: format!(
                        "M {},{y} L {},{y}",
                        self.bounding_rect.x,
                        self.bounding_rect.x + self.bounding_rect.width
                    ),
                    fill: None,
                    stroke_fill: self.axis_theme.axis_line_color.clone(),
                    stroke_width: self.axis_config.axis_line_width,
                }],
            });
        }

        if self.show_label {
            let ticks = self.tick_values();
            out.push(XyChartDrawableElem::Text {
                group_texts: vec!["bottom-axis".to_string(), "label".to_string()],
                data: ticks
                    .iter()
                    .map(|t| XyChartTextData {
                        text: t.clone(),
                        x: self.get_scale_value(t)
                            + self.label_rotation_offset(AxisLabelDimensionRef::Height),
                        y: self.bounding_rect.y
                            + self.axis_config.label_padding
                            + (if self.show_tick {
                                self.axis_config.tick_length
                            } else {
                                0.0
                            })
                            + (if self.show_axis_line {
                                self.axis_config.axis_line_width
                            } else {
                                0.0
                            })
                            + self
                                .label_rotation_offset(AxisLabelDimensionRef::Width)
                                .abs(),
                        fill: self.axis_theme.label_color.clone(),
                        font_size: self.axis_config.label_font_size,
                        rotation: self.normalized_label_rotation_rad().to_degrees(),
                        vertical_pos: "top".to_string(),
                        horizontal_pos: "center".to_string(),
                    })
                    .collect(),
            });
        }

        if self.show_tick {
            let y = self.bounding_rect.y
                + (if self.show_axis_line {
                    self.axis_config.axis_line_width
                } else {
                    0.0
                });
            let ticks = self.tick_values();
            out.push(XyChartDrawableElem::Path {
                group_texts: vec!["bottom-axis".to_string(), "ticks".to_string()],
                data: ticks
                    .iter()
                    .map(|t| {
                        let x = self.get_scale_value(t);
                        XyChartPathData {
                            path: format!("M {x},{y} L {x},{}", y + self.axis_config.tick_length),
                            fill: None,
                            stroke_fill: self.axis_theme.tick_color.clone(),
                            stroke_width: self.axis_config.tick_width,
                        }
                    })
                    .collect(),
            });
        }

        if self.show_title {
            out.push(XyChartDrawableElem::Text {
                group_texts: vec!["bottom-axis".to_string(), "title".to_string()],
                data: vec![XyChartTextData {
                    text: self.title.clone(),
                    x: self.range.0 + (self.range.1 - self.range.0) / 2.0,
                    y: self.bounding_rect.y + self.bounding_rect.height
                        - self.axis_config.title_padding
                        - self.title_text_height,
                    fill: self.axis_theme.title_color.clone(),
                    font_size: self.axis_config.title_font_size,
                    rotation: 0.0,
                    vertical_pos: "top".to_string(),
                    horizontal_pos: "center".to_string(),
                }],
            });
        }

        out
    }

    #[must_use]
    fn drawable_elements_for_top_axis(&self) -> Vec<XyChartDrawableElem> {
        let mut out: Vec<XyChartDrawableElem> = Vec::new();

        if self.show_axis_line {
            let y = self.bounding_rect.y + self.bounding_rect.height
                - self.axis_config.axis_line_width / 2.0;
            out.push(XyChartDrawableElem::Path {
                group_texts: vec!["top-axis".to_string(), "axis-line".to_string()],
                data: vec![XyChartPathData {
                    path: format!(
                        "M {},{y} L {},{y}",
                        self.bounding_rect.x,
                        self.bounding_rect.x + self.bounding_rect.width
                    ),
                    fill: None,
                    stroke_fill: self.axis_theme.axis_line_color.clone(),
                    stroke_width: self.axis_config.axis_line_width,
                }],
            });
        }

        if self.show_label {
            let ticks = self.tick_values();
            out.push(XyChartDrawableElem::Text {
                group_texts: vec!["top-axis".to_string(), "label".to_string()],
                data: ticks
                    .iter()
                    .map(|t| XyChartTextData {
                        text: t.clone(),
                        x: self.get_scale_value(t),
                        y: self.bounding_rect.y
                            + (if self.show_title {
                                self.title_text_height + self.axis_config.title_padding * 2.0
                            } else {
                                0.0
                            })
                            + self.axis_config.label_padding,
                        fill: self.axis_theme.label_color.clone(),
                        font_size: self.axis_config.label_font_size,
                        rotation: 0.0,
                        vertical_pos: "top".to_string(),
                        horizontal_pos: "center".to_string(),
                    })
                    .collect(),
            });
        }

        if self.show_tick {
            let y = self.bounding_rect.y;
            let ticks = self.tick_values();
            out.push(XyChartDrawableElem::Path {
                group_texts: vec!["top-axis".to_string(), "ticks".to_string()],
                data: ticks
                    .iter()
                    .map(|t| {
                        let x = self.get_scale_value(t);
                        let y0 = y + self.bounding_rect.height
                            - (if self.show_axis_line {
                                self.axis_config.axis_line_width
                            } else {
                                0.0
                            });
                        let y1 = y + self.bounding_rect.height
                            - self.axis_config.tick_length
                            - (if self.show_axis_line {
                                self.axis_config.axis_line_width
                            } else {
                                0.0
                            });
                        XyChartPathData {
                            path: format!("M {x},{y0} L {x},{y1}"),
                            fill: None,
                            stroke_fill: self.axis_theme.tick_color.clone(),
                            stroke_width: self.axis_config.tick_width,
                        }
                    })
                    .collect(),
            });
        }

        if self.show_title {
            out.push(XyChartDrawableElem::Text {
                group_texts: vec!["top-axis".to_string(), "title".to_string()],
                data: vec![XyChartTextData {
                    text: self.title.clone(),
                    x: self.bounding_rect.x + self.bounding_rect.width / 2.0,
                    y: self.bounding_rect.y + self.axis_config.title_padding,
                    fill: self.axis_theme.title_color.clone(),
                    font_size: self.axis_config.title_font_size,
                    rotation: 0.0,
                    vertical_pos: "top".to_string(),
                    horizontal_pos: "center".to_string(),
                }],
            });
        }

        out
    }
}

#[derive(Debug, Clone, Copy)]
enum AxisLabelDimensionRef {
    Width,
    Height,
}

#[must_use]
fn line_path(points: &[(f64, f64)]) -> Option<String> {
    let (first, rest) = points.split_first()?;
    let mut out = String::with_capacity(points.len().saturating_mul(24).max(24));
    let _ = write!(&mut out, "M{},{}", first.0, first.1);

    match rest {
        [] => {
            out.push('Z');
            return Some(out);
        },
        [_, ..] => {},
    }

    for p in rest {
        let _ = write!(&mut out, "L{},{}", p.0, p.1);
    }

    Some(out)
}

/// Lays out a typed XYChart render model without a compatibility-JSON round trip.
pub(crate) fn layout_xychart_diagram_typed(
    model: &XyChartDiagramRenderModel,
    diagram_title: Option<&str>,
    series_paint: &XyChartSeriesPaintPlan,
    typography_theme: &theme::XyChartTypographyThemePlan,
    text_measurer: &dyn TextMeasurer,
) -> Result<XyChartDiagramLayout> {
    if series_paint.plot_count() != model.plots.len() {
        return Err(Error::InvalidModel {
            message: "XY Chart series paint plan does not match the semantic model".to_string(),
        });
    }

    if model
        .orientation
        .as_str()
        .split_whitespace()
        .next()
        .is_some_and(|t| t != "vertical" && t != "horizontal" && !t.is_empty())
    {
        return Err(Error::InvalidModel {
            message: format!("unexpected xychart orientation: {}", model.orientation),
        });
    }

    let mut chart_cfg = series_paint.chart_config().clone();
    typography_theme.apply_font(
        XyChartTextRole::Title,
        &mut chart_cfg.title_font_size,
        &mut chart_cfg.title_font_weight,
    );
    typography_theme.apply_font(
        XyChartTextRole::Legend,
        &mut chart_cfg.legend_font_size,
        &mut chart_cfg.legend_font_weight,
    );
    typography_theme.apply_font(
        XyChartTextRole::XAxisTitle,
        &mut chart_cfg.x_axis.title_font_size,
        &mut chart_cfg.x_axis.title_font_weight,
    );
    typography_theme.apply_font(
        XyChartTextRole::YAxisTitle,
        &mut chart_cfg.y_axis.title_font_size,
        &mut chart_cfg.y_axis.title_font_weight,
    );
    typography_theme.apply_font(
        XyChartTextRole::XAxisLabel,
        &mut chart_cfg.x_axis.label_font_size,
        &mut chart_cfg.x_axis.label_font_weight,
    );
    typography_theme.apply_font(
        XyChartTextRole::YAxisLabel,
        &mut chart_cfg.y_axis.label_font_size,
        &mut chart_cfg.y_axis.label_font_weight,
    );
    let theme_cfg = series_paint.css_binding();

    let title = match model
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .or_else(|| {
            diagram_title
                .map(str::trim)
                .filter(|title| !title.is_empty())
        }) {
        ::core::option::Option::Some(option_value) => option_value,
        ::core::option::Option::None => "",
    }
        .to_owned();
    let mut chart_width =
        reserved_plot_span(chart_cfg.width, chart_cfg.plot_reserved_space_percent)?;
    let mut chart_height =
        reserved_plot_span(chart_cfg.height, chart_cfg.plot_reserved_space_percent)?;
    let mut available_width = chart_cfg.width - chart_width;
    let mut available_height = chart_cfg.height - chart_height;
    let title_dimension = max_text_dimension(
        std::slice::from_ref(&title),
        chart_cfg.title_font_size,
        typography_theme.font_family_css(),
        chart_cfg.title_font_weight.as_deref(),
        text_measurer,
    );
    let title_height = title_dimension.height + 2.0 * chart_cfg.title_padding;
    let show_chart_title =
        chart_cfg.show_title && !title.is_empty() && title_height <= available_height;

    let mut drawables: Vec<XyChartDrawableElem> =
        Vec::with_capacity(model.plots.len().saturating_mul(2).saturating_add(4));

    let (x_axis_kind, x_axis_title) = match &model.x_axis {
        XyChartAxisModel::Band { title: title_2, categories } => (
            AxisKind::Band {
                categories: categories.clone(),
            },
            title_2.clone(),
        ),
        XyChartAxisModel::Linear { title: title_3, min, max } => (
            AxisKind::Linear {
                domain: (match *(min) {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => 0.0,
                }, match *(max) {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => 1.0,
                }),
            },
            title_3.clone(),
        ),
    };
    let (y_axis_kind, y_axis_title) = match &model.y_axis {
        XyChartAxisModel::Band { title: title_4, categories } => (
            AxisKind::Band {
                categories: categories.clone(),
            },
            title_4.clone(),
        ),
        XyChartAxisModel::Linear { title: title_5, min, max } => (
            AxisKind::Linear {
                domain: (match *(min) {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => 0.0,
                }, match *(max) {
                    ::core::option::Option::Some(option_value) => option_value,
                    ::core::option::Option::None => 1.0,
                }),
            },
            title_5.clone(),
        ),
    };

    let x_axis_theme = AxisThemeConfig {
        title_color: theme_cfg.x_axis_title_color.clone(),
        label_color: theme_cfg.x_axis_label_color.clone(),
        tick_color: theme_cfg.x_axis_tick_color.clone(),
        axis_line_color: theme_cfg.x_axis_line_color.clone(),
    };
    let y_axis_theme = AxisThemeConfig {
        title_color: theme_cfg.y_axis_title_color.clone(),
        label_color: theme_cfg.y_axis_label_color.clone(),
        tick_color: theme_cfg.y_axis_tick_color.clone(),
        axis_line_color: theme_cfg.y_axis_line_color.clone(),
    };

    let mut x_axis = Axis::new(
        x_axis_kind,
        chart_cfg.x_axis.clone(),
        x_axis_theme,
        x_axis_title,
    );
    let mut y_axis = Axis::new(
        y_axis_kind,
        chart_cfg.y_axis.clone(),
        y_axis_theme,
        y_axis_title,
    );

    let mut legend_plots = if chart_cfg.show_legend {
        model
            .plots
            .iter()
            .enumerate()
            .filter(|(_, plot)| plot.title.as_deref().is_some_and(|title_6| !title_6.is_empty()))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let legend_origin;

    let plot_rect = if chart_cfg.chart_orientation == "horizontal" {
        let title_y_end = if show_chart_title { title_height } else { 0.0 };
        available_height = (available_height - title_y_end).max(0.0);

        x_axis.set_axis_position(AxisPosition::Left);
        let space_used_x = x_axis.calculate_space(
            Dimension {
                width: available_width,
                height: available_height,
            },
            typography_theme.font_family_css(),
            text_measurer,
        );
        available_width = (available_width - space_used_x.width).max(0.0);
        let plot_x = space_used_x.width;

        y_axis.set_axis_position(AxisPosition::Top);
        let space_used_y = y_axis.calculate_space(
            Dimension {
                width: available_width,
                height: available_height,
            },
            typography_theme.font_family_css(),
            text_measurer,
        );
        available_height = (available_height - space_used_y.height).max(0.0);
        let plot_y = title_y_end + space_used_y.height;

        let legend_space = calculate_legend_space(
            &legend_plots,
            &chart_cfg,
            Dimension {
                width: available_width,
                height: chart_height,
            },
            typography_theme.font_family_css(),
            text_measurer,
        );

        if ::core::matches!(::core::primitive::f64::classify(legend_space.width), ::core::num::FpCategory::Zero) && ::core::matches!(::core::primitive::f64::classify(legend_space.height), ::core::num::FpCategory::Zero) {
            legend_plots.clear();
        }

        available_width = (available_width - legend_space.width).max(0.0);

        if available_width > 0.0 {
            chart_width += available_width;
        }

        if available_height > 0.0 {
            chart_height += available_height;
        }

        let plot_rect = BoundingRect {
            x: plot_x,
            y: plot_y,
            width: chart_width,
            height: chart_height,
        };
        legend_origin = pt(
            plot_x + chart_width,
            plot_y + ((chart_height - legend_space.height) / 2.0).max(0.0),
        );

        y_axis.set_range((plot_x, plot_x + chart_width));
        y_axis.set_bounding_box_xy(pt(plot_x, title_y_end));
        x_axis.set_range((plot_y, plot_y + chart_height));
        x_axis.set_bounding_box_xy(pt(0.0, plot_y));
        plot_rect
    } else {
        let plot_y = if show_chart_title { title_height } else { 0.0 };
        available_height = (available_height - plot_y).max(0.0);

        x_axis.set_axis_position(AxisPosition::Bottom);
        let space_used_x = x_axis.calculate_space(
            Dimension {
                width: available_width,
                height: available_height,
            },
            typography_theme.font_family_css(),
            text_measurer,
        );
        available_height = (available_height - space_used_x.height).max(0.0);

        y_axis.set_axis_position(AxisPosition::Left);
        let space_used_y = y_axis.calculate_space(
            Dimension {
                width: available_width,
                height: available_height,
            },
            typography_theme.font_family_css(),
            text_measurer,
        );
        let plot_x = space_used_y.width;
        available_width = (available_width - space_used_y.width).max(0.0);

        let legend_space = calculate_legend_space(
            &legend_plots,
            &chart_cfg,
            Dimension {
                width: available_width,
                height: chart_height,
            },
            typography_theme.font_family_css(),
            text_measurer,
        );

        if ::core::matches!(::core::primitive::f64::classify(legend_space.width), ::core::num::FpCategory::Zero) && ::core::matches!(::core::primitive::f64::classify(legend_space.height), ::core::num::FpCategory::Zero) {
            legend_plots.clear();
        }

        available_width = (available_width - legend_space.width).max(0.0);

        if available_width > 0.0 {
            chart_width += available_width;
        }

        if available_height > 0.0 {
            chart_height += available_height;
        }

        let plot_rect = BoundingRect {
            x: plot_x,
            y: plot_y,
            width: chart_width,
            height: chart_height,
        };
        legend_origin = pt(
            plot_x + chart_width,
            plot_y + ((chart_height - legend_space.height) / 2.0).max(0.0),
        );

        x_axis.set_range((plot_x, plot_x + chart_width));
        x_axis.set_bounding_box_xy(pt(plot_x, plot_y + chart_height));
        y_axis.set_range((plot_y, plot_y + chart_height));
        y_axis.set_bounding_box_xy(pt(0.0, plot_y));
        plot_rect
    };

    finite_geometry([
        plot_rect.x,
        plot_rect.y,
        plot_rect.width,
        plot_rect.height,
        plot_rect.x + plot_rect.width,
        plot_rect.y + plot_rect.height,
        legend_origin.x,
        legend_origin.y,
    ])?;

    if show_chart_title {


        let half_width = title_dimension.width.min(chart_cfg.width) / 2.0;
        let title_x = (plot_rect.x + plot_rect.width / 2.0)
            .max(half_width)
            .min(chart_cfg.width - half_width);
        drawables.push(XyChartDrawableElem::Text {
            group_texts: vec!["chart-title".to_string()],
            data: vec![XyChartTextData {
                text: title,
                x: title_x,
                y: title_height / 2.0,
                fill: theme_cfg.title_color.clone(),
                font_size: chart_cfg.title_font_size,
                rotation: 0.0,
                vertical_pos: "middle".to_string(),
                horizontal_pos: "center".to_string(),
            }],
        });
    }

    if model
        .plots
        .iter()
        .any(|p| matches!(p.plot_type, XyChartPlotType::Bar))
    {
        x_axis.recalculate_outer_padding_to_draw_bar();
    }

    for (plot_index, plot) in model.plots.iter().enumerate() {
        let fill_color = series_paint
            .fill_css(plot_index)
            .ok_or_else(|| Error::InvalidModel {
                message: "XY Chart series fill plan is incomplete".to_string(),
            })?;
        let stroke_color =
            series_paint
                .stroke_css(plot_index)
                .ok_or_else(|| Error::InvalidModel {
                    message: "XY Chart series stroke plan is incomplete".to_string(),
                })?;

        match plot.plot_type {
            XyChartPlotType::Bar => {
                let bar_padding_percent = 0.05;
                let bar_width = (x_axis.outer_padding * 2.0).min(x_axis.tick_distance())
                    * (1.0 - bar_padding_percent);
                let bar_width_half = bar_width / 2.0;
                finite_geometry([bar_width])?;

                let mut rects: Vec<XyChartRectData> = Vec::with_capacity(plot.data.len());

                for (cat, value) in &plot.data {
                    let x = x_axis.get_scale_value(cat);
                    let y = y_axis.get_scale_number(*value);

                    if value.is_some() {
                        finite_geometry([x, y])?;
                    }

                    let rect = if chart_cfg.chart_orientation == "horizontal" {
                        XyChartRectData {
                            x: plot_rect.x,
                            y: x - bar_width_half,
                            width: y - plot_rect.x,
                            height: bar_width,
                            fill: fill_color.to_string(),
                            stroke_fill: stroke_color.to_string(),
                            stroke_width: series_paint
                                .stroke_width(plot_index)
                                .expect("resolved series width"),
                        }
                    } else {
                        XyChartRectData {
                            x: x - bar_width_half,
                            y,
                            width: bar_width,
                            height: plot_rect.y + plot_rect.height - y,
                            fill: fill_color.to_string(),
                            stroke_fill: stroke_color.to_string(),
                            stroke_width: series_paint
                                .stroke_width(plot_index)
                                .expect("resolved series width"),
                        }
                    };

                    if value.is_some() {
                        finite_geometry([rect.x, rect.y, rect.width, rect.height])?;
                    }

                    rects.push(rect);
                }

                drawables.push(XyChartDrawableElem::Rect {
                    group_texts: vec!["plot".to_string(), format!("bar-plot-{plot_index}")],
                    data: rects,
                });
            }
            XyChartPlotType::Line => {
                let mut points: Vec<(f64, f64)> = Vec::with_capacity(plot.data.len());

                for (cat, value) in &plot.data {
                    let x = x_axis.get_scale_value(cat);
                    let y = y_axis.get_scale_number(*value);

                    if value.is_some() {
                        finite_geometry([x, y])?;
                    }

                    points.push(if chart_cfg.chart_orientation == "horizontal" {
                        (y, x)
                    } else {
                        (x, y)
                    });
                }

                {
                    match line_path(&points) {
                        Some(path) => {
                            drawables.push(XyChartDrawableElem::Path {
                                group_texts: vec!["plot".to_string(), format!("line-plot-{plot_index}")],
                                data: vec![XyChartPathData {
                                    path,
                                    fill: (fill_color != "none").then(|| fill_color.to_string()),
                                    stroke_fill: stroke_color.to_string(),
                                    stroke_width: series_paint
                                        .stroke_width(plot_index)
                                        .expect("resolved series width"),
                                }],
                            });

                            if !plot.point_labels.is_empty() {
                                let label_offset = 10.0;
                                let font_size = 12.0;
                                let labels = points
                                    .iter()
                                    .enumerate()
                                    .filter_map(|(idx, (px, py))| {
                                        let label = plot.point_labels.get(idx)?;

                                        if label.is_empty() {
                                            return None;
                                        }

                                        let (x, y, vertical_pos, horizontal_pos) =
                                            if chart_cfg.chart_orientation == "horizontal" {
                                                (
                                                    px + label_offset,
                                                    *py,
                                                    "middle".to_string(),
                                                    "left".to_string(),
                                                )
                                            } else {
                                                (
                                                    *px,
                                                    py - label_offset,
                                                    "middle".to_string(),
                                                    "center".to_string(),
                                                )
                                            };
                                        Some(XyChartTextData {
                                            text: label.clone(),
                                            x,
                                            y,
                                            fill: series_paint
                                                .point_label_fill_css(plot_index)
                                                .expect("resolved point-label paint")
                                                .to_string(),
                                            font_size,
                                            rotation: 0.0,
                                            vertical_pos,
                                            horizontal_pos,
                                        })
                                    })
                                    .collect::<Vec<_>>();

                                if !labels.is_empty() {
                                    drawables.push(XyChartDrawableElem::Text {
                                        group_texts: vec![
                                            "plot".to_string(),
                                            format!("line-plot-{plot_index}"),
                                            "labels".to_string(),
                                        ],
                                        data: labels,
                                    });
                                }
                            }
                        },
                        ::core::option::Option::None => {},
                    };
                }
            }
        }
    }

    if !series_paint.record_legend_layout(legend_plots.iter().map(|(index, _)| *index)) {
        return Err(Error::InvalidModel {
            message: "XY Chart legend plan was already finalized".to_owned(),
        });
    }

    drawables.extend(legend_drawable_elements(
        &legend_plots,
        legend_origin,
        &chart_cfg,
        &theme_cfg.legend_text_color,
        series_paint,
    )?);

    drawables.extend(x_axis.drawable_elements());
    drawables.extend(y_axis.drawable_elements());

    let label_data = if chart_cfg.show_data_label {
        match model
            .plots
            .first()
            .map(|p| {
                p.data
                    .iter()
                    .map(|(_, y)| {
                        match y.map(|v| format!("{v}")) {
                            ::core::option::Option::Some(explicit_present) => explicit_present,
                            ::core::option::Option::None => "null".to_string(),
                        }
                    })
                    .collect()
            }) {
            ::core::option::Option::Some(option_value) => option_value,
            ::core::option::Option::None => ::std::vec::Vec::new(),
        }
    } else {
        Vec::new()
    };

    Ok(XyChartDiagramLayout {
        width: chart_cfg.width,
        height: chart_cfg.height,
        chart_orientation: chart_cfg.chart_orientation,
        show_data_label: chart_cfg.show_data_label,
        show_data_label_outside_bar: chart_cfg.show_data_label_outside_bar,
        background_color: theme_cfg.background_color.clone(),
        label_data,
        drawables,
    })
}

/// Shared rectangle-label selection and sizing, including Mermaid's legend-marker labels.
/// Labels with a zero font size remain serialized but are not visible theme occurrences.
pub(crate) struct XyChartRectDataLabels<'a> {
    pub(crate) items: Vec<XyChartRectDataLabel<'a>>,
    pub(crate) font_size: f64,
}

pub(crate) struct XyChartRectDataLabel<'a> {
    pub(crate) rect: &'a XyChartRectData,
    pub(crate) item: usize,
    pub(crate) label: &'a str,
}

pub(crate) const XY_CHART_DATA_LABEL_INSET_PX: f64 = 10.0;

#[must_use]
pub(crate) fn rect_data_labels<'a>(
    data: &'a [XyChartRectData],
    labels: &'a [String],
    orientation: &str,
) -> XyChartRectDataLabels<'a> {
    let items = data
        .iter()
        .enumerate()
        .filter_map(|(item, rect)| {
            let label = labels.get(item)?;
            (rect.width > 0.0 && rect.height > 0.0).then_some(XyChartRectDataLabel {
                rect,
                item,
                label,
            })
        })
        .collect::<Vec<_>>();
    let mut min_font = f64::INFINITY;

    for item in &items {
        let fs = if orientation == "horizontal" {
            horizontal_label_font_size(
                item.rect.width,
                item.label,
                item.rect.height * 0.7,
                XY_CHART_DATA_LABEL_INSET_PX,
            )
        } else {
            let denominator = javascript_string_length(item.label) * 0.7;
            let initial = if denominator <= 0.0 {
                0.0
            } else {
                item.rect.width / denominator
            };
            vertical_label_font_size(
                item.rect.width,
                item.rect.height,
                item.label,
                initial,
                XY_CHART_DATA_LABEL_INSET_PX,
            )
        };
        min_font = min_font.min(fs);
    }

    let font_size = if min_font.is_finite() { min_font } else { 0.0 }
        .floor()
        .max(0.0);
    XyChartRectDataLabels { items, font_size }
}

/// Mermaid uses JavaScript's `String#length` for XYChart bar labels, which counts UTF-16 code
/// units rather than Unicode scalar values.
#[must_use]
fn javascript_string_length(text: &str) -> f64 {
    text.encode_utf16().count() as f64
}

/// Computes the result of Mermaid's one-pixel decrement loop without iterating once per pixel.
///
/// The upstream renderer decrements a candidate font size until it fits. That is observable for
/// ordinary dimensions, but a finite value such as `1e308` cannot make numerical progress when
/// subtracting one, so the browser algorithm never terminates. The fit predicates are monotonic;
/// solving their upper bound directly preserves the same discrete candidate for normal values and
/// makes extreme finite dimensions terminate in constant time.
#[must_use]
fn font_size_after_unit_decrements(initial: f64, maximum_that_fits: f64) -> f64 {
    if !(initial.is_finite() && initial > 0.0) || maximum_that_fits.is_nan() {
        return 0.0;
    }

    if maximum_that_fits >= initial {
        return initial;
    }

    if maximum_that_fits <= 0.0 {
        return 0.0;
    }

    let decrements = (initial - maximum_that_fits).ceil();
    let candidate = initial - decrements;

    if candidate.is_finite() && candidate > 0.0 {
        candidate
    } else {
        0.0
    }
}

#[must_use]
fn horizontal_label_font_size(item_width: f64, label: &str, initial: f64, inset_px: f64) -> f64 {
    let denominator = javascript_string_length(label) * 0.7;
    let maximum_that_fits = if denominator > 0.0 {
        (item_width - inset_px) / denominator
    } else {
        f64::INFINITY
    };
    font_size_after_unit_decrements(initial, maximum_that_fits)
}

#[must_use]
fn vertical_label_font_size(
    item_width: f64,
    item_height: f64,
    label: &str,
    initial: f64,
    y_offset: f64,
) -> f64 {
    let denominator = javascript_string_length(label) * 0.7;
    let horizontal_maximum = if denominator > 0.0 {
        item_width / denominator
    } else {
        f64::INFINITY
    };
    let maximum_that_fits = horizontal_maximum.min(item_height - y_offset);
    font_size_after_unit_decrements(initial, maximum_that_fits)
}

#[cfg(test)]
mod data_label_tests {
    use super::*;

    fn upstream_decrement(initial: f64, maximum_that_fits: f64) -> f64 {
        let mut font_size = initial;
        while font_size > maximum_that_fits && font_size > 0.0 {
            font_size -= 1.0;
        }
        font_size
    }

    #[test]
    fn closed_form_font_sizing_matches_mermaid_for_normal_dimensions() {
        for (initial, maximum_that_fits) in [(10.2, 8.0), (10.0, 8.8), (8.0, 8.0), (0.5, 0.0)] {
            assert_eq!(
                font_size_after_unit_decrements(initial, maximum_that_fits)
                    .floor()
                    .max(0.0),
                upstream_decrement(initial, maximum_that_fits)
                    .floor()
                    .max(0.0),
            );
        }
    }

    #[test]
    fn finite_plot_reservation_recovers_only_intermediate_overflow() {
        assert_eq!(
            reserved_plot_span(701.0, 33.3).unwrap(),
            (701.0_f64 * 33.3 / 100.0).floor()
        );
        assert_eq!(reserved_plot_span(1e308, 50.0).unwrap(), 5e307);
        assert!(matches!(
            reserved_plot_span(1.7e308, 200.0),
            Err(Error::InvalidModel { .. })
        ));
    }

    #[test]
    fn extreme_finite_axis_domain_retains_its_linear_mapping_and_bounded_ticks() {
        for (value, expected) in [(-1e308, 0.0), (0.0, 350.0), (1e308, 700.0)] {
            assert_eq!(
                linear_scale_value(value, (-1e308, 1e308), (0.0, 700.0)),
                expected
            );
        }
        let ticks = d3_ticks(-1e308, 1e308, 10);
        assert!(!ticks.is_empty() && ticks.len() <= 20);
        assert!(ticks.iter().all(|value| value.is_finite()));
        assert!(ticks.first().unwrap() < &0.0 && ticks.last().unwrap() > &0.0);
    }

    #[test]
    fn huge_finite_bar_dimensions_complete_without_a_decrement_loop() {
        let font_size = horizontal_label_font_size(1e308, "123", 7e307, 10.0);

        assert!(font_size.is_finite());
        assert!(font_size > 0.0);
    }

    #[test]
    fn bar_label_length_uses_javascript_utf16_code_units() {
        assert_eq!(javascript_string_length("A"), 1.0);
        assert_eq!(javascript_string_length("\u{1F469}\u{200D}\u{1F4BB}"), 5.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::TextMetrics;
    use kurbo::Shape;
    use serde_json::json;

    struct UserUnitMeasurer;

    impl TextMeasurer for UserUnitMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: text.encode_utf16().count() as f64 * style.font_size,
                height: style.font_size,
                line_count: 1,
            }
        }
    }

    fn layout(title: &str, orientation: &str, height: f64, legend: bool) -> XyChartDiagramLayout {
        let model = XyChartDiagramRenderModel {
            semantic: merman_model::families::xy_chart::XyChartDiagramModel {
                orientation: orientation.to_string(),
                title: Some(title.to_string()),
                acc_title: None,
                acc_descr: None,
                x_axis: XyChartAxisModel::Band {
                    title: String::new(),
                    categories: vec!["a".to_string(), "b".to_string(), "c".to_string()],
                },
                y_axis: XyChartAxisModel::Linear {
                    title: "A fairly wide y-axis title".to_string(),
                    min: Some(0.0),
                    max: Some(100000.0),
                },
                plots: vec![XyChartPlotModel {
                    plot_type: XyChartPlotType::Bar,
                    title: Some("series".to_string()),
                    values: vec![10.0, 20.0, 30.0],
                    data: vec![
                        ("a".to_string(), Some(10.0)),
                        ("b".to_string(), Some(20.0)),
                        ("c".to_string(), Some(30.0)),
                    ],
                    point_labels: Vec::new(),
                }],
            },
            display: Default::default(),
        };
        let config = crate::test_support::presentation_config(json!({ "xyChart": {
            "width": 700, "height": height, "showLegend": legend
        }}));
        let work_meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::interactive(),
        );
        let series_paint =
            XyChartSeriesPaintPlan::resolve(None, &config, &model, &work_meter).unwrap();
        let typography = XyChartTypographyThemePlan::resolve(None, &config, &work_meter).unwrap();
        layout_xychart_diagram_typed(&model, None, &series_paint, &typography, &UserUnitMeasurer)
            .unwrap()
    }

    fn title(layout: &XyChartDiagramLayout) -> Option<&XyChartTextData> {
        layout.drawables.iter().find_map(|drawable| match drawable {
            XyChartDrawableElem::Text { group_texts, data } if group_texts == &["chart-title"] => {
                data.first()
            }
            _ => None,
        })
    }

    fn plot_span(layout: &XyChartDiagramLayout) -> (f64, f64) {
        let axis = if layout.chart_orientation == "horizontal" {
            "top-axis"
        } else {
            "bottom-axis"
        };
        let path = layout
            .drawables
            .iter()
            .find_map(|drawable| match drawable {
                XyChartDrawableElem::Path { group_texts, data }
                    if group_texts == &[axis, "axis-line"] =>
                {
                    data.first()
                }
                _ => None,
            })
            .expect("axis spanning the plot");
        let bounds = kurbo::BezPath::from_svg(&path.path).unwrap().bounding_box();
        (bounds.x0, bounds.x1)
    }

    #[test]
    fn xychart_title_centers_on_plot_for_both_orientations_and_legend_modes() {
        for orientation in ["vertical", "horizontal"] {
            for legend in [false, true] {
                let layout = layout("Title", orientation, 500.0, legend);
                let (start, end) = plot_span(&layout);
                assert!(start > 0.0);
                assert!((title(&layout).unwrap().x - (start + end) / 2.0).abs() < 1e-9);
                if legend {
                    assert!(end < layout.width);
                } else {
                    assert_eq!(end, layout.width);
                }
            }
        }
    }

    #[test]
    fn xychart_wide_title_clamps_to_chart_and_can_span_legend_columns() {
        let wide = "x".repeat(32);
        let without_legend = layout(&wide, "vertical", 500.0, false);
        assert_eq!(title(&without_legend).unwrap().x, 380.0);

        let with_legend = layout(&wide, "vertical", 500.0, true);
        let (start, end) = plot_span(&with_legend);
        let label = title(&with_legend).unwrap();
        assert_eq!(label.x, (start + end) / 2.0);
        assert!(label.x + 320.0 > end);
        assert!(label.x + 320.0 <= with_legend.width);
        let legend_marker = with_legend
            .drawables
            .iter()
            .find_map(|drawable| match drawable {
                XyChartDrawableElem::Rect { group_texts, data }
                    if group_texts == &["legend", "markers"] =>
                {
                    data.first()
                }
                _ => None,
            })
            .unwrap();
        assert!(label.y + 10.0 <= legend_marker.y - 10.0);

        let oversized = layout(&"x".repeat(40), "vertical", 500.0, false);
        assert_eq!(title(&oversized).unwrap().x, oversized.width / 2.0);
    }

    #[test]
    fn xychart_title_needs_space_after_the_plot_reservation() {
        for orientation in ["vertical", "horizontal"] {
            let short = layout("Title", orientation, 60.0, false);
            assert!(title(&short).is_none());
            let mut bars = 0;
            for drawable in &short.drawables {
                if let XyChartDrawableElem::Rect { group_texts, data } = drawable
                    && group_texts == &["plot", "bar-plot-0"]
                {
                    bars += data.len();
                    for bar in data {
                        assert!(bar.y + bar.height <= short.height + 1e-9);
                    }
                }
            }
            assert_eq!(bars, 3);
            assert!(title(&layout("Title", orientation, 100.0, false)).is_some());
        }
    }

    #[test]
    fn xychart_measurement_keeps_svg_user_units_without_display_rescaling() {
        let dimensions = max_text_dimension(
            &["CPU".to_string()],
            14.0,
            "sans-serif",
            None,
            &UserUnitMeasurer,
        );
        assert_eq!(dimensions.width, 42.0);
        assert_eq!(dimensions.height, 14.0);
    }

    #[test]
    fn xychart_dimension_probe_keeps_literal_markup_in_one_svg_run() {
        let measurer = crate::text::DeterministicTextMeasurer {
            char_width_factor: 1.0,
            ..Default::default()
        };
        let text = "AAAAAA<br/>BBBBBB";
        let dimensions = max_text_dimension(&[text.to_owned()], 10.0, "serif", None, &measurer);
        assert_eq!(dimensions.width, text.len() as f64 * 10.0);
        assert_eq!(dimensions.height, 11.0);
    }

    #[test]
    fn xychart_measures_title_axes_and_legend_with_the_configured_font() {
        use std::sync::Mutex;
        struct RecordingFont(Mutex<Vec<String>>);
        impl TextMeasurer for RecordingFont {
            fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
                assert_eq!(style.font_family.as_deref(), Some("Inter,sans-serif"));
                self.0.lock().unwrap().push(text.to_owned());
                UserUnitMeasurer.measure(text, style)
            }
        }
        let parsed = merman_core::MermaidEngine::new().parse_diagram_for_render_model_sync(
            "xychart\ntitle Chart\nx-axis Categories [One, Two, Three]\ny-axis Amount 0 --> 40\nline Series [10, 30, 20]",
            merman_core::ParseOptions::strict(),
        ).unwrap().unwrap().into_presentation();
        let merman_model::presentation::RenderSemanticModel::XyChart(model) = parsed.model() else {
            panic!("expected XYChart render model");
        };
        let measurer = RecordingFont(Mutex::new(Vec::new()));
        let config = crate::test_support::presentation_config(json!({
            "fontFamily":"Inter, sans-serif", "xyChart":{"showLegend":true},
        }));
        let work_meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::interactive(),
        );
        let series = XyChartSeriesPaintPlan::resolve(None, &config, model, &work_meter).unwrap();
        let typography = XyChartTypographyThemePlan::resolve(None, &config, &work_meter).unwrap();
        layout_xychart_diagram_typed(model, None, &series, &typography, &measurer).unwrap();
        let text = measurer.0.lock().unwrap();
        for expected in ["Chart", "Categories", "Amount", "One", "Series"] {
            assert!(
                text.iter().any(|text| text == expected),
                "missing measurement: {expected}"
            );
        }
    }
}
