//! Resolves symbolic script data into the compact autohint tables.

mod data;

use data::{FEATURES, SCRIPTS};
use std::collections::BTreeMap;
use std::fmt::{self, Write};
use std::io::{self, Write as _};

enum Group {
  Default,
  Cjk,
  Indic,
}

impl fmt::Display for Group {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    to.write_str(match self {
      Self::Default => "Default",
      Self::Cjk => "Cjk",
      Self::Indic => "Indic",
    })
  }
}

struct Feature {
  tag: &'static str,
  name: &'static str,
}

struct Script {
  tag: &'static str,
  name: &'static str,
  group: Group,
  features: bool,
  hint_top_to_bottom: bool,
  standard: &'static str,
  base: &'static [(u32, u32)],
  non_base: &'static [(u32, u32)],
  blues: &'static [(&'static str, &'static [&'static str])],
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
  Base,
  NonBase,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Coverage {
  style: usize,
  kind: Kind,
}

struct Style {
  constant: String,
  name: String,
  script: usize,
  feature: Option<&'static str>,
}

fn tag_source(tag: &str) -> String {
  format!("{{ let Ok(value) = Tag::new(b{tag:?}); value }}")
}

fn glyph_source(glyphs: &str) -> String {
  let mut literal = String::from("\"");
  for glyph in glyphs.chars() {
    match glyph {
      '\u{200d}' => literal.push_str("\\u{200d}"),
      '\\' => literal.push_str("\\\\"),
      '"' => literal.push_str("\\\""),
      control if control.is_control() => literal.extend(control.escape_default()),
      other => literal.push(other),
    }
  }
  literal.push('"');
  literal
}

fn add_script(out: &mut String, script: &Script) -> fmt::Result {
  let mut unicode_tag = script.tag.to_ascii_lowercase();
  if let Some(initial) = unicode_tag.get_mut(..1) {
    initial.make_ascii_uppercase();
  }

  writeln!(out, "    ScriptClass {{")?;
  writeln!(out, "        name: {:?},", script.name)?;
  writeln!(out, "        group: ScriptGroup::{},", script.group)?;
  writeln!(out, "        tag: ({}),", tag_source(&unicode_tag))?;
  writeln!(
    out,
    "        hint_top_to_bottom: {},",
    script.hint_top_to_bottom
  )?;
  writeln!(out, "        std_chars: {},", glyph_source(script.standard))?;
  if script.blues.is_empty() {
    writeln!(out, "        blues: &[],")?;
  } else {
    writeln!(out, "        blues: &[")?;
    for &(text, flags) in script.blues {
      if let Some((first, rest)) = flags.split_first() {
        write!(
          out,
          "            ({}, BlueZones::{first}",
          glyph_source(text)
        )?;
        for flag in rest {
          write!(out, ".union(BlueZones::{flag})")?;
        }
        writeln!(out, "),")?;
      }
    }
    writeln!(out, "        ],")?;
  }
  writeln!(out, "    }},")
}

fn collect_coverage(coverage: &mut BTreeMap<u32, Coverage>, script: &Script, style: usize) {
  // First script wins when the FreeType source ranges overlap.
  for &(first, last) in script.base {
    for code in first..=last {
      coverage.entry(code).or_insert(Coverage {
        style,
        kind: Kind::Base,
      });
    }
  }
  for &(first, last) in script.non_base {
    for code in first..=last {
      if let Some(existing) = coverage.get_mut(&code).filter(|entry| entry.style == style) {
        existing.kind = Kind::NonBase;
      }
    }
  }
}

fn add_ranges(out: &mut String, coverage: BTreeMap<u32, Coverage>) -> fmt::Result {
  let mut entries = coverage.into_iter().peekable();
  while let Some((first, props)) = entries.next() {
    let mut last = first;
    while let Some((code, _)) = entries.next_if(|&(code, next)| code == last + 1 && next == props)
    {
      last = code;
    }
    let constructor = match props.kind {
      Kind::Base => "base_range",
      Kind::NonBase => "non_base_range",
    };
    writeln!(out, "    {constructor}({first}, {last}, {}),", props.style)?;
  }
  Ok(())
}

fn generate() -> Result<String, fmt::Error> {
  let mut out =
    String::from("\n\n\n\n#[rustfmt::skip]\npub const SCRIPT_CLASSES: &[ScriptClass] = &[\n");
  let mut styles = Vec::new();
  let mut coverage = BTreeMap::new();

  for (index, script) in SCRIPTS.iter().enumerate() {
    add_script(&mut out, script)?;
    if script.features {
      for feature in FEATURES {
        styles.push(Style {
          constant: format!("{}_{}", script.tag, feature.tag.to_ascii_uppercase()),
          name: format!("{} {}", script.name, feature.name),
          script: index,
          feature: Some(feature.tag),
        });
      }
    }
    collect_coverage(&mut coverage, script, styles.len());
    styles.push(Style {
      constant: script.tag.to_string(),
      name: script.name.to_string(),
      script: index,
      feature: None,
    });
  }

  writeln!(out, "];\n")?;
  writeln!(
    out,
    "#[expect(dead_code, reason = \"the generated index names every script class for lookup by tag\")]\nimpl ScriptClass {{"
  )?;
  for (index, script) in SCRIPTS.iter().enumerate() {
    writeln!(out, "    pub const {}: usize = {index};", script.tag)?;
  }

  writeln!(
    out,
    "}}\n\n#[rustfmt::skip]\npub const STYLE_CLASSES: &[StyleClass] = &["
  )?;
  for (index, style) in styles.iter().enumerate() {
    let feature = style.feature.map_or_else(
      || "None".to_string(),
      |tag| format!("Some({})", tag_source(tag)),
    );
    writeln!(
      out,
      "    StyleClass {{ name: {:?}, index: {index}, script: &SCRIPT_CLASSES[{}], feature: {feature} }},",
      style.name, style.script
    )?;
  }
  writeln!(
    out,
    "];\n\n#[expect(dead_code, reason = \"the generated index names every style class for lookup by tag\")]\nimpl StyleClass {{"
  )?;
  for (index, style) in styles.iter().enumerate() {
    writeln!(out, "    pub const {}: usize = {index};", style.constant)?;
  }
  writeln!(
    out,
    "}}\n\n#[rustfmt::skip]\npub(super) const STYLE_RANGES: &[StyleRange] = &["
  )?;
  add_ranges(&mut out, coverage)?;
  writeln!(out, "];\n")?;
  Ok(out)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  io::stdout().lock().write_all(generate()?.as_bytes())?;
  Ok(())
}
