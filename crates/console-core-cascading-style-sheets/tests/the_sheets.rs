use console_core_cascading_style_sheets::{
  AttributeOperator, Declaration, DeclarationTokenizer, Element, Matching, PseudoClass,
  StyleSheet,
};
use console_core_never::Never;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Entry {
  name: &'static str,
  attributes: &'static [(&'static str, &'static str)],
  parent: Option<u64>,
  previous: Option<u64>,
}

const TREE: [Entry; 9] = [
  Entry {
    name: "svg",
    attributes: &[("id", "root")],
    parent: None,
    previous: None,
  },
  Entry {
    name: "g",
    attributes: &[("class", "a b")],
    parent: Some(0),
    previous: None,
  },
  Entry {
    name: "rect",
    attributes: &[("id", "r1"), ("fill", "red")],
    parent: Some(1),
    previous: None,
  },
  Entry {
    name: "rect",
    attributes: &[("class", "b"), ("fill", "blue")],
    parent: Some(1),
    previous: Some(2),
  },
  Entry {
    name: "circle",
    attributes: &[("lang", "en-US")],
    parent: Some(1),
    previous: Some(3),
  },
  Entry {
    name: "text",
    attributes: &[("lang", "en")],
    parent: Some(0),
    previous: Some(1),
  },
  Entry {
    name: "tspan",
    attributes: &[],
    parent: Some(5),
    previous: None,
  },
  Entry {
    name: "g",
    attributes: &[("class", "x")],
    parent: Some(0),
    previous: Some(5),
  },
  Entry {
    name: "rect",
    attributes: &[],
    parent: Some(7),
    previous: None,
  },
];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Node(&'static Entry);

impl Node {
  fn parent_element(&self) -> Result<Option<Self>, Never> {
    Ok(self.0.parent.and_then(|position| {
      let Ok(at) = console_core_number_conversion::index(position);

      TREE.get(at).map(Node)
    }))
  }
}

impl Element for Node {
  fn ancestor_elements(&self) -> impl Iterator<Item = Self> {
    let Ok(first) = self.parent_element();

    TREE.iter().scan(first, |parent, _| {
      let current = parent.take()?;
      let Ok(next) = current.parent_element();

      *parent = next;

      Some(current)
    })
  }

  fn prev_sibling_element(&self) -> Option<Self> {
    self.0.previous.and_then(|position| {
      let Ok(at) = console_core_number_conversion::index(position);

      TREE.get(at).map(Node)
    })
  }

  fn has_local_name(&self, name: &str) -> Matching {
    Matching::from(self.0.name == name)
  }

  fn attribute_matches(&self, local_name: &str, operator: AttributeOperator<'_>) -> Matching {
    Matching::from(
      self
        .0
        .attributes
        .iter()
        .any(|(name, value)| *name == local_name && operator.matches(value) == Ok(Matching::Yes)),
    )
  }

  fn pseudo_class_matches(&self, class: PseudoClass<'_>) -> Matching {
    Matching::from(match class {
      PseudoClass::FirstChild => self.0.previous.is_none(),
      PseudoClass::Lang(lang) => self
        .0
        .attributes
        .iter()
        .any(|(name, value)| *name == "lang" && value.starts_with(lang)),
      PseudoClass::Link
      | PseudoClass::Visited
      | PseudoClass::Hover
      | PseudoClass::Active
      | PseudoClass::Focus => false,
    })
  }
}

fn applied(sheets: &[&str]) -> Result<String, console_core_never::Never> {
  let mut sheet = StyleSheet::default();

  for text in sheets {
    let Ok(()) = sheet.parse_more(text);
  }

  let mut out = format!("rules={}\n", sheet.rules.len());
  let mut at = 0u64;

  for entry in &TREE {
    let mut line = format!("{}{}:", entry.name, at);
    at = at.saturating_add(1);

    for rule in &sheet.rules {
      let Ok(matching) = rule.selector.matches(&Node(entry));

      match matching {
        Matching::Yes => {
          for declaration in &rule.declarations {
            let Ok(()) = write_declaration(declaration, &mut line);
          }

          line.push_str(" |");
        }
        Matching::No => {}
      }
    }

    out.push_str(&line);
    out.push('\n');
  }

  Ok(out)
}

fn write_declaration(
  declaration: &Declaration<'_>,
  line: &mut String,
) -> Result<(), console_core_never::Never> {
  let importance = match ::core::primitive::bool::then_some(declaration.important, ()) {
    ::core::option::Option::Some(()) => "!",
    ::core::option::Option::None => "",
  };

  line.push_str(&format!(
    " {}={}{}",
    declaration.name, declaration.value, importance
  ));

  Ok(())
}

fn tokens(text: &str) -> Result<String, console_core_never::Never> {
  let mut out = String::new();

  for Declaration {
    name,
    value,
    important,
  } in DeclarationTokenizer::from(text)
  {
    match ::core::primitive::bool::then_some(out.is_empty(), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => out.push_str(" ; "),
    }

    let importance = match ::core::primitive::bool::then_some(important, ()) {
      ::core::option::Option::Some(()) => "!",
      ::core::option::Option::None => "",
    };

    out.push_str(&format!("{name}={value}{importance}"));
  }

  Ok(out)
}

const SHEETS: &[&[&str]] = &[
  &["rect { fill: red } .a rect { stroke: blue !important } #r1 { fill: green }"],
  &["g, rect, circle { opacity: .5 }"],
  &[
    "g > rect { a: b } rect + rect { c: d } rect + circle { e: f } g rect { h: i } svg g rect { j: k }",
  ],
  &[
    "[fill] { a: 1 } [fill=red] { b: 2 } [fill=\"blue\"] { c: 3 } [class~=b] { d: 4 } [lang|=en] { e: 5 } [lang|=\"en\"] { f: 6 }",
  ],
  &[
    "rect:first-child { a: b } :hover { c: d } rect:unknown { e: f } g:first-child { g: h } *:first-child { i: j }",
  ],
  &["@import url(x); @media print { rect { fill: red } } rect { a: b }"],
  &["@charset 'x'; rect { a: b } @font-face { font-family: x } circle { c: d }"],
  &["@media x { a { b: c } d { e: f } } rect { x: y }"],
  &["/* c */ rect /* x */ { fill /* y */ : red /* z */ }"],
  &[
    "rect { fill }",
    "rect { : red }",
    "rect { fill:red",
    "{ a:b }",
    "rect > { a:b }",
    "> rect { a:b }",
    "rect, { a:b }",
    "circle { z: 1 } /* never closed",
  ],
  &[
    "rect { fill: url(#a) ; stroke: rgb(1, 2, 3); font-family: 'A B', serif; width: 5px; height: 50%; color: #fff; c2: #abc123; margin: -1.5em +2 .5 }",
  ],
  &[
    "* { a: b } *rect { c: d } rect* { e: f } g  rect { g: h } g\n>\nrect { i: j } g>rect { k: l } g +rect { m: n }",
  ],
  &["rect { a: b !important; c: d ! important; e: f !imp; g: h!important }"],
  &["rect { *zoom: 1; a: b }"],
  &["rect {} circle { ; } g { a:b;;;c:d }"],
  &["é { a: b } .é { c: d } rect\\26 x { e: f }"],
  &[
    "rect:lang(en) { a: b } circle:lang(en) { c: d } rect:lang() { e: f } circle:lang( en ) { g: h } circle:lang(fr) { i: j }",
  ],
  &["rect { a: b; c d; e: f }"],
  &[
    "rect { a: b }",
    "rect { c: d }",
    "#r1 { e: f }",
    "rect { g: h }",
    "g rect { i: j }",
    "rect.b { k: l }",
    "rect[fill] { m: n }",
  ],
  &["rect { a: 'unclosed } circle { b: c }"],
  &["rect { a: \"x\\\"y\" ; b: 'p' } circle { c: url( 'x)' ) }"],
  &["rect { a: b } } circle { c: d }"],
  &["rect { a: b ; } , circle { c: d }"],
  &["", "   ", "/**/", "/* x */ /* y */"],
  &["rect, circle { a: b } g, rect { c: d }"],
  &["rect.a { a: b } .a.b { c: d } .b.a { e: f } rect#r1.b { g: h }"],
  &["rect[fill=red][id=r1] { a: b } rect[fill=red][id=r2] { c: d } [class=\"a b\"] { e: f }"],
  &["rect[fill { a: b } circle { c: d }"],
  &["rect[fill=] { a: b } circle { c: d }"],
  &["rect ~ circle { a: b } circle { c: d }"],
  &["rect::before { a: b } circle { c: d }"],
  &["a { b: c d e f g }", "rect { margin: 0 auto !important }"],
  &["rect { a: 1.5.5 ; b: 5% 10% ; c: +.5 ; d: -x ; e: #-a ; f: #12 }"],
  &["rect { a: foo(bar baz ; b: c }"],
  &["rect { a: b c: d }"],
  &["rect { a:b } /* x */ circle { c:d }"],
  &["rect { a:b }\r\n\tcircle { c:d }\x0c g { e:f }"],
  &["rect > g { a: b } svg > g > rect { c: d } svg > g rect { e: f } svg g > rect { g: h }"],
];

#[test]
fn sheets_apply_as_they_always_did() {
  let mut all = String::new();
  let mut at = 0u64;

  for sheets in SHEETS {
    let Ok(applied) = applied(sheets);

    all.push_str(&format!("== {at}\n{applied}"));
    at = at.saturating_add(1);
  }

  assert_eq!(all, include_str!("sheets.golden"));
}

#[test]
fn declarations_tokenize_as_they_always_did() {
  let cases = [
    "background: url(\"img.png\"); color:red !important",
    "fill:red;stroke:blue",
    "fill:red;;; stroke : blue ;",
    "*zoom:1; a:b",
    "a:b; c; d:e",
    "a: ; b: c",
    "  /* x */ a /* y */ : /* z */ b /* w */ ; c:d",
    "a:b !important;c:d!important",
    "a:'x y';b:\"p;q\"",
    "font: 12px/1.5 'A', serif",
    "a:b c d(e f) g",
    "",
    ";",
    "a",
    "a:",
    ":b",
    "a:b;;c",
    "é:ü",
    "-a:b; --c:d; _e:f",
    "a: #fff #12 #-x",
    "a:1.2.3 4.px 5%x",
  ];
  let mut all = String::new();

  for case in cases {
    let Ok(tokens) = tokens(case);

    all.push_str(&format!("{case:?} -> {tokens}\n"));
  }

  assert_eq!(all, include_str!("tokens.golden"));
}
