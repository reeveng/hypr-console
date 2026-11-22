use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
  EndOfData,
  ReservedBits,
  MissingPageInformation,
  UnknownPageHeight,
  EmptyPage,
  UnknownSegmentType,
  InvalidReferredCount,
  InvalidReference,
  MissingEndMarker,
  MissingPatternDictionary,
  InvalidHuffmanCode,
  InvalidTableSelection,
  MissingTables,
  UnexpectedOutOfBand,
  ConflictingCodes,
  PrefixTooLong,
  InvalidCombinationOperator,
  InvalidDimension,
  InvalidMmrData,
  GrayScaleOutOfRange,
  InvalidAdaptivePixel,
  NoSymbols,
  TooManySymbols,
  SymbolOutOfRange,
  InvalidSymbol,
  InvalidStripDelta,
  TooManyInstances,
  Overflow,
}

impl fmt::Display for DecodeError {
  fn fmt(&self, to: &mut fmt::Formatter<'_>) -> fmt::Result {
    let said = match self {
      Self::EndOfData => "unexpected end of input",
      Self::ReservedBits => "reserved bits must be zero",
      Self::MissingPageInformation => "missing page information segment",
      Self::UnknownPageHeight => "page height unknown with no stripe segments",
      Self::EmptyPage => "page has zero width or height",
      Self::UnknownSegmentType => "unknown or reserved segment type",
      Self::InvalidReferredCount => "invalid referred-to segment count",
      Self::InvalidReference => "segment refers to larger segment number",
      Self::MissingEndMarker => "missing end marker for unknown-length region",
      Self::MissingPatternDictionary => "missing required pattern dictionary",
      Self::InvalidHuffmanCode => "invalid Huffman code",
      Self::InvalidTableSelection => "invalid Huffman table selection",
      Self::MissingTables => "not enough referred Huffman tables",
      Self::UnexpectedOutOfBand => "unexpected out-of-band value",
      Self::ConflictingCodes => "conflicting Huffman codes",
      Self::PrefixTooLong => "Huffman prefix length too large",
      Self::InvalidCombinationOperator => "invalid combination operator",
      Self::InvalidDimension => "invalid dimension value",
      Self::InvalidMmrData => "invalid MMR-encoded data",
      Self::GrayScaleOutOfRange => "gray-scale value exceeds pattern count",
      Self::InvalidAdaptivePixel => "invalid adaptive template pixel location",
      Self::NoSymbols => "no symbols available",
      Self::TooManySymbols => "symbol dictionary contains too many symbols",
      Self::SymbolOutOfRange => "symbol ID out of range",
      Self::InvalidSymbol => "invalid symbol encountered",
      Self::InvalidStripDelta => "strip delta overflow",
      Self::TooManyInstances => "too many symbol instances",
      Self::Overflow => "a size or a position overflowed",
    };

    write!(to, "{said}")
  }
}

impl std::error::Error for DecodeError {}
