#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Alias {
  pub alias: String,

  pub prefer: Vec<String>,

  pub accept: Vec<String>,

  pub default: Vec<String>,
}
