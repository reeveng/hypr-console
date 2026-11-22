use crate::Property;

#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Test {
  pub qual: TestQual,
  pub target: TestTarget,
  pub compare: TestCompare,
  pub value: Property,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TestTarget {
  #[default]
  Default,
  Pattern,
  Font,
  Scan,
}

parse_enum! {
    TestTarget,
    (Default, "default"),
    (Pattern, "pattern"),
    (Font, "font"),
    (Scan, "scan"),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TestCompare {
  #[default]
  Eq,
  NotEq,
  Less,
  LessEq,
  More,
  MoreEq,
  Contains,
  NotContains,
}

parse_enum! {
    TestCompare,
    (Eq, "eq"),
    (NotEq, "not_eq"),
    (Less, "less"),
    (LessEq, "less_eq"),
    (More, "more"),
    (MoreEq, "more_eq"),
    (Contains, "contains"),
    (NotContains, "not_contains"),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TestQual {
  #[default]
  Any,
  All,
}

parse_enum! {
    TestQual,
    (Any, "any"),
    (All, "all"),
}
