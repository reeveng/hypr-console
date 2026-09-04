//! The numbers, written down where somebody can read them without running this.

use console_colour as col;

use crate::measure::Row;
use crate::palette::Palette;
use crate::spec::Spec;
use crate::terminal::{SLOTS, Shade, Terminal};

/// A ratio as it is written: `7:1` rather than `7.0:1`, `4.5:1` as it is.
pub fn ratio(value: f64) -> String {
    format!("{value}")
}

/// An `Lc` floor as it is written, and a dash where there is no claim to make.
pub fn asked_lc(value: f64) -> String {
    match value > 0.0 {
        true => format!("Lc {value}"),
        false => "--".to_string(),
    }
}

pub fn write(
    spec: &Spec,
    palette: &Palette,
    rows: &[Row],
    terminal: &Terminal,
) -> Result<String, col::Short> {
    let head = [
        format!("# {}", spec.meta.name),
        String::new(),
        spec.meta.about.clone(),
        String::new(),
        "Written by `console-theme` from `theme/palette.toml`. Every number".into(),
        "here is measured after the colour has been quantised to eight bits a".into(),
        "channel, which is what a contrast checker reads off the screen and is a".into(),
        "tenth of a point away from the arithmetic on the same two colours.".into(),
        String::new(),
        "Every pairing is measured twice. The ratio is WCAG 2, which is what the".into(),
        "law asks for and what a checker will report. `Lc` is APCA, which knows".into(),
        "which of the two colours is the paper: it is negative here because".into(),
        "everything on this desktop is pale ink on a dark ground. A colour is".into(),
        "lifted until it clears both, and on a palette this dark it is almost".into(),
        "always `Lc` that decides where it lands.".into(),
        String::new(),
        "## The colours".into(),
        String::new(),
        "| | colour | spent on |".into(),
        "| --- | --- | --- |".into(),
    ];

    let colours = spec
        .colour
        .iter()
        .map(|(name, declared)| {
            Ok(format!("| `{name}` | `#{}` | {} |", palette.must(name)?, declared.spent))
        })
        .collect::<Result<Vec<String>, col::Short>>()?;

    let asked = [
        String::new(),
        "## What was asked of them".into(),
        String::new(),
        "| front | on | asked | got | | asked | got | |".into(),
        "| --- | --- | --- | --- | --- | --- | --- | --- |".into(),
    ]
    .into_iter()
    .chain(rows.iter().map(|row| {
        format!(
            "| `{}` | `{}` | {}:1 | **{:.2}:1** | {} | {} | **{:.1}** | {} |",
            row.front,
            row.back,
            ratio(row.asked),
            row.got,
            row.grade(),
            asked_lc(row.asked_lc),
            row.got_lc,
            row.grade_lc()
        )
    }));

    let sixteen = [
        String::new(),
        "## The terminal".into(),
        String::new(),
        "| slot | normal | | | bright | | |".into(),
        "| --- | --- | --- | --- | --- | --- | --- |".into(),
    ]
    .into_iter()
    .chain(SLOTS.map(|slot| {
        let (normal, bright) = (
            terminal.slot(Shade::Normal, slot),
            terminal.slot(Shade::Bright, slot),
        );
        format!(
            "| {slot} | `#{normal}` | {:.2}:1 | {:.1} | `#{bright}` | {:.2}:1 | {:.1} |",
            col::contrast(normal, &terminal.background),
            col::lc(normal, &terminal.background),
            col::contrast(bright, &terminal.background),
            col::lc(bright, &terminal.background)
        )
    }));

    let lines: Vec<String> = head
        .into_iter()
        .chain(colours)
        .chain(asked)
        .chain(sixteen)
        .collect();
    Ok(format!("{}\n", lines.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::measure;
    use crate::spend::tests::{blossom, palette_spec};

    fn written() -> String {
        let (spec, palette) = (palette_spec(), blossom());
        let terminal = Terminal::of(&spec, &palette).expect("the terminal table is declared");
        let rows = measure(&spec, &palette).expect("every pairing names a declared colour");
        write(&spec, &palette, &rows, &terminal).expect("a report")
    }

    #[test]
    fn a_whole_ratio_loses_its_nought() {
        assert_eq!(ratio(7.0), "7");
        assert_eq!(ratio(4.5), "4.5");
        assert_eq!(ratio(1.05), "1.05");
        assert_eq!(ratio(10.0), "10");
    }

    #[test]
    fn a_pairing_with_no_lc_to_ask_for_says_so_rather_than_saying_nought() {
        assert_eq!(asked_lc(75.0), "Lc 75");
        assert_eq!(asked_lc(0.0), "--");
    }

    #[test]
    fn every_colour_declared_is_listed_with_what_it_is_spent_on() {
        let report = written();
        for name in palette_spec().colour.keys() {
            assert!(report.contains(&format!("| `{name}` |")), "{name} is not listed");
        }
    }

    #[test]
    fn every_pairing_measured_is_reported() {
        let (spec, palette) = (palette_spec(), blossom());
        let rows = measure(&spec, &palette).expect("every pairing names a declared colour");
        let report = written();
        let reported = report.lines().filter(|l| l.contains(":1 | **")).count();
        assert_eq!(reported, rows.len());
    }

    #[test]
    fn the_report_is_three_tables_and_says_what_each_is() {
        let report = written();
        for heading in ["## The colours", "## What was asked of them", "## The terminal"] {
            assert!(report.contains(heading), "{heading} is missing");
        }
    }

    #[test]
    fn nothing_in_it_is_reported_as_under() {
        // The report is only written when the palette clears what it declares,
        // so an "under" in it would mean the gate above it let something past.
        assert!(!written().contains("| under |"));
    }
}
