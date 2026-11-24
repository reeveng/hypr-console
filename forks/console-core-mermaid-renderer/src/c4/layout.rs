use super::{
    C4_FRAMED_FRAME_WIDTH, C4_NEO_FRAMED_LABEL_PADDING_X, C4_NEO_FRAMED_LABEL_PADDING_Y, C4Conf,
    C4ConfigView, C4Look, C4Model, C4NodeShape, c4_node_shape, c4_stereotype_text, measure_c4_text,
    measure_c4_unified_text,
};
use crate::model::{
    Bounds, C4BoundaryLayout, C4DiagramLayout, C4ImageLayout, C4RelLayout, C4ShapeLayout,
    C4TextBlockLayout, LayoutPoint,
};
use crate::text::TextMeasurer;
use crate::{Error, Result};
use merman_model::families::c4::{C4Boundary, C4DiagramModel};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
struct BoundsData {
    startx: Option<f64>,
    stopx: Option<f64>,
    starty: Option<f64>,
    stopy: Option<f64>,
    width_limit: f64,
}

#[derive(Debug, Clone, Default)]
struct BoundsNext {
    startx: f64,
    stopx: f64,
    starty: f64,
    stopy: f64,
    cnt: usize,
}

#[derive(Debug, Clone, Default)]
struct BoundsState {
    data: BoundsData,
    next: BoundsNext,
}

impl BoundsState {
    fn set_data(&mut self, startx: f64, stopx: f64, starty: f64, stopy: f64) {
        self.next.startx = startx;
        self.data.startx = Some(startx);
        self.next.stopx = stopx;
        self.data.stopx = Some(stopx);
        self.next.starty = starty;
        self.data.starty = Some(starty);
        self.next.stopy = stopy;
        self.data.stopy = Some(stopy);
    }

    fn bump_last_margin(&mut self, margin: f64) {
        match self.data.stopx.as_mut() {
            Some(v) => {
                *v += margin;
            },
            ::core::option::Option::None => {},
        }

        match self.data.stopy.as_mut() {
            Some(v) => {
                *v += margin;
            },
            ::core::option::Option::None => {},
        }
    }

    fn update_val_opt(target: &mut Option<f64>, val: f64, fun: fn(f64, f64) -> f64) {
        match target {
            None => *target = Some(val),
            Some(existing) => *existing = fun(val, *existing),
        }
    }

    fn update_val(target: &mut f64, val: f64, fun: fn(f64, f64) -> f64) {
        *target = fun(val, *target);
    }

    fn insert_rect(&mut self, rect: &mut Rect, c4_shape_in_row: usize, conf: &C4Conf) {
        self.next.cnt += 1;

        let startx = if <::core::primitive::f64 as ::core::cmp::PartialOrd>::partial_cmp(&self.next.startx, &self.next.stopx) == ::core::option::Option::Some(::core::cmp::Ordering::Equal) {
            self.next.stopx + rect.margin
        } else {
            self.next.stopx + rect.margin * 2.0
        };
        let mut stopx = startx + rect.size.width;
        let starty = self.next.starty + rect.margin * 2.0;
        let mut stopy = starty + rect.size.height;

        if startx >= self.data.width_limit
            || stopx >= self.data.width_limit
            || self.next.cnt > c4_shape_in_row
        {
            let startx2 = self.next.startx + rect.margin + conf.next_line_padding_x;
            let starty2 = self.next.stopy + rect.margin * 2.0;

            stopx = startx2 + rect.size.width;
            stopy = starty2 + rect.size.height;

            self.next.stopx = stopx;
            self.next.starty = self.next.stopy;
            self.next.stopy = stopy;
            self.next.cnt = 1;

            rect.origin.x = startx2;
            rect.origin.y = starty2;
        } else {
            rect.origin.x = startx;
            rect.origin.y = starty;
        }

        Self::update_val_opt(&mut self.data.startx, rect.origin.x, f64::min);
        Self::update_val_opt(&mut self.data.starty, rect.origin.y, f64::min);
        Self::update_val_opt(&mut self.data.stopx, stopx, f64::max);
        Self::update_val_opt(&mut self.data.stopy, stopy, f64::max);

        Self::update_val(&mut self.next.startx, rect.origin.x, f64::min);
        Self::update_val(&mut self.next.starty, rect.origin.y, f64::min);
        Self::update_val(&mut self.next.stopx, stopx, f64::max);
        Self::update_val(&mut self.next.stopy, stopy, f64::max);
    }
}

#[derive(Debug, Clone)]
struct Rect {
    origin: crate::geom::Point,
    size: crate::geom::Size,
    margin: f64,
}

struct C4LayoutContext<'a> {
    model: &'a C4Model,
    typography: &'a super::C4TypographyThemePlan,
    conf: &'a C4Conf,
    c4_shape_in_row: usize,
    c4_boundary_in_row: usize,
    measurer: &'a dyn TextMeasurer,
    boundary_children: &'a HashMap<String, Vec<usize>>,
    shape_children: &'a HashMap<String, Vec<usize>>,
}

struct C4LayoutState {
    boundaries: HashMap<String, C4BoundaryLayout>,
    shapes: HashMap<String, C4ShapeLayout>,
    global_max_x: f64,
    global_max_y: f64,
}

#[must_use]
fn has_sprite(v: &Option<Value>) -> bool {
    v.as_ref().is_some_and(|v_2| match v_2 {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(_) => true,
        Value::String(s) => !s.trim().is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    })
}

#[must_use]
fn intersect_rect_point(rect: &Rect, target: LayoutPoint) -> LayoutPoint {


    let center_x = rect.origin.x + rect.size.width / 2.0;
    let center_y = rect.origin.y + rect.size.height / 2.0;
    let dx = target.x - center_x;
    let dy = target.y - center_y;
    let mut half_width = rect.size.width / 2.0;
    let mut half_height = rect.size.height / 2.0;
    let (x, y) = if dy.abs() * half_width > dx.abs() * half_height {
        if dy < 0.0 {
            half_height = -half_height;
        }

        (
            if ::core::matches!(::core::primitive::f64::classify(dy), ::core::num::FpCategory::Zero) {
                0.0
            } else {
                half_height * dx / dy
            },
            half_height,
        )
    } else {
        if dx < 0.0 {
            half_width = -half_width;
        }

        (
            half_width,
            if ::core::matches!(::core::primitive::f64::classify(dx), ::core::num::FpCategory::Zero) { 0.0 } else { half_width * dy / dx },
        )
    };
    LayoutPoint {
        x: center_x + x,
        y: center_y + y,
    }
}

#[must_use]
fn intersect_cylinder_point(from: &Rect, end_point: LayoutPoint) -> LayoutPoint {
    let mut point = intersect_rect_point(from, end_point.clone());
    let center_x = from.origin.x + from.size.width / 2.0;
    let center_y = from.origin.y + from.size.height / 2.0;
    let x = point.x - center_x;
    let width = from.size.width.max(1.0);
    let rx = width / 2.0;
    let ry = rx / (2.5 + width / 50.0);

    if !::core::matches!(::core::primitive::f64::classify(rx), ::core::num::FpCategory::Zero)
        && (x.abs() < width / 2.0
            || ((x.abs() - width / 2.0).abs() < 1e-12
                && (point.y - center_y).abs() > from.size.height / 2.0 - ry))
    {
        let mut cap = ry * ry * (1.0 - (x * x) / (rx * rx));
        cap = cap.max(0.0).sqrt();
        cap = ry - cap;

        if end_point.y - center_y > 0.0 {
            cap = -cap;
        }

        point.y += cap;
    }

    point
}

#[must_use]
fn intersect_horizontal_cylinder_point(from: &Rect, end_point: LayoutPoint) -> LayoutPoint {
    let mut point = intersect_rect_point(from, end_point.clone());
    let center_x = from.origin.x + from.size.width / 2.0;
    let center_y = from.origin.y + from.size.height / 2.0;
    let y = point.y - center_y;
    let half_height = from.size.height / 2.0;
    let ry = half_height;
    let rx = if ::core::matches!(::core::primitive::f64::classify(ry), ::core::num::FpCategory::Zero) {
        0.0
    } else {
        ry / (2.5 + from.size.height / 50.0)
    };

    if !::core::matches!(::core::primitive::f64::classify(ry), ::core::num::FpCategory::Zero)
        && (y.abs() < half_height
            || (y.abs() - half_height).abs() < 1e-12
                && (point.x - center_x).abs() > from.size.width / 2.0 - rx)
    {
        let mut cap = rx * rx * (1.0 - (y * y) / (ry * ry));
        cap = cap.abs().sqrt();
        cap = rx - cap;

        if end_point.x - center_x > 0.0 {
            cap = -cap;
        }

        point.x += cap;
    }

    point
}

#[must_use]
fn cross(a: LayoutPoint, b: LayoutPoint) -> f64 {
    a.x * b.y - a.y * b.x
}

#[must_use]
fn line_polygon_intersection(
    origin: LayoutPoint,
    target: LayoutPoint,
    polygon: &[LayoutPoint],
) -> Option<LayoutPoint> {
    let direction = LayoutPoint {
        x: target.x - origin.x,
        y: target.y - origin.y,
    };

    if direction.x.abs() < 1e-12 && direction.y.abs() < 1e-12 {
        return None;
    }

    let mut nearest: Option<(f64, LayoutPoint)> = None;

    for (a, b) in polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .take(polygon.len())
    {
        let edge = LayoutPoint {
            x: b.x - a.x,
            y: b.y - a.y,
        };
        let origin_to_a = LayoutPoint {
            x: a.x - origin.x,
            y: a.y - origin.y,
        };
        let denominator = cross(direction.clone(), edge.clone());

        if denominator.abs() < 1e-12 {
            continue;
        }

        let t = cross(origin_to_a.clone(), edge) / denominator;
        let u = cross(origin_to_a, direction.clone()) / denominator;

        if !(0.0..=1.0).contains(&u) {
            continue;
        }

        let point = LayoutPoint {
            x: origin.x + direction.x * t,
            y: origin.y + direction.y * t,
        };



        let distance = (point.x - target.x).hypot(point.y - target.y);

        if nearest.as_ref().is_none_or(|(best, _)| distance < *best) {
            nearest = Some((distance, point));
        }
    }

    nearest.map(|(_, point)| point)
}

#[must_use]
fn person_polygon(PersonPolygonArguments { width, height }: PersonPolygonArguments) -> Vec<LayoutPoint> {
    fn append_arc(
        points: &mut Vec<LayoutPoint>,
        center_x: f64,
        center_y: f64,
        radius: f64,
        count: usize,
        start_deg: f64,
        end_deg: f64,
    ) {

        let start = start_deg.to_radians();
        let step = (end_deg.to_radians() - start) / (count - 1) as f64;

        for index in 0..count {
            let angle = start + index as f64 * step;
            points.push(LayoutPoint {
                x: -(center_x + radius * angle.cos()),
                y: -(center_y + radius * angle.sin()),
            });
        }
    }

    let head_radius = (width * 0.23).clamp(16.0, 56.0);
    let overlap = head_radius * 0.27;
    let body_height = (height - 2.0 * head_radius + overlap).max(0.0);
    let body_radius = (width * 0.177).min(body_height * 0.45);
    let top = -height / 2.0;
    let body_top = top + 2.0 * head_radius - overlap;
    let head_center_y = top + head_radius;
    let phi_right_deg = ((body_top - head_center_y) / head_radius)
        .min(1.0)
        .asin()
        .to_degrees();


    let mut points = Vec::with_capacity(72);
    append_arc(
        &mut points,
        0.0,
        -head_center_y,
        head_radius,
        24,
        180.0 + phi_right_deg,
        -phi_right_deg,
    );

    for (center_x, center_y, start, end) in [
        (
            width / 2.0 - body_radius,
            -(body_top + body_radius),
            90.0,
            0.0,
        ),
        (
            width / 2.0 - body_radius,
            -(height / 2.0 - body_radius),
            360.0,
            270.0,
        ),
        (
            -(width / 2.0 - body_radius),
            -(height / 2.0 - body_radius),
            270.0,
            180.0,
        ),
        (
            -(width / 2.0 - body_radius),
            -(body_top + body_radius),
            180.0,
            90.0,
        ),
    ] {
        append_arc(&mut points, center_x, center_y, body_radius, 12, start, end);
    }

    points
}

struct PersonPolygonArguments {
    width: f64,
    height: f64,
}

#[must_use]
fn intersect_polygon_point(
    from: &Rect,
    end_point: LayoutPoint,
    polygon: &[LayoutPoint],
) -> LayoutPoint {
    let center = LayoutPoint {
        x: from.origin.x + from.size.width / 2.0,
        y: from.origin.y + from.size.height / 2.0,
    };


    let origin = LayoutPoint {
        x: from.size.width / 2.0
            + polygon
                .iter()
                .map(|point| point.x)
                .fold(f64::INFINITY, f64::min),
        y: from.size.height / 2.0
            + polygon
                .iter()
                .map(|point| point.y)
                .fold(f64::INFINITY, f64::min),
    };
    let target = LayoutPoint {
        x: origin.x + (end_point.x - center.x),
        y: origin.y + (end_point.y - center.y),
    };

    match line_polygon_intersection(origin.clone(), target, polygon) {
        Some(point) => {
            return LayoutPoint {
                x: center.x + point.x - origin.x,
                y: center.y + point.y - origin.y,
            };
        },
        ::core::option::Option::None => {},
    }

    center
}

#[must_use]
fn intersect_framed_point(from: &Rect, end_point: LayoutPoint) -> LayoutPoint {

    let inner_width = from.size.width - 16.0;
    let height = from.size.height;
    let polygon = [
        (0.0, 0.0),
        (inner_width, 0.0),
        (inner_width, -height),
        (0.0, -height),
        (0.0, 0.0),
        (-8.0, 0.0),
        (inner_width + 8.0, 0.0),
        (inner_width + 8.0, -height),
        (-8.0, -height),
        (-8.0, 0.0),
    ]
    .map(|(x, y)| LayoutPoint { x, y });
    intersect_polygon_point(from, end_point, &polygon)
}

#[must_use]
fn intersect_shape_point(shape: C4NodeShape, from: &Rect, end_point: LayoutPoint) -> LayoutPoint {
    match shape {
        C4NodeShape::Rounded => intersect_rect_point(from, end_point),
        C4NodeShape::Framed => intersect_framed_point(from, end_point),
        C4NodeShape::Person => {
            let polygon = person_polygon(self::PersonPolygonArguments { width: from.size.width, height: from.size.height });
            intersect_polygon_point(from, end_point, &polygon)
        }
        C4NodeShape::Cylinder => intersect_cylinder_point(from, end_point),
        C4NodeShape::HorizontalCylinder => intersect_horizontal_cylinder_point(from, end_point),
    }
}

fn layout_c4_shape_array(
    current_bounds: &mut BoundsState,
    shape_indices: &[usize],
    ctx: &C4LayoutContext<'_>,
    state: &mut C4LayoutState,
) {
    for idx in shape_indices {
        let shape = &ctx.model.shapes[*idx];
        let type_c4_shape = shape.type_c4_shape.as_str().to_string();
        let shape_kind = c4_node_shape(shape);
        let look = ctx.conf.look;



        let text_wrap = ctx.conf.wrap;
        let text_limit_width = match shape_kind {


            C4NodeShape::Cylinder => {
                let label_padding = if look.is_neo() {
                    24.0
                } else {
                    ctx.conf.c4_shape_padding
                };
                (ctx.conf.width - label_padding - ctx.conf.c4_shape_padding * 2.0).max(32.0)
            }
            C4NodeShape::HorizontalCylinder => {


                let label_padding = if look.is_neo() {
                    12.0
                } else {
                    ctx.conf.c4_shape_padding / 2.0
                };
                (ctx.conf.width - label_padding - ctx.conf.c4_shape_padding * 2.0).max(32.0)
            }
            C4NodeShape::Rounded | C4NodeShape::Framed | C4NodeShape::Person => (ctx.conf.width - ctx.conf.c4_shape_padding * 2.0).max(32.0),
        };
        let text_conf = ctx
            .typography
            .element_style(&type_c4_shape)
            .expect("C4 preparation binds every model shape type before layout")
            .font
            .clone();




        let label_text = shape.label.as_str().to_string();
        let has_label = !label_text.trim().is_empty();
        let label_m = if !has_label {
            None
        } else {
            let mut name_conf = text_conf.clone();
            name_conf.font_weight = Some("bold".to_string());
            Some(measure_c4_unified_text(
                ctx.measurer,
                &label_text,
                &name_conf,
                text_wrap,
                text_limit_width,
            ))
        };
        let label = C4TextBlockLayout {
            text: label_text,
            y: 0.0,
            width: label_m.as_ref().map_or(0.0, |m| m.metrics.width),
            height: label_m.as_ref().map_or(0.0, |m| m.metrics.height),
            line_count: label_m.as_ref().map_or(0, |m| m.metrics.line_count),
            render_plan: label_m.map(|m| m.render_plan),
        };




        let stereotype_text = c4_stereotype_text(shape);
        let mut stereotype_conf = text_conf.clone();
        stereotype_conf.font_size *= 0.75;
        let stereotype_m = measure_c4_unified_text(
            ctx.measurer,
            &stereotype_text,
            &stereotype_conf,
            text_wrap,
            text_limit_width,
        );
        let type_block = C4TextBlockLayout {
            text: stereotype_text,
            y: 0.0,
            width: stereotype_m.metrics.width,
            height: stereotype_m.metrics.height,
            line_count: stereotype_m.metrics.line_count,
            render_plan: Some(stereotype_m.render_plan),
        };

        let descr_block = shape
            .descr
            .as_ref()
            .filter(|text| !text.as_str().is_empty())
            .map(|descr| {
                let text = descr.as_str().to_string();
                let mut descr_conf = text_conf.clone();
                descr_conf.font_size *= 0.82;
                let measured = measure_c4_unified_text(
                    ctx.measurer,
                    &text,
                    &descr_conf,
                    text_wrap,
                    text_limit_width,
                );
                C4TextBlockLayout {
                    text,
                    y: 0.0,
                    width: measured.metrics.width,
                    height: measured.metrics.height,
                    line_count: measured.metrics.line_count,
                    render_plan: Some(measured.render_plan),
                }
            });

        let section_gap = 3.0;
        let has_descr = descr_block.is_some();
        let section_count = ::core::primitive::usize::wrapping_add(usize::from(has_label), 1).wrapping_add(usize::from(has_descr));
        let content_width = label
            .width
            .max(type_block.width)
            .max(match descr_block.as_ref().map(|block| block.width) {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => 0.0,
            });
        let content_height = label.height
            + type_block.height
            + match descr_block
                .as_ref()
                .map(|block| block.height) {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => 0.0,
            }
            + section_gap * section_count.saturating_sub(1) as f64;



        let label_padding_y = match (shape_kind, look) {


            (C4NodeShape::Framed, C4Look::Neo) => C4_NEO_FRAMED_LABEL_PADDING_Y / 2.0,
            (C4NodeShape::HorizontalCylinder, C4Look::Neo) => 6.0,
            (C4NodeShape::Cylinder, C4Look::Neo) => 24.0,
            _ => ctx.conf.c4_shape_padding,
        };
        let mut section_y =
            (ctx.conf.height.max(content_height + 2.0 * label_padding_y) - content_height) / 2.0;
        let mut label_2 = label;

        if has_label {
            label_2.y = section_y + label_2.height / 2.0;
            section_y += label_2.height + section_gap;
        }

        let mut type_block_2 = type_block;
        type_block_2.y = section_y + type_block_2.height / 2.0;
        section_y += type_block_2.height + section_gap;
        let mut descr_block_2 = descr_block;

        match descr_block_2.as_mut() {
            Some(descr) => {
                descr.y = section_y + descr.height / 2.0;
            },
            ::core::option::Option::None => {},
        }

        let image = C4ImageLayout {
            width: 0.0,
            height: 0.0,
            y: 0.0,
        };
        let padding = ctx.conf.c4_shape_padding;
        let base_width = (content_width + 2.0 * padding).max(ctx.conf.width);


        let base_height = content_height + 2.0 * padding;
        let (width, height) = match shape_kind {
            C4NodeShape::Rounded => (base_width, base_height),
            C4NodeShape::Framed => {
                let label_padding_x = if look.is_neo() {
                    C4_NEO_FRAMED_LABEL_PADDING_X
                } else {
                    padding
                };
                let label_padding_y_2 = if look.is_neo() {
                    C4_NEO_FRAMED_LABEL_PADDING_Y
                } else {
                    padding
                };
                (
                    (content_width + 2.0 * C4_FRAMED_FRAME_WIDTH + label_padding_x)
                        .max(ctx.conf.width),
                    content_height + label_padding_y_2,
                )
            }
            C4NodeShape::Person => {
                let width = base_width.max(100.0);
                let head_radius = (width * 0.23).clamp(16.0, 56.0);
                let overlap = head_radius * 0.27;
                let body_height = base_height;
                (width, body_height + 2.0 * head_radius - overlap)
            }
            C4NodeShape::Cylinder => {


                let label_padding = if look.is_neo() { 24.0 } else { padding };
                let width = (content_width + label_padding).max(ctx.conf.width).max(1.0);
                let rx = width / 2.0;
                let ry = rx / (2.5 + width / 50.0);


                (width, content_height + label_padding + 3.0 * ry)
            }
            C4NodeShape::HorizontalCylinder => {




                let label_padding = if look.is_neo() { 12.0 } else { padding / 2.0 };
                let inner_width = (ctx.conf.width - label_padding).max(10.0);
                let body_height = content_height + label_padding;
                let ry = body_height / 2.0;
                let rx = ry / (2.5 + body_height / 50.0);
                let width = inner_width.max(content_width) + rx + label_padding;
                (width, body_height)
            }
        };
        let margin = ctx.conf.c4_shape_margin;

        let mut rect = Rect {
            origin: crate::geom::point(0.0, 0.0),
            size: crate::geom::Size::new(width, height),
            margin,
        };
        current_bounds.insert_rect(&mut rect, ctx.c4_shape_in_row, ctx.conf);

        state.shapes.insert(
            shape.alias.clone(),
            C4ShapeLayout {
                alias: shape.alias.clone(),
                parent_boundary: shape.parent_boundary.clone(),
                type_c4_shape: type_c4_shape.clone(),
                x: rect.origin.x,
                y: rect.origin.y,
                width: rect.size.width,
                height: rect.size.height,
                margin: rect.margin,
                image,
                type_block: type_block_2,
                label: label_2,
                ty: None,
                techn: None,
                descr: descr_block_2,
            },
        );
    }

    current_bounds.bump_last_margin(ctx.conf.c4_shape_margin);
}

struct PendingC4BoundaryLayout {
    alias: String,
    parent_boundary: String,
    image: C4ImageLayout,
    label: C4TextBlockLayout,
    ty: Option<C4TextBlockLayout>,
    descr: Option<C4TextBlockLayout>,
}

struct C4BoundaryFrame {
    boundary_indices: Vec<usize>,
    next_index: usize,
    parent_bounds: BoundsState,
    current_bounds: BoundsState,
    pending: Option<PendingC4BoundaryLayout>,
}

impl C4BoundaryFrame {
    #[must_use]
    fn new(
        boundary_indices: Vec<usize>,
        parent_bounds: BoundsState,
        ctx: &C4LayoutContext<'_>,
    ) -> Self {
        let denom = ctx.c4_boundary_in_row.min(boundary_indices.len().max(1));
        let width_limit = parent_bounds.data.width_limit / denom as f64;
        let mut current_bounds = BoundsState::default();
        current_bounds.data.width_limit = width_limit;

        Self {
            boundary_indices,
            next_index: 0,
            parent_bounds,
            current_bounds,
            pending: None,
        }
    }
}

#[must_use]
fn prepare_c4_boundary_layout(
    boundary: &C4Boundary,
    width_limit: f64,
    ctx: &C4LayoutContext<'_>,
) -> (PendingC4BoundaryLayout, f64) {
    let mut y = 0.0;

    let mut image = C4ImageLayout {
        width: 0.0,
        height: 0.0,
        y: 0.0,
    };

    if has_sprite(&boundary.sprite) {
        image.width = 48.0;
        image.height = 48.0;
        image.y = y;
        y = image.y + image.height;
    }




    let text_wrap = (match boundary.wrap {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => false,
    }) && ctx.conf.wrap;
    let mut label_conf = ctx.conf.boundary_font();
    label_conf.font_size += 2.0;
    label_conf.font_weight = Some("bold".to_string());

    let label_text = boundary.label.as_str().to_string();
    let label_m = measure_c4_text(
        ctx.measurer,
        &label_text,
        &label_conf,
        text_wrap,
        width_limit,
    );
    let label = C4TextBlockLayout {
        text: label_text,
        y: y + 8.0,
        width: label_m.width,
        height: label_m.height,
        line_count: label_m.line_count,
        render_plan: None,
    };
    y = label.y + label.height;

    let mut ty: Option<C4TextBlockLayout> = None;

    match boundary.ty.as_ref().filter(|t| !t.as_str().is_empty()) {
        Some(boundary_ty) => {
            let ty_text = format!("[{}]", boundary_ty.as_str());
            let ty_conf = ctx.conf.boundary_font();
            let m = measure_c4_text(ctx.measurer, &ty_text, &ty_conf, text_wrap, width_limit);
            let block = C4TextBlockLayout {
                text: ty_text,
                y: y + 5.0,
                width: m.width,
                height: m.height,
                line_count: m.line_count,
                render_plan: None,
            };
            y = block.y + block.height;
            ty = Some(block);
        },
        ::core::option::Option::None => {},
    }

    let mut descr: Option<C4TextBlockLayout> = None;

    match boundary.descr.as_ref().filter(|t| !t.as_str().is_empty()) {
        Some(boundary_descr) => {
            let descr_text = boundary_descr.as_str().to_string();
            let mut descr_conf = ctx.conf.boundary_font();
            descr_conf.font_size -= 2.0;
            let m = measure_c4_text(
                ctx.measurer,
                &descr_text,
                &descr_conf,
                text_wrap,
                width_limit,
            );
            let block = C4TextBlockLayout {
                text: descr_text,
                y: y + 20.0,
                width: m.width,
                height: m.height,
                line_count: m.line_count,
                render_plan: None,
            };
            y = block.y + block.height;
            descr = Some(block);
        },
        ::core::option::Option::None => {},
    }

    (
        PendingC4BoundaryLayout {
            alias: boundary.alias.clone(),
            parent_boundary: boundary.parent_boundary.clone(),
            image,
            label,
            ty,
            descr,
        },
        y,
    )
}

fn finish_c4_boundary_layout(
    parent_bounds: &mut BoundsState,
    current_bounds: &BoundsState,
    pending: PendingC4BoundaryLayout,
    ctx: &C4LayoutContext<'_>,
    state: &mut C4LayoutState,
) {
    let startx = match current_bounds.data.startx {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0.0,
    };
    let stopx = match current_bounds.data.stopx {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => startx,
    };
    let starty = match current_bounds.data.starty {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0.0,
    };
    let stopy = match current_bounds.data.stopy {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => starty,
    };

    state.boundaries.insert(
        pending.alias.clone(),
        C4BoundaryLayout {
            alias: pending.alias,
            parent_boundary: pending.parent_boundary,
            x: startx,
            y: starty,
            width: stopx - startx,
            height: stopy - starty,
            image: pending.image,
            label: pending.label,
            ty: pending.ty,
            descr: pending.descr,
        },
    );

    let stopx_with_margin = stopx + ctx.conf.c4_shape_margin;
    let stopy_with_margin = stopy + ctx.conf.c4_shape_margin;
    parent_bounds.data.stopx = Some(
        match parent_bounds
            .data
            .stopx {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => stopx_with_margin,
        }
            .max(stopx_with_margin),
    );
    parent_bounds.data.stopy = Some(
        match parent_bounds
            .data
            .stopy {
            ::core::option::Option::Some(present) => present,
            ::core::option::Option::None => stopy_with_margin,
        }
            .max(stopy_with_margin),
    );

    state.global_max_x = state
        .global_max_x
        .max(parent_bounds.data.stopx.unwrap_or(state.global_max_x));
    state.global_max_y = state
        .global_max_y
        .max(parent_bounds.data.stopy.unwrap_or(state.global_max_y));
}

fn layout_inside_boundary(
    parent_bounds: &mut BoundsState,
    boundary_indices: &[usize],
    ctx: &C4LayoutContext<'_>,
    state: &mut C4LayoutState,
) -> Result<()> {
    let mut stack = vec![C4BoundaryFrame::new(
        boundary_indices.to_vec(),
        parent_bounds.clone(),
        ctx,
    )];

    while let Some(frame) = stack.last_mut() {
        match frame.pending.take() {
            Some(pending) => {
                finish_c4_boundary_layout(
                    &mut frame.parent_bounds,
                    &frame.current_bounds,
                    pending,
                    ctx,
                    state,
                );
                continue;
            },
            ::core::option::Option::None => {},
        }

        if frame.next_index >= frame.boundary_indices.len() {
            let finished = {
                let optional_finished = stack.pop();

                match optional_finished {
                    Some(matched_finished) => matched_finished,
                    ::core::option::Option::None => break,
                }
            };

            match stack.last_mut() {
                Some(parent) => {
                    parent.current_bounds = finished.parent_bounds;
                    continue;
                },
                ::core::option::Option::None => {},
            }

            *parent_bounds = finished.parent_bounds;
            return Ok(());
        }

        let i = frame.next_index;
        let idx = frame.boundary_indices[i];
        frame.next_index += 1;

        let boundary = &ctx.model.boundaries[idx];
        let width_limit = frame.current_bounds.data.width_limit;
        let (pending, y) = prepare_c4_boundary_layout(boundary, width_limit, ctx);

        let parent_startx = frame
            .parent_bounds
            .data
            .startx
            .ok_or_else(|| Error::InvalidModel {
                message: "c4: parent bounds missing startx".to_string(),
            })?;
        let parent_stopy = frame
            .parent_bounds
            .data
            .stopy
            .ok_or_else(|| Error::InvalidModel {
                message: "c4: parent bounds missing stopy".to_string(),
            })?;

        if i == 0 || i % ctx.c4_boundary_in_row == 0 {
            let x = parent_startx + ctx.conf.diagram_margin_x;
            let y0 = parent_stopy + ctx.conf.diagram_margin_y + y;
            frame.current_bounds.set_data(x, x, y0, y0);
        } else {
            let startx = match frame.current_bounds.data.startx {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => parent_startx,
            };
            let stopx = match frame.current_bounds.data.stopx {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => startx,
            };
            let x = if <::core::primitive::f64 as ::core::cmp::PartialOrd>::partial_cmp(&stopx, &startx) != ::core::option::Option::Some(::core::cmp::Ordering::Equal) {
                stopx + ctx.conf.diagram_margin_x
            } else {
                startx
            };
            let y0 = match frame.current_bounds.data.starty {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => parent_stopy,
            };
            frame.current_bounds.set_data(x, x, y0, y0);
        }

        if let Some(shape_indices) = ctx.shape_children.get(&boundary.alias)
            && !shape_indices.is_empty()
        {
            layout_c4_shape_array(&mut frame.current_bounds, shape_indices, ctx, state);
        }

        if let Some(next_boundaries) = ctx.boundary_children.get(&boundary.alias)
            && !next_boundaries.is_empty()
        {
            frame.pending = Some(pending);
            let child_parent_bounds = frame.current_bounds.clone();
            stack.push(C4BoundaryFrame::new(
                next_boundaries.clone(),
                child_parent_bounds,
                ctx,
            ));
            continue;
        }

        finish_c4_boundary_layout(
            &mut frame.parent_bounds,
            &frame.current_bounds,
            pending,
            ctx,
            state,
        );
    }

    Ok(())
}

/// Lays out a typed C4 render model without a compatibility-JSON round trip.
pub(crate) fn layout_c4_diagram_typed(
    model: &C4DiagramModel,
    effective_config: &Value,
    typography: &super::C4TypographyThemePlan,
    measurer: &dyn TextMeasurer,
    container_width: f64,
    container_height: f64,
    screen_available_width: Option<f64>,
) -> Result<C4DiagramLayout> {
    let c4_cfg = C4ConfigView::new(effective_config);
    let conf = c4_cfg
        .layout_settings_with_prepared_fonts(typography.boundary_font(), typography.message_font());

    let c4_shape_in_row = (model.layout.c4_shape_in_row.max(1)) as usize;
    let c4_boundary_in_row = (model.layout.c4_boundary_in_row.max(1)) as usize;

    let mut boundary_children: HashMap<String, Vec<usize>> = HashMap::new();

    for (i, b) in model.boundaries.iter().enumerate() {
        boundary_children
            .entry(b.parent_boundary.clone())
            .or_default()
            .push(i);
    }

    let mut shape_children: HashMap<String, Vec<usize>> = HashMap::new();

    for (i, s) in model.shapes.iter().enumerate() {
        shape_children
            .entry(s.parent_boundary.clone())
            .or_default()
            .push(i);
    }

    let mut state = C4LayoutState {
        boundaries: HashMap::new(),
        shapes: HashMap::new(),
        global_max_x: conf.diagram_margin_x,
        global_max_y: conf.diagram_margin_y,
    };

    let mut screen_bounds = BoundsState::default();
    screen_bounds.set_data(
        conf.diagram_margin_x,
        conf.diagram_margin_x,
        conf.diagram_margin_y,
        conf.diagram_margin_y,
    );
    screen_bounds.data.width_limit = match screen_available_width {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => container_width,
    };

    let root_boundaries = match boundary_children.get("").cloned() {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => ::std::vec::Vec::new(),
    };

    if root_boundaries.is_empty() {
        return Err(Error::InvalidModel {
            message: "c4: expected at least the implicit global boundary".to_string(),
        });
    }

    let ctx = C4LayoutContext {
        model,
        typography,
        conf: &conf,
        c4_shape_in_row,
        c4_boundary_in_row,
        measurer,
        boundary_children: &boundary_children,
        shape_children: &shape_children,
    };

    layout_inside_boundary(&mut screen_bounds, &root_boundaries, &ctx, &mut state)?;

    screen_bounds.data.stopx = Some(state.global_max_x);
    screen_bounds.data.stopy = Some(state.global_max_y);

    let box_startx = match screen_bounds.data.startx {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0.0,
    };
    let box_starty = match screen_bounds.data.starty {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0.0,
    };
    let box_stopx = match screen_bounds.data.stopx {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => conf.diagram_margin_x,
    };
    let box_stopy = match screen_bounds.data.stopy {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => conf.diagram_margin_y,
    };

    let width = (box_stopx - box_startx) + 2.0 * conf.diagram_margin_x;
    let height = (box_stopy - box_starty) + 2.0 * conf.diagram_margin_y;

    let bounds = Some(Bounds {
        min_x: box_startx,
        min_y: box_starty,
        max_x: box_stopx,
        max_y: box_stopy,
    });

    let mut endpoint_rects: HashMap<&str, (Rect, Option<C4NodeShape>)> = HashMap::new();

    for s in model.shapes.iter() {
        let l = {
            let optional_l = state.shapes.get(&s.alias);

            match optional_l {
                Some(matched_l) => matched_l,
                ::core::option::Option::None => continue,
            }
        };

        endpoint_rects.insert(
            s.alias.as_str(),
            (
                Rect {
                    origin: crate::geom::point(l.x, l.y),
                    size: crate::geom::Size::new(l.width, l.height),
                    margin: l.margin,
                },
                Some(c4_node_shape(s)),
            ),
        );
    }


    for boundary in &model.boundaries {
        let layout = {
            let optional_layout = state.boundaries.get(&boundary.alias);

            match optional_layout {
                Some(matched_layout) => matched_layout,
                ::core::option::Option::None => continue,
            }
        };

        endpoint_rects
            .entry(boundary.alias.as_str())
            .or_insert_with(|| {
                (
                    Rect {
                        origin: crate::geom::point(layout.x, layout.y),
                        size: crate::geom::Size::new(layout.width, layout.height),
                        margin: 0.0,
                    },
                    None,
                )
            });
    }

    let rel_font = conf.message_font();
    let mut rels_out: Vec<C4RelLayout> = Vec::new();

    for (i, rel) in model.rels.iter().enumerate() {
        let mut label_text = rel.label.as_str().to_string();

        if model.c4_type == "C4Dynamic" {
            label_text = format!("{}: {}", i + 1, label_text);
        }

        let rel_text_wrap = rel.wrap && conf.wrap;

        let label_limit = measurer.measure(&label_text, &rel_font).width;
        let label_m = measure_c4_text(measurer, &label_text, &rel_font, rel_text_wrap, label_limit);
        let label = C4TextBlockLayout {
            text: label_text,
            y: 0.0,
            width: label_m.width,
            height: label_m.height,
            line_count: label_m.line_count,
            render_plan: None,
        };

        let techn = rel
            .techn
            .as_ref()
            .filter(|t| !t.as_str().is_empty())
            .map(|t| {
                let text = t.as_str().to_string();
                let limit = measurer.measure(&text, &rel_font).width;
                let m = measure_c4_text(measurer, &text, &rel_font, rel_text_wrap, limit);
                C4TextBlockLayout {
                    text,
                    y: 0.0,
                    width: m.width,
                    height: m.height,
                    line_count: m.line_count,
                    render_plan: None,
                }
            });

        let descr = rel
            .descr
            .as_ref()
            .filter(|t| !t.as_str().is_empty())
            .map(|t| {
                let text = t.as_str().to_string();
                let limit = measurer.measure(&text, &rel_font).width;
                let m = measure_c4_text(measurer, &text, &rel_font, rel_text_wrap, limit);
                C4TextBlockLayout {
                    text,
                    y: 0.0,
                    width: m.width,
                    height: m.height,
                    line_count: m.line_count,
                    render_plan: None,
                }
            });

        let (from, from_shape) =
            endpoint_rects
                .get(rel.from_alias.as_str())
                .ok_or_else(|| Error::InvalidModel {
                    message: format!(
                        "c4: relationship references missing from element {}",
                        rel.from_alias
                    ),
                })?;
        let (to, to_shape) =
            endpoint_rects
                .get(rel.to_alias.as_str())
                .ok_or_else(|| Error::InvalidModel {
                    message: format!(
                        "c4: relationship references missing to element {}",
                        rel.to_alias
                    ),
                })?;

        let from_center = LayoutPoint {
            x: to.origin.x + to.size.width / 2.0,
            y: to.origin.y + to.size.height / 2.0,
        };
        let to_center = LayoutPoint {
            x: from.origin.x + from.size.width / 2.0,
            y: from.origin.y + from.size.height / 2.0,
        };
        let start_point = match from_shape {
            Some(shape) => intersect_shape_point(*shape, from, from_center),
            None => intersect_rect_point(from, from_center),
        };
        let end_point = match to_shape {
            Some(shape) => intersect_shape_point(*shape, to, to_center),
            None => intersect_rect_point(to, to_center),
        };

        rels_out.push(C4RelLayout {
            from: rel.from_alias.clone(),
            to: rel.to_alias.clone(),
            rel_type: rel.rel_type.clone(),
            start_point,
            end_point,
            offset_x: rel.offset_x,
            offset_y: rel.offset_y,
            label,
            techn,
            descr,
        });
    }

    let mut boundaries_out = Vec::with_capacity(model.boundaries.len());

    for b in &model.boundaries {
        let Some(l) = state.boundaries.get(&b.alias) else {
            return Err(Error::InvalidModel {
                message: format!("c4: missing boundary layout for {}", b.alias),
            });
        };

        boundaries_out.push(l.clone());
    }

    let mut shapes_out = Vec::with_capacity(model.shapes.len());

    for s in &model.shapes {
        let Some(l) = state.shapes.get(&s.alias) else {
            return Err(Error::InvalidModel {
                message: format!("c4: missing shape layout for {}", s.alias),
            });
        };

        shapes_out.push(l.clone());
    }

    Ok(C4DiagramLayout {
        bounds,
        width,
        height,
        container_width,
        container_height,
        screen_available_width,
        c4_type: model.c4_type.clone(),
        title: model.title.clone(),
        use_max_width: conf.use_max_width,
        boundaries: boundaries_out,
        shapes: shapes_out,
        rels: rels_out,
    })
}
