//! The numbers, written down where someone can read them without running this.

use console_core_color::{Ground, HexColor};
use console_core_color as col;
use console_core_never::Never;

use crate::measure::Row;
use crate::palette::Palette;
use crate::configuration::Configuration;
use crate::terminal::{SLOTS, Shade, Terminal};

pub fn ratio(value: f64) -> Result<String, Never> {
    Ok(format!("{value}"))
}

pub fn asked_lc(value: f64) -> Result<String, Never> {
    Ok(match value > 0.0 {
        true => format!("Contrast {value}"),
        false => "--".to_string(),
    })
}

pub fn write(
    configuration: &Configuration,
    palette: &Palette,
    rows: &[Row],
    terminal: &Terminal,
) -> Result<String, col::Short> {
    let Ok(head) = head(configuration);
    let colors = colors(configuration, palette)?;
    let Ok(asked) = asked(rows);
    let Ok(sixteen) = sixteen(terminal);

    let lines: Vec<String> = head
        .into_iter()
        .chain(colors)
        .chain(asked)
        .chain(sixteen)
        .collect();
    Ok(format!("{}\n", lines.join("\n")))
}

fn head(configuration: &Configuration) -> Result<[String; 20], Never> {
    Ok([
        format!("# {}", configuration.meta.name),
        String::new(),
        configuration.meta.about.clone(),
        String::new(),
        String::from("Written by `console-palette` from `theme/palette.toml`. Every number"),
        String::from("here is measured after the color has been quantised to eight bits a"),
        String::from("channel, which is what a contrast checker reads off the screen and is a"),
        String::from("tenth of a point away from the arithmetic on the same two colors."),
        String::new(),
        String::from("Every pairing is measured twice. The ratio is WCAG 2, which is what the"),
        String::from("law asks for and what a checker will report. `Contrast` is APCA, which knows"),
        String::from("which of the two colors is the paper: it is negative here because"),
        String::from("everything on this desktop is pale ink on a dark ground. A color is"),
        String::from("lifted until it clears both, and on a palette this dark it is almost"),
        String::from("always `Contrast` that decides where it lands."),
        String::new(),
        String::from("## The colors"),
        String::new(),
        String::from("| | color | spent on |"),
        String::from("| --- | --- | --- |"),
    ])
}

fn colors(configuration: &Configuration, palette: &Palette) -> Result<Vec<String>, col::Short> {
    configuration
        .color
        .iter()
        .map(|(name, declared)| {
            let color = palette.must(name)?;

            Ok(format!("| `{name}` | `#{color}` | {} |", declared.spent))
        })
        .collect::<Result<Vec<String>, col::Short>>()
}

fn asked(rows: &[Row]) -> Result<impl Iterator<Item = String> + '_, Never> {
    Ok([
        String::new(),
        String::from("## What was asked of them"),
        String::new(),
        String::from("| front | on | asked | got | | asked | got | |"),
        String::from("| --- | --- | --- | --- | --- | --- | --- | --- |"),
    ]
    .into_iter()
    .chain(rows.iter().map(|row| {
        let Ok(asked) = ratio(row.asked);

        let Ok(grade) = row.grade();

        let Ok(lc) = asked_lc(row.asked_lc);

        let Ok(grade_lc) = row.grade_lc();

        format!(
            "| `{}` | `{}` | {asked}:1 | **{:.2}:1** | {grade} | {lc} | **{:.1}** | {grade_lc} |",
            row.front, row.back, row.got, row.got_lc
        )
    })))
}

fn sixteen(terminal: &Terminal) -> Result<impl Iterator<Item = String> + '_, Never> {
    Ok([
        String::new(),
        String::from("## The terminal"),
        String::new(),
        String::from("| slot | normal | | | bright | | |"),
        String::from("| --- | --- | --- | --- | --- | --- | --- |"),
    ]
    .into_iter()
    .chain(SLOTS.map(|slot| {
        let Ok(normal) = terminal.slot(Shade::Normal, slot);

        let Ok(bright) = terminal.slot(Shade::Bright, slot);

        let Ok(normal_ratio) = col::contrast(HexColor(normal), Ground(&terminal.background));
        let Ok(normal_lc) = col::lc(HexColor(normal), Ground(&terminal.background));
        let Ok(bright_ratio) = col::contrast(HexColor(bright), Ground(&terminal.background));
        let Ok(bright_lc) = col::lc(HexColor(bright), Ground(&terminal.background));

        format!(
            "| {slot} | `#{normal}` | {normal_ratio:.2}:1 | {normal_lc:.1} \
             | `#{bright}` | {bright_ratio:.2}:1 | {bright_lc:.1} |"
        )
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::measure;
    use crate::spend::tests::{blossom, declared_palette};

    fn written() -> String {
        let (configuration, palette) = (declared_palette(), blossom());
        let terminal = Terminal::of(&configuration, &palette).expect("the terminal table is declared");
        let rows = measure(&configuration, &palette).expect("every pairing names a declared color");
        write(&configuration, &palette, &rows, &terminal).expect("a report")
    }

    #[test]
    fn a_whole_ratio_loses_its_nought() {
        assert_eq!(ratio(7.0), Ok("7".to_string()));
        assert_eq!(ratio(4.5), Ok("4.5".to_string()));
        assert_eq!(ratio(1.05), Ok("1.05".to_string()));
        assert_eq!(ratio(10.0), Ok("10".to_string()));
    }

    #[test]
    fn a_pairing_with_no_lc_to_ask_for_says_so_rather_than_saying_nought() {
        assert_eq!(asked_lc(75.0), Ok("Contrast 75".to_string()));
        assert_eq!(asked_lc(0.0), Ok("--".to_string()));
    }

    #[test]
    fn every_color_declared_is_listed_with_what_it_is_spent_on() {
        let report = written();
        for name in declared_palette().color.keys() {
            assert!(report.contains(&format!("| `{name}` |")), "{name} is not listed");
        }
    }

    #[test]
    fn every_pairing_measured_is_reported() {
        let (configuration, palette) = (declared_palette(), blossom());
        let rows = measure(&configuration, &palette).expect("every pairing names a declared color");
        let report = written();
        assert_eq!(report.lines().filter(|l| l.contains(":1 | **")).count(), rows.len());
    }

    #[test]
    fn the_report_is_three_tables_and_says_what_each_is() {
        let report = written();
        for heading in ["## The colors", "## What was asked of them", "## The terminal"] {
            assert!(report.contains(heading), "{heading} is missing");
        }
    }

    #[test]
    fn nothing_in_it_is_reported_as_under() {
        assert!(!written().contains("| under |"));
    }
}
