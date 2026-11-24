use super::super::*;

pub(super) fn write_palette_css(
    out: &mut impl SvgOutput,
    id: SvgDiagramId<'_>,
    binding: &crate::block::BlockCssThemeBinding,
    options: &SvgExecution<'_>,
) -> Result<()> {
    let look = &binding.palette_look;

    for (index, (border, background)) in binding.palette.iter().enumerate() {
        options.checkpoint_emit()?;
        let _ = write!(
            out,
            r#"#{id} [data-look="{look}"][data-color-id="color-{index}"].node rect.composite{{stroke:{};"#,
            border
        );

        if let Some(background_2) = background {
            let _ = write!(out, "fill:{background};", background = background_2);
        }

        out.push('}');
        out.checkpoint()?;
    }

    Ok(())
}
