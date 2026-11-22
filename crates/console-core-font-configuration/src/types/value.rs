use crate::{Constant, IntOrRange, PropertyKind};
use std::str::{FromStr, ParseBoolError};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(from = "bool", into = "bool"))]
pub enum Bool {
  True,
  #[default]
  False,
}

impl From<bool> for Bool {
  fn from(value: bool) -> Self {
    match bool::then_some(value, ()) {
      Some(()) => Self::True,
      None => Self::False,
    }
  }
}

#[cfg(feature = "serde")]
impl From<Bool> for bool {
  fn from(value: Bool) -> Self {
    match value {
      Bool::True => true,
      Bool::False => false,
    }
  }
}

impl FromStr for Bool {
  type Err = ParseBoolError;

  fn from_str(text: &str) -> Result<Self, Self::Err> {
    text.parse::<bool>().map(Self::from)
  }
}

pub type Int = u32;
pub type Double = f64;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ListOp {
  Times,
  Divide,
  Or,
  And,
  Plus,
  Minus,
}

parse_enum! {
    ListOp,
    (Times, "times"),
    (Divide, "divide"),
    (Or, "or"),
    (And, "and"),
    (Plus, "plus"),
    (Minus, "minus"),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum UnaryOp {
  Not,

  Cecil,
  Floor,
  Round,
  Trunc,
}

parse_enum! {
    UnaryOp,
    (Not, "not"),
    (Cecil, "cecil"),
    (Floor, "floor"),
    (Round, "round"),
    (Trunc, "trunc"),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BinaryOp {
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
    BinaryOp,
    (Eq, "eq"),
    (NotEq, "not_eq"),
    (Less, "less"),
    (LessEq, "less_eq"),
    (More, "more"),
    (MoreEq, "more_eq"),
    (Contains, "contains"),
    (NotContains, "not_contains"),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TernaryOp {
  If,
}

parse_enum! {
    TernaryOp,
    (If, "if"),
}

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Expression {
  Simple(Value),
  Unary(UnaryOp, Box<Self>),
  Binary(BinaryOp, Box<[Self; 2]>),
  Ternary(TernaryOp, Box<[Self; 3]>),
  List(ListOp, Vec<Self>),
  Matrix(Box<[Self; 4]>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PropertyTarget {
  #[default]
  Default,
  Font,
  Pattern,
}

parse_enum! {
    PropertyTarget,
    (Default, "default"),
    (Font, "font"),
    (Pattern, "pattern"),
}

pub type CharSet = Vec<IntOrRange>;

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Value {
  Int(Int),

  Double(Double),

  String(String),

  Constant(Constant),

  Bool(Bool),

  Range(Int, Int),

  LangSet(String),

  CharSet(CharSet),

  Property(PropertyTarget, PropertyKind),
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the listed wrapper types use identical From implementations that forward directly to their corresponding Value variants"
  )
)]
macro_rules! from_value {
	($($name:ident,)+) => {
        $(
            impl From<$name> for Value {
                fn from(v: $name) -> Value {
                    Value::$name(v)
                }
            }
        )+
	};
}

from_value! {
    Int,
    Bool,
    Double,
    Constant,
    CharSet,
}

impl From<bool> for Value {
  fn from(value: bool) -> Self {
    Self::Bool(Bool::from(value))
  }
}

impl<'a> From<&'a str> for Value {
  fn from(s: &'a str) -> Self {
    Value::String(String::from(s))
  }
}

impl From<String> for Value {
  fn from(s: String) -> Self {
    Value::String(s)
  }
}

impl From<(PropertyTarget, PropertyKind)> for Value {
  fn from((target, kind): (PropertyTarget, PropertyKind)) -> Self {
    Value::Property(target, kind)
  }
}

impl<V> From<V> for Expression
where
  Value: From<V>,
{
  fn from(v: V) -> Self {
    Expression::Simple(Value::from(v))
  }
}
