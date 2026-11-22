use console_core_font_metrics::GlyphId;
use console_core_never::Never;
use std::cell::Cell;
use std::collections::HashMap;
use std::fmt;

pub(crate) struct GlyphSimulator {
  mappings: Cell<Mappings>,
}

#[derive(Default)]
struct Mappings {
  glyph_to_string: HashMap<GlyphId, String>,
  string_to_glyph: HashMap<String, GlyphId>,
  next: u32,
}

impl fmt::Debug for GlyphSimulator {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter
      .debug_struct("GlyphSimulator")
      .finish_non_exhaustive()
  }
}

impl GlyphSimulator {
  pub(crate) fn new() -> Result<Self, Never> {
    let mut mappings = Mappings {
      next: 1,
      ..Mappings::default()
    };
    mappings
      .glyph_to_string
      .insert(GlyphId::NOTDEF, "notdef".to_string());
    mappings
      .string_to_glyph
      .insert("notdef".to_string(), GlyphId::NOTDEF);

    Ok(Self {
      mappings: Cell::new(mappings),
    })
  }

  pub(crate) fn string_to_glyph(&self, string: &str) -> Result<GlyphId, Never> {
    let mut mappings = self.mappings.take();
    let glyph = match mappings.string_to_glyph.get(string) {
      Some(known) => *known,
      None => {
        let glyph = GlyphId::from(mappings.next);
        mappings.string_to_glyph.insert(string.to_string(), glyph);
        mappings.glyph_to_string.insert(glyph, string.to_string());
        mappings.next = mappings.next.wrapping_add(1);

        glyph
      }
    };
    self.mappings.set(mappings);

    Ok(glyph)
  }

  pub(crate) fn glyph_to_string(&self, glyph: GlyphId) -> Result<Option<String>, Never> {
    let mappings = self.mappings.take();
    let text = mappings.glyph_to_string.get(&glyph).cloned();
    self.mappings.set(mappings);

    Ok(text)
  }
}

#[cfg(test)]
mod tests {
  use super::GlyphSimulator;

  #[test]
  fn assigned_glyphs_can_be_found_by_name_and_identifier() {
    let Ok(simulated) = GlyphSimulator::new();
    let Ok(first) = simulated.string_to_glyph("drawn");
    let Ok(again) = simulated.string_to_glyph("drawn");
    let Ok(second) = simulated.string_to_glyph("another");
    let Ok(text) = simulated.glyph_to_string(first);
    assert_eq!(first, again);
    assert_ne!(first, second);
    assert_eq!(text.as_deref(), Some("drawn"));
  }
}
