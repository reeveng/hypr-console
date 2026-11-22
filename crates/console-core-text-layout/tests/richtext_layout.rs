use console_core_text_layout::{Attrs, Buffer, FontSystem, Metrics, Shaping};

#[test]
fn empty_lines_use_span_metrics() -> Result<(), Box<dyn std::error::Error>> {
  let Ok(mut font_system) = FontSystem::new();

  let Ok(metrics) = Metrics::new(32.0, console_core_text_layout::LineHeight(44.0));
  let Ok(mut buffer) = Buffer::new(&mut font_system, metrics);
  let Ok(mut drawing) = buffer.borrow_with(&mut font_system);

  let Ok(attrs) = Attrs::new();
  let Ok(small_attrs) = attrs.clone().metrics(
    match Metrics::relative(8.0, console_core_text_layout::LineHeightScale(1.2)) {
      Ok(value) => value,
    },
  );

  let Ok(()) = drawing.set_rich_text(
    [
      ("Before", attrs.clone()),
      ("\n\n\nSmall\n\n", small_attrs),
      ("After", attrs.clone()),
    ],
    &attrs,
    Shaping::Advanced,
    None,
  );

  let Ok(()) = drawing.set_size(Some(500.0), Some(500.0));

  let Ok(layout_runs) = drawing.layout_runs();
  let line_heights: Vec<f32> = layout_runs.map(|run| run.line_height).collect();

  assert_eq!(line_heights.len(), 6);

  for (index, expected) in [(0, 44.0), (1, 9.6), (2, 9.6), (4, 9.6), (5, 44.0)] {
    let found = line_heights.get(index).ok_or("missing layout run")?;
    assert!(
      (found - expected).abs() < 0.1,
      "line {index}: expected {expected}, got {found}"
    );
  }

  Ok(())
}
