use crate::{Int, IntOrRange};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Configuration {
  pub blanks: Vec<IntOrRange>,
  pub rescans: Vec<Int>,
}
