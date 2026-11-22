//! The categories returned by this crate.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsOneAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CcittSizeOkAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsWhiteSpaceCharacterAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsRegularCharacterAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsEolCharacterAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsNearlyZeroAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsDigitAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsDigitOrMinusAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Emptiness {
  Empty,
  NonEmpty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainsKeyAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsRepairedAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HasOptionalContentGroupsAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeedsDecryptionAnswer {
  Yes,
  No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
  Reached,
  Pending,
}

#[doc = "The grammar used to skip PDF objects."]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectReadMode {
  #[doc = "Document objects may contain indirect references."]
  Document,
  #[doc = "Content-stream operands use the content grammar."]
  ContentStream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObjectStorage {
  Standalone,
  ObjectStream,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParentChainContainsAnswer {
  Yes,
  No,
}
