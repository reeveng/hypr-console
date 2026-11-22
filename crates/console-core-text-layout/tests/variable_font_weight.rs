use std::path::PathBuf;

use console_core_text_layout::{
  Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Weight, fontdb,
};

#[test]
fn variable_font_all_weights_match() -> Result<(), Box<dyn std::error::Error>> {
  let repo_dir = env!("CARGO_MANIFEST_DIR");
  let fonts_path = PathBuf::from(&repo_dir).join("fonts");

  let Ok(mut font_system) = FontSystem::new();
  let Ok(db_mut) = font_system.db_mut();
  let Ok(()) = db_mut.load_font_data(match std::fs::read(fonts_path.join("InterVariable.ttf")) {
    Ok(value) => value,
    Err(error) => return Err(Box::new(error)),
  });

  for w in [100, 200, 300, 400, 500, 600, 700, 800, 900] {
    let Ok(metrics) = Metrics::new(16.0, console_core_text_layout::LineHeight(20.0));
    let Ok(mut buffer) = Buffer::new(&mut font_system, metrics);

    let glyph_font_ids: Vec<fontdb::ID>;

    {
      let Ok(mut buffer_2) = buffer.borrow_with(&mut font_system);
      let Ok(attrs_family) = match Attrs::new() {
        Ok(value) => value,
      }
      .family(Family::Name("Inter Variable"));
      let Ok(attrs) = attrs_family.weight(Weight(w));

      let Ok(()) = buffer_2.set_size(Some(300.0), Some(100.0));

      let Ok(()) = buffer_2.set_text("Hello world", &attrs, Shaping::Advanced, None);

      let Ok(()) = buffer_2.shape_until_scroll(console_core_text_layout::Pruning::Discard);

      let Ok(layout_runs) = buffer_2.layout_runs();
      glyph_font_ids = layout_runs
        .flat_map(|run| run.glyphs.iter().map(|g| g.font_id))
        .collect();
    }

    assert!(!glyph_font_ids.is_empty(), "Weight {w}: no glyphs produced");

    for id in &glyph_font_ids {
      let Ok(db) = font_system.db();
      let Ok(db_face) = db.face(*id);
      let face = match db_face {
        Some(value) => value,
        None => return Err(Box::new(std::io::Error::other("Fixture value is missing"))),
      };
      let family = &(match (face.families).first() {
        Some(value) => value,
        None => {
          return Err(Box::new(std::io::Error::other(
            "Fixture index is outside its collection",
          )));
        }
      })
      .0;
      assert!(
        family.contains("Inter"),
        "Weight {w}: expected Inter, got \"{family}\""
      );
    }
  }

  Ok(())
}
