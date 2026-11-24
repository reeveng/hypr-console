use super::super::*;
use super::SequenceEmitCheckpoints;
use super::geometry::node_left_top;
use super::model::SequenceSvgModel;
use merman_model::families::sequence::{SequenceControlKind, SequenceControlRole};
use console_core_perfect_hash::collections::FixedMap;

#[derive(Debug, Clone, Copy)]
pub(super) struct SequenceFrameRenderOptions<'a> {
    pub(super) actor_text_style: &'a TextStyle,
    pub(super) box_layouts: &'a [crate::sequence::SequenceBoxLayout],
    pub(super) box_title_height: f64,
    pub(super) box_height: f64,
    pub(super) box_margin: f64,
    pub(super) box_text_margin: f64,
    pub(super) rect_default_fill: &'a str,
}

#[must_use]
fn box_rect_geometry(
    box_layout: &crate::sequence::SequenceBoxLayout,
    BoxRectGeometryArguments { height, margin }: BoxRectGeometryArguments,
) -> Option<[f64; 4]> {
    let padding = margin * 2.0;
    let checked_value = box_layout.x?;
    Some([
        checked_value - padding,
        -padding * 0.25,
        box_layout.width + padding * 2.0,
        height + padding * 0.75,
    ])
}

struct BoxRectGeometryArguments {
    height: f64,
    margin: f64,
}

pub(super) fn builtin_box_paint_bounds(
    boxes: &[crate::sequence::SequenceBoxLayout],
    height: f64,
    margin: f64,
    theme: &crate::sequence::SequenceCompatBinding,
    checkpoints: SequenceEmitCheckpoints<'_>,
) -> Result<Option<Bounds>> {
    if !theme.is_neo || theme.drop_shadow.as_str() != "url(#drop-shadow)" {
        return Ok(None);
    }

    let mut bounds: Option<Bounds> = None;

    for (index, box_layout) in boxes.iter().enumerate() {
        checkpoints.checkpoint_loop(index)?;

        let (x, y, width, height_2) = {
            let optional_x = box_rect_geometry(box_layout, self::BoxRectGeometryArguments { height, margin });

            match optional_x {
                Some([matched_x, matched_y, matched_width, matched_height_2]) => (matched_x, matched_y, matched_width, matched_height_2),
                ::core::option::Option::None => continue,
            }
        };


        let half_stroke = 0.5;
        let offset = super::super::look_defs::NEO_SHADOW_OFFSET_PX;
        let paint = Bounds {
            min_x: x - half_stroke,
            min_y: y - half_stroke,
            max_x: x + width + half_stroke + offset,
            max_y: y + height_2 + half_stroke + offset,
        };

        if let Some(bounds_2) = &mut bounds {
            bounds_2.min_x = bounds_2.min_x.min(paint.min_x);
            bounds_2.min_y = bounds_2.min_y.min(paint.min_y);
            bounds_2.max_x = bounds_2.max_x.max(paint.max_x);
            bounds_2.max_y = bounds_2.max_y.max(paint.max_y);
        } else {
            bounds = Some(paint);
        }
    }

    Ok(bounds)
}

pub(super) fn render_sequence_box_frames_and_rect_blocks(
    out: &mut impl SvgOutput,
    model: &SequenceSvgModel,
    nodes_by_id: &FixedMap<&str, &LayoutNode>,
    options: SequenceFrameRenderOptions<'_>,
    label_ctx: &super::actor_shapes::ActorLabelContext<'_>,
    checkpoints: SequenceEmitCheckpoints<'_>,
) -> Result<()> {
    let actor_typography = label_ctx.typography;
    let typography_receipt = label_ctx.typography_receipt;


    let max_box_title_height = options.box_title_height;

    checkpoints.checkpoint()?;

    for (emission_index, (b, box_layout)) in model
        .boxes
        .iter()
        .zip(options.box_layouts)
        .rev()
        .enumerate()
    {
        checkpoints.checkpoint_loop(emission_index)?;

        let Some([x, y, w, h]) =
            box_rect_geometry(box_layout, self::BoxRectGeometryArguments { height: options.box_height, margin: options.box_margin })
        else {
            if b.name.is_some() {
                typography_receipt
                    .record_missing_text_effect(crate::sequence::SequenceTextSurface::BoxTitle);
            }

            continue;
        };

        out.push_str("<g>");
        let _ = write!(
            out,
            r#"<rect x="{x}" y="{y}" fill="{fill}" stroke="rgb(0,0,0, 0.5)" width="{w}" height="{h}" class="rect"/>"#,
            x = fmt(x),
            y = fmt(y),
            w = fmt(w),
            h = fmt(h),
            fill = escape_xml_display(&b.fill),
        );

        if let Some(name) = box_layout.label.as_deref().filter(|name| !name.is_empty()) {
            let cx = x + (w / 2.0);


            let text_y = options.box_text_margin + max_box_title_height / 2.0;
            let lines = crate::text::split_html_br_lines(name);
            let count = lines.len().max(1) as f64;

            for (index, raw) in lines.into_iter().enumerate() {
                checkpoints.checkpoint_loop(index)?;
                let decoded = crate::sequence::prepare_sequence_text_line(raw);
                let name_2 = decoded.as_ref();
                let dy = (index as f64 - (count - 1.0) / 2.0) * options.actor_text_style.font_size;
                let application = label_ctx.write_shadow(
                    out,
                    name_2,
                    cx,
                    text_y + dy,
                    options.actor_text_style.font_size,
                    super::text_effect::TextShadowBaseline::Middle,
                )?;
                let filter = match application
                    .as_ref()
                    .map(|a| format!(" filter=\"{}\"", escape_attr(&a.filter))) {
                    ::core::option::Option::Some(present) => present,
                    ::core::option::Option::None => ::std::string::String::new(),
                };
                let style = actor_typography.terminal_style(
                    "text-anchor: middle",
                    format!(
                        "text-anchor: middle; {}",
                        super::settings::sequence_text_style_attribute(options.actor_text_style)
                    ),
                );
                let _ = write!(
                    out,
                    r#"<text x="{x}" y="{y}" dominant-baseline="central" alignment-baseline="central" class="text" style="{style}"{filter}><tspan x="{x}" dy="{dy}">{text}</tspan></text>"#,
                    x = fmt(cx),
                    y = fmt(text_y),
                    dy = fmt(dy),
                    style = escape_attr_display(&style),
                    text = super::escape_prepared_text(name_2)
                );
                out.checkpoint()?;
                label_ctx.record_shadow(
                    application.as_ref(),
                    name_2,
                    crate::sequence::SequenceTextSurface::BoxTitle,
                    0.0,
                );
                typography_receipt
                    .record_terminal_text(crate::sequence::SequenceTextSurface::BoxTitle);
            }
        }

        out.push_str("</g>");
    }


    {
        #[derive(Debug, Clone, Copy)]
        struct RectBlock<'a> {
            fill: &'a str,
            x: f64,
            y: f64,
            w: f64,
            h: f64,
        }

        #[must_use]
        fn contains(a: &RectBlock<'_>, b: &RectBlock<'_>) -> bool {
            const EPS: f64 = 1e-9;
            a.x <= b.x + EPS
                && a.y <= b.y + EPS
                && (a.x + a.w) >= (b.x + b.w) - EPS
                && (a.y + a.h) >= (b.y + b.h) - EPS
        }

        let mut rects: Vec<RectBlock<'_>> = Vec::with_capacity(model.messages.len());

        for (message_index, msg) in model.messages.iter().enumerate() {
            checkpoints.checkpoint_loop(message_index)?;

            if !msg.control_semantics().is_some_and(|semantics| {
                semantics.kind == SequenceControlKind::Rect
                    && semantics.role == SequenceControlRole::Start
            }) {
                continue;
            }

            let explicit_fill = msg.message_text();
            let fill = if explicit_fill.is_empty() {
                options.rect_default_fill
            } else {
                explicit_fill
            };
            let node_id = format!("rect-{}", msg.id);

            let n = {
                let optional_n = nodes_by_id.get(node_id.as_str()).copied();

                match optional_n {
                    Some(matched_n) => matched_n,
                    ::core::option::Option::None => continue,
                }
            };

            let (x, y) = node_left_top(n);
            rects.push(RectBlock {
                fill,
                x,
                y,
                w: n.width,
                h: n.height,
            });
        }




        checkpoints.checkpoint()?;
        rects.sort_by(|a, b| {
            if contains(a, b) && !contains(b, a) {
                return std::cmp::Ordering::Less;
            }

            if contains(b, a) && !contains(a, b) {
                return std::cmp::Ordering::Greater;
            }

            match b.y.partial_cmp(&a.y) {
                ::core::option::Option::Some(present) => present,
                ::core::option::Option::None => std::cmp::Ordering::Equal,
            }
                .then_with(|| match a.x.partial_cmp(&b.x) {
                    ::core::option::Option::Some(present) => present,
                    ::core::option::Option::None => std::cmp::Ordering::Equal,
                })
        });

        checkpoints.checkpoint()?;

        for (rect_index, r) in rects.into_iter().enumerate() {
            checkpoints.checkpoint_loop(rect_index)?;
            let _ = write!(
                out,
                r#"<rect x="{x}" y="{y}" fill="{fill}" width="{w}" height="{h}" class="rect"/>"#,
                x = fmt(r.x),
                y = fmt(r.y),
                w = fmt(r.w),
                h = fmt(r.h),
                fill = escape_xml_display(r.fill)
            );
        }
    }

    checkpoints.checkpoint()
}
