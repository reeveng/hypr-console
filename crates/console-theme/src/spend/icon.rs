//! The box that stands where an application ships no icon.

use console_colour::Short;
use crate::palette::Palette;

/// An empty box keeps the names in one column: without it those rows sit flush
/// left and the list looks broken rather than merely incomplete.
pub fn spend(palette: &Palette) -> Result<String, Short> {
    let edge = palette.must("edge")?;
    Ok(format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64">
  <!-- Written by console-theme. Where an icon would be, for an
       application that ships none. An empty box keeps the names in one column:
       without it those rows sit flush left and the list looks broken rather
       than merely incomplete. -->
  <rect x="8.5" y="8.5" width="47" height="47" rx="8"
        fill="none" stroke="#{edge}" stroke-width="3"/>
  <circle cx="32" cy="32" r="7" fill="#{edge}" fill-opacity="0.55"/>
</svg>
"##
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spend::tests::blossom;

    #[test]
    fn it_is_drawn_in_the_one_colour_a_border_is_drawn_in() {
        let svg = spend(&blossom()).expect("every colour it spends is declared");
        assert_eq!(svg.matches(&format!("#{}", blossom().must("edge").expect("a declared colour"))).count(), 2);
    }

    #[test]
    fn it_sits_inside_its_own_box() {
        // A stroke is drawn centred on its path, so a 3-wide edge on a rect at
        // 8.5 reaches 7.0 and nothing is clipped by the viewBox.
        let svg = spend(&blossom()).expect("every colour it spends is declared");
        assert!(svg.contains(r#"viewBox="0 0 64 64""#));
        assert!(svg.contains(r#"x="8.5""#) && svg.contains(r#"stroke-width="3""#));
    }
}
