//! Mermaid blocks in a Markdown note, rendered through the maintained native
//! parser and SVG libraries. Previewing never writes back to the source.

use std::sync::Arc;
use std::time::Duration;

use console_core_geometry::Size;
use console_core_never::Never;
use console_core_number_conversion::{fitted, whole_u32};
use console_core_shapes::Pixels;
use merman::svg::SvgPipeline;
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer, SvgRequest};
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};

const LONGEST: u32 = 50_000;

const LARGEST: u32 = 2048;

#[derive(Debug)]
pub enum DiagramError {
    TooLong,
    Render(Box<merman::RenderError>),
    Svg(console_core_scalable_vector_graphics::Error),
    NoDiagram,
    NoPixels,
}

impl std::fmt::Display for DiagramError {
    fn fmt(&self, to: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiagramError::TooLong => write!(to, "This diagram is longer than {LONGEST} characters. Shorten it to preview it."),
            DiagramError::Render(fault) => write!(to, "This diagram could not be drawn: {fault}"),
            DiagramError::Svg(fault) => write!(to, "This diagram could not be displayed: {fault}"),
            DiagramError::NoDiagram => write!(to, "There is no Mermaid diagram in this block. Return to editing to add one."),
            DiagramError::NoPixels => write!(to, "There is not enough room to draw this diagram."),
        }
    }
}

impl std::error::Error for DiagramError {}

pub fn sources(text: &str) -> Result<Vec<String>, Never> {
    let mut found = Vec::new();
    let mut source = None::<String>;

    for event in Parser::new(text) {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                source = match info.split_whitespace().next() == Some("mermaid") {
                    true => Some(String::new()),
                    false => None,
                };
            },
            Event::Text(text) => match &mut source {
                Some(block) => block.push_str(&text),
                None => {},
            },
            Event::End(TagEnd::CodeBlock) => match source.take() {
                Some(block) => found.push(block),
                None => {},
            },
            Event::Start(_)
            | Event::End(_)
            | Event::Code(_)
            | Event::InlineMath(_)
            | Event::DisplayMath(_)
            | Event::Html(_)
            | Event::InlineHtml(_)
            | Event::FootnoteReference(_)
            | Event::SoftBreak
            | Event::HardBreak
            | Event::Rule
            | Event::TaskListMarker(_) => {},
        }
    }

    Ok(found)
}

pub fn drawn(source: &str, within: Size<u32>) -> Result<Pixels, DiagramError> {
    let Ok(long) = fitted::<_, u32>(source.chars().count());

    match long > LONGEST {
        true => return Err(DiagramError::TooLong),
        false => {},
    }

    let request = SvgRequest { pipeline: Some(SvgPipeline::resvg_safe()), ..SvgRequest::default() };
    let control = OperationControl::new().with_deadline(Duration::from_secs(2));
    let output = Renderer::new().render(RenderRequest::svg(source, control, request)).map_err(|fault| DiagramError::Render(Box::new(fault)))?;

    #[cfg_attr(
        dylint_lib = "explicit016_no_wildcard_arm",
        allow(
            explicit016_no_wildcard_arm,
            reason = "RenderOutput is a foreign non_exhaustive enum without an SVG accessor; this SVG request rejects every other output format"
        )
    )]
    let svg = match output {
        RenderOutput::Svg(Some(svg)) => svg,
        _ => return Err(DiagramError::NoDiagram),
    };

    rasterized(svg.svg(), within)
}

fn rasterized(svg: &str, within: Size<u32>) -> Result<Pixels, DiagramError> {
    let mut options = console_core_scalable_vector_graphics::Options::default();

    options.fontdb_mut().load_system_fonts();
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);

    let tree = console_core_scalable_vector_graphics::Tree::from_str(svg, &options).map_err(DiagramError::Svg)?;
    let size = tree.size();
    let Ok(wide) = fitted::<u32, u16>(within.width.clamp(1, LARGEST));
    let Ok(tall) = fitted::<u32, u16>(within.height.clamp(1, LARGEST));
    let scale = (f32::from(wide) / size.width()).min(f32::from(tall) / size.height()).min(1.0);
    let Ok(width) = whole_u32(f64::from(size.width() * scale));
    let Ok(height) = whole_u32(f64::from(size.height() * scale));
    let mut pixmap = console_core_path_rasterizer::Pixmap::new(width.max(1), height.max(1)).ok_or(DiagramError::NoPixels)?;
    let transform = console_core_path_rasterizer::Transform::from_scale(scale, scale);

    console_core_scalable_vector_graphics_rasterizer::render(&tree, transform, &mut pixmap.as_mut());

    let bytes = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();

            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();

    Ok(Pixels { width: pixmap.width(), height: pixmap.height(), stride: pixmap.width().saturating_mul(4), bytes: Arc::new(bytes) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_fences_reach_the_native_renderer() -> Result<(), Box<dyn std::error::Error>> {
        let note = "Plans\n\n````text\n```mermaid\nnot a diagram\n```\n````\n\n> ~~~mermaid\n> flowchart LR\n>   A[Write] --> B[Read]\n> ~~~\n\n```mermaid\nsequenceDiagram\n  Alice->>Bob: Hello\n```\n";
        let Ok(blocks) = sources(note);

        assert_eq!(blocks.len(), 2);

        for source in blocks {
            let pixels = drawn(&source, Size { width: 400, height: 300 })?;
            let Ok(bytes) = console_core_number_conversion::index(pixels.stride.saturating_mul(pixels.height));

            assert!(pixels.width > 0 && pixels.width <= 400);
            assert!(pixels.height > 0 && pixels.height <= 300);
            assert_eq!(pixels.bytes.len(), bytes);
            let (colors, _remainder) = pixels.bytes.as_chunks::<4>();

            assert!(colors.iter().any(|pixel| pixel.get(3).copied() != Some(0)));
        }

        assert!(matches!(drawn("this is not a diagram", Size { width: 400, height: 300 }), Err(DiagramError::Render(_))));

        Ok(())
    }
}
