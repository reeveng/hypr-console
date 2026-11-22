pub(crate) mod lexer;
pub(crate) mod parser;
pub(crate) mod resolver;
mod token;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Group<'a> {
  pub layout: &'a str,
  pub variant: &'a str,
}
