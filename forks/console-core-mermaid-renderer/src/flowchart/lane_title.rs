use unicode_segmentation::{ClusterKind, UnicodeSegmentation};

#[must_use]
pub(crate) fn zero_width_lane_title_rows(
    title: &str,
) -> Vec<Vec<(String, crate::text::MermaidMarkdownWordType)>> {
    let title_3 = title
        .strip_prefix('`')
        .and_then(|title_2| title_2.strip_suffix('`'))
        .unwrap_or(title);
    crate::text::mermaid_markdown_to_lines(title_3, true)
        .into_iter()
        .flat_map(|line| {
            if line.is_empty() {
                return vec![Vec::new()];
            }

            line.into_iter()
                .flat_map(|(word, style)| {
                    let visible =
                        crate::text::SvgTextEntityMode::CreateTextSource.visible_word(&word);
                    let Ok(graphemes) = visible.graphemes(ClusterKind::Extended);
                    graphemes
                        .map(|grapheme| vec![(grapheme.to_string(), style)])
                        .collect::<Vec<_>>()
                })
                .collect()
        })
        .collect()
}
