use console_core_never::Never;
mod handlers;

use crate::xkb::code_map::CodeMap;
use crate::xkb::span::Span;
use crate::xkb::span::SpanExt;
use crate::xkb::span::SpanUnit;
use crate::xkb::span::Spanned;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
use console_input_keymaps_derive::diagnostic_kind;
use std::error::Error;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::ops::ControlFlow;
use std::path::PathBuf;

pub trait DiagnosticHandler {
  fn filter(&self, _kind: DiagnosticKind, _fatality: Fatality) -> Result<Relevance, Never> {
    Ok(Relevance::Relevant)
  }

  fn handle(&mut self, diag: Diagnostic);
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Relevance {
  Relevant,
  Irrelevant,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Fatality {
  Recoverable,
  Fatal,
}

pub(crate) struct DiagnosticSink<'a, 'b> {
  handler: &'a mut (dyn DiagnosticHandler + 'b),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
#[non_exhaustive]
pub enum Severity {
  Debug,
  Warning,
  Error,
}

#[diagnostic_kind]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum DiagnosticKind {
  #[severity = Error]
  OctalStringEscapeOverflow,
  #[severity = Error]
  UnknownEscapeSequence,
  #[severity = Error]
  FileOpenFailed,
  #[severity = Error]
  DeserializeRegistryFailed,
  #[severity = Error]
  FileReadFailed,
  #[severity = Warning]
  FileNotFound,
  #[severity = Warning]
  UnexpectedItemType,
  #[severity = Warning]
  MultipleDefaultItems,
  #[severity = Warning]
  DuplicateItemName,
  #[severity = Error]
  MissingIncludeFileName,
  #[severity = Error]
  UnterminatedIncludeMapName,
  #[severity = Error]
  MissingIncludeMergeMode,
  #[severity = Error]
  InvalidIncludeGroupIndex,
  #[severity = Error]
  UnterminatedKeyName,
  #[severity = Error]
  UnterminatedString,
  #[severity = Error]
  InvalidFloatLiteral,
  #[severity = Error]
  InvalidIntegerLiteral,
  #[severity = Error]
  UnlexableByte,
  #[severity = Error]
  MaxIncludeDepthReached,
  #[severity = Error]
  MaxIncludesReached,
  #[severity = Error]
  EmptyMacroName,
  #[severity = Error]
  UnexpectedEof,
  #[severity = Error]
  UnexpectedToken,
  #[severity = Error]
  ExpectedEol,
  #[severity = Error]
  ExpectedIndexStart,
  #[severity = Error]
  ExpectedIndexEnd,
  #[severity = Error]
  InvalidMatcherIndex,
  #[severity = Error]
  TooDeeplyNested,
  #[severity = Error]
  UnexpectedDeclaration,
  #[severity = Error]
  U32Overflow,
  #[severity = Error]
  UnknownKeysym,
  #[severity = Error]
  UnknownGroup,
  #[severity = Error]
  UnknownLevel,
  #[severity = Error]
  UnknownRadioGroup,
  #[severity = Error]
  RecursiveInclude,
  #[severity = Error]
  InvalidNumberOfRuleKeys,
  #[severity = Error]
  UnexpectedRule,
  #[severity = Error]
  InvalidNumberOfRuleValues,
  #[severity = Error]
  InvalidPercentEncoding,
  #[severity = Error]
  InvalidIndicatorIndex,
  #[severity = Error]
  TooManyIndicators,
  #[severity = Error]
  UnknownKeycodesVariable,
  #[severity = Error]
  UnknownTypesVariable,
  #[severity = Error]
  UnknownCompatVariable,
  #[severity = Error]
  UnknownKeyAlias,
  #[severity = Warning]
  DuplicateKeyTypeDefinition,
  #[severity = Error]
  UnknownAction,
  #[severity = Warning]
  IgnoredInterpretField,
  #[severity = Warning]
  IgnoredIndicatorField,
  #[severity = Error]
  UnknownModifier,
  #[severity = Warning]
  IgnoredModMapEntry,
  #[severity = Error]
  UnknownKey,
  #[severity = Error]
  UnknownSymbolsVariable,
  #[severity = Warning]
  IgnoredKeyField,
  #[severity = Error]
  DiscardingGroup,
  #[severity = Warning]
  MissingGroupName,
  #[severity = Error]
  TooManyVirtualModifiers,
  #[severity = Warning]
  IgnoringVmodRedefinition,
  #[severity = Error]
  HomeNotSet,
  #[severity = Error]
  UnknownRulesEscapeSequence,
  #[severity = Warning]
  UsingFirstInsteadOfDefault,
  #[severity = Error]
  ModsCalculationOverflow,
  #[severity = Error]
  KeysymCalculationOverflow,
  #[severity = Error]
  LevelCalculationOverflow,
  #[severity = Error]
  RadioGroupCalculationOverflow,
  #[severity = Error]
  UnsupportedExpressionForGroup,
  #[severity = Error]
  UnsupportedExpressionForGroupChange,
  #[severity = Error]
  GroupCalculationOverflow,
  #[severity = Error]
  UnsupportedExpressionForLevel,
  #[severity = Error]
  UnsupportedExpressionForRadioGroup,
  #[severity = Error]
  UnsupportedExpressionForString,
  #[severity = Error]
  UnsupportedExpressionForBoolean,
  #[severity = Error]
  UnsupportedExpressionForKeysym,
  #[severity = Error]
  UnsupportedExpressionForModMask,
  #[severity = Error]
  UnsupportedExpressionForKeyRepeats,
  #[severity = Error]
  UnsupportedExpressionForLockModsAffect,
  #[severity = Error]
  UnsupportedExpressionForAction,
  #[severity = Error]
  UnsupportedExpressionForSymbolsOrActions,
  #[severity = Error]
  UnsupportedExpressionForKeycode,
  #[severity = Error]
  UnknownBooleanValue,
  #[severity = Debug]
  UnimplementedAction,
  #[severity = Error]
  UnknownParameterForNoAction,
  #[severity = Error]
  UnknownParameterForVoidAction,
  #[severity = Error]
  UnknownParameterForSetMods,
  #[severity = Error]
  UnknownParameterForLatchMods,
  #[severity = Error]
  UnknownParameterForLockMods,
  #[severity = Error]
  UnknownParameterForSetGroup,
  #[severity = Error]
  UnknownParameterForLatchGroup,
  #[severity = Error]
  UnknownParameterForLockGroup,
  #[severity = Error]
  MissingValueForSetModsMods,
  #[severity = Error]
  MissingValueForLatchModsMods,
  #[severity = Error]
  MissingValueForLockModsMods,
  #[severity = Error]
  MissingValueForLockModsAffect,
  #[severity = Error]
  MissingValueForSetGroupGroup,
  #[severity = Error]
  MissingValueForLatchGroupGroup,
  #[severity = Error]
  MissingValueForLockGroupGroup,
  #[severity = Error]
  MissingValueForRedirectKeyKey,
  #[severity = Error]
  MissingValueForRedirectKeyClearmods,
  #[severity = Error]
  MissingValueForRedirectKeyMods,
  #[severity = Error]
  MissingValueForSetControlsControls,
  #[severity = Error]
  MissingValueForLockControlsControls,
  #[severity = Error]
  MissingValueForLockControlsAffect,
  #[severity = Error]
  UnknownLockModsAffect,
  #[severity = Error]
  NotOneInterpretFilterArgument,
  #[severity = Error]
  NamedFilterArgArgument,
  #[severity = Error]
  UnknownFilterPredicate,
  #[severity = Error]
  UnknownInterpretField,
  #[severity = Debug]
  UnimplementedInterpretField,
  #[severity = Error]
  MissingInterpretActionValue,
  #[severity = Error]
  MissingInterpretVirtualmodValue,
  #[severity = Error]
  UnknownInterpretVirtualModifier,
  #[severity = Error]
  UnknownTypeField,
  #[severity = Error]
  MissingTypeModifiersValue,
  #[severity = Error]
  MissingTypeMapValue,
  #[severity = Error]
  MissingTypePreserveValue,
  #[severity = Error]
  MissingTypeLevelNameValue,
  #[severity = Error]
  MissingInterpretUseModMapModValue,
  #[severity = Error]
  InvalidInterpretUseModMapModValue,
  #[severity = Debug]
  UnimplementedIndicatorField,
  #[severity = Error]
  UnknownIndicatorField,
  #[severity = Error]
  MissingIndicatorModifiersValue,
  #[severity = Error]
  GroupOutOfBounds,
  #[severity = Error]
  LevelOutOfBounds,
  #[severity = Error]
  RadioGroupOutOfBounds,
  #[severity = Error]
  MissingIndicatorGroupsValue,
  #[severity = Error]
  UnknownControl,
  #[severity = Error]
  MissingIndicatorControlValue,
  #[severity = Error]
  UnknownModComponent,
  #[severity = Error]
  UnsupportedExpressionForModComponent,
  #[severity = Error]
  UnsupportedExpressionForControl,
  #[severity = Error]
  MissingIndicatorWhichModStateValue,
  #[severity = Error]
  UnknownGroupComponent,
  #[severity = Error]
  UnsupportedExpressionForGroupComponent,
  #[severity = Error]
  MissingIndicatorWhichGroupStateValue,
  #[severity = Error]
  UnknownKeycode,
  #[severity = Error]
  UnknownKeyField,
  #[severity = Error]
  MissingKeyGroupsRedirectValue,
  #[severity = Error]
  MissingKeySymbolsValue,
  #[severity = Error]
  MissingKeyActionsValue,
  #[severity = Error]
  MissingKeyVirtualModifiersValue,
  #[severity = Error]
  UnknownKeyRepeatingValue,
  #[severity = Error]
  MissingKeyTypeValue,
  #[severity = Error]
  UnknownKeyType,
  #[severity = Error]
  VirtualModifierHasRealName,
  #[severity = Error]
  UnterminatedKeysym,
  #[severity = Error]
  UnknownComposeIncludeEscape,
  #[severity = Error]
  LocaleComposeFileNotResolved,
  #[severity = Debug]
  IgnoringDuplicateComposeEntry,
  #[severity = Debug]
  IgnoringComposePrefix,
  #[severity = Debug]
  ComposeProductionOverwritten,
  #[severity = Error]
  NonUTF8Path,
  #[severity = Error]
  ComposeRuleWithoutConditions,
  #[severity = Error]
  MaxRuntimeReached,
  #[severity = Error]
  MaxComposeRulesReached,
  #[severity = Error]
  UnknownParameterForRedirectKey,
  #[severity = Error]
  UnknownParameterForSetControls,
  #[severity = Error]
  UnknownParameterForLockControls,
  #[severity = Error]
  MissingKeyOverlayValue,
  #[severity = Error]
  MissingKeyRadiogroupValue,
  #[severity = Error]
  UnterminatedUnicodeEscape,
  #[severity = Error]
  UnopenedUnicodeEscape,
  #[severity = Error]
  InvalidUnicodeEscapeRepresentation,
  #[severity = Error]
  InvalidUnicodeCodepoint,
  #[severity = Error]
  KeysymStringNotUtf8,
  #[severity = Error]
  MultipleKeysymsInInterpret,
}

impl DiagnosticKind {
  pub fn severity(&self) -> Result<Severity, Never> {
    Ok(self.severity_())
  }
}

pub struct Diagnostic {
  kind: DiagnosticKind,
  location: DiagnosticLocation,
}

pub(crate) struct DiagnosticLocation {
  message: Option<Box<dyn Display + Send + Sync>>,
  source_file: Option<PathBuf>,
  line_num: u32,
  in_line_offset: u32,
  inner: Option<Box<DiagnosticLocation>>,
}

impl<'a, 'b> DiagnosticSink<'a, 'b> {
  pub(crate) fn new(handler: &'a mut (dyn DiagnosticHandler + 'b)) -> Result<Self, Never> {
    Ok(Self { handler })
  }

  fn push_(
    &mut self,
    map: &mut CodeMap,
    kind: DiagnosticKind,
    message: Spanned<impl Display + Send + Sync + 'static>,
    fatality: Fatality,
  ) -> Result<(), Never> {
    let Ok(relevance) = self.handler.filter(kind, fatality);

    match relevance {
      Relevance::Relevant => {}
      Relevance::Irrelevant => {
        return Ok(());
      }
    }

    let Ok(diagnostic) = Diagnostic::new(map, kind, message.val, message.span);
    self.handler.handle(diagnostic);

    Ok(())
  }

  pub(crate) fn push(
    &mut self,
    map: &mut CodeMap,
    kind: DiagnosticKind,
    message: Spanned<impl Display + Send + Sync + 'static>,
  ) -> Result<(), Never> {
    let Ok(()) = self.push_(map, kind, message, Fatality::Recoverable);

    Ok(())
  }

  pub(crate) fn push_fatal(
    &mut self,
    map: &mut CodeMap,
    kind: DiagnosticKind,
    message: Spanned<impl Display + Send + Sync + 'static>,
  ) -> Result<Diagnostic, Never> {
    let said = message.val.to_string();
    let Ok(()) = self.push_(
      map,
      kind,
      said.clone().spanned2(message.span),
      Fatality::Fatal,
    );
    Diagnostic::new(map, kind, said, message.span)
  }
}

impl DiagnosticLocation {
  fn new(
    map: &CodeMap,
    message: Option<Box<dyn Display + Send + Sync + 'static>>,
    span: Span,
  ) -> Result<Self, Never> {
    let Ok(innermost) = Self::at(map, message, span, None);
    let Ok(codes) = map.codes();
    let walked = (0..codes).try_fold(innermost, |(location, include), _outward| match include {
      Some(include_span) => {
        let Ok(outer) = Self::at(map, None, include_span, Some(Box::new(location)));

        ControlFlow::Continue(outer)
      }
      None => ControlFlow::Break(location),
    });

    Ok(match walked {
      ControlFlow::Break(location) | ControlFlow::Continue((location, _)) => location,
    })
  }

  fn at(
    map: &CodeMap,
    message: Option<Box<dyn Display + Send + Sync + 'static>>,
    span: Span,
    inner: Option<Box<DiagnosticLocation>>,
  ) -> Result<(Self, Option<Span>), Never> {
    let Ok(found) = map.get(span);
    let info = match found {
      Some(info) => info,
      None => {
        let unplaced = Self {
          message,
          source_file: None,
          line_num: 0,
          in_line_offset: 0,
          inner,
        };
        return Ok((unplaced, None));
      }
    };
    let lo = span.lo.max(info.span.lo).saturating_sub(info.lines_offset);
    let Ok(line_num) = fitted::<_, u32>(info.lines.partition_point(|start| *start <= lo));
    let Ok(lines_before) = index(line_num);
    let in_line_offset = match info.lines.get(..lines_before).and_then(<[SpanUnit]>::last) {
      Some(line_lo) => lo.saturating_sub(*line_lo),
      None => 0,
    };
    let location = Self {
      message,
      source_file: info.file.cloned(),
      line_num,
      in_line_offset,
      inner,
    };
    Ok((location, info.include_span))
  }

  fn write_location(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "at {} {}:{}: {}",
      std::fmt::from_fn(|out| match &self.source_file {
        Some(p) => Display::fmt(&p.display(), out),
        None => out.write_str("<anonymous file>"),
      }),
      self.line_num,
      self.in_line_offset,
      std::fmt::from_fn(|out| match &self.message {
        Some(m) => m.fmt(out),
        None => out.write_str("while processing include"),
      }),
    )
  }
}

impl Diagnostic {
  pub(crate) fn new(
    map: &CodeMap,
    kind: DiagnosticKind,
    message: impl Display + Send + Sync + 'static,
    span: Span,
  ) -> Result<Self, Never> {
    let Ok(location) = DiagnosticLocation::new(map, Some(Box::new(message)), span);
    Ok(Self { kind, location })
  }

  pub fn kind(&self) -> Result<DiagnosticKind, Never> {
    Ok(self.kind)
  }
}

impl Debug for Diagnostic {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    self.location.write_location(f)
  }
}

impl Display for Diagnostic {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    self.location.write_location(f)
  }
}

impl Error for Diagnostic {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    self.location.source()
  }
}

impl Debug for DiagnosticLocation {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    self.write_location(f)
  }
}

impl Display for DiagnosticLocation {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    self.write_location(f)
  }
}

impl Error for DiagnosticLocation {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    match self.inner.as_deref() {
      Some(inner) => {
        let source: &(dyn Error + 'static) = inner;

        Some(source)
      }
      None => None,
    }
  }
}
