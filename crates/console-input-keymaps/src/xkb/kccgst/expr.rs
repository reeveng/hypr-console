use crate::Keycode;
use crate::Keysym;
use crate::ModifierIndex;
use crate::ModifierMask;
use crate::syms;
use crate::xkb::code_map::CodeMap;
use crate::xkb::controls::ControlMask;
use crate::xkb::diagnostic::DiagnosticKind;
use crate::xkb::diagnostic::DiagnosticSink;
use crate::xkb::group::GroupChange;
use crate::xkb::group::GroupIdx;
use crate::xkb::group::GroupMask;
use crate::xkb::group_component::GroupComponent;
use crate::xkb::interner::Interned;
use crate::xkb::interner::Interner;
use crate::xkb::kccgst::ast::Call;
use crate::xkb::kccgst::ast::CallArg;
use crate::xkb::kccgst::ast::Expr;
use crate::xkb::kccgst::ast::Path;
use crate::xkb::kccgst::ast::PathComponentRef;
use crate::xkb::kccgst::ast::Var;
use crate::xkb::keymap::KeyOverlay;
use crate::xkb::keymap::actions::Affect;
use crate::xkb::level::Level;
use crate::xkb::meaning::Keywords;
use crate::xkb::meaning::Meaning;
use crate::xkb::meaning::MeaningCache;
use crate::xkb::mod_component::ModComponentMask;
use crate::xkb::modmap::Combine;
use crate::xkb::modmap::ModifierTree;
use crate::xkb::modmap::Vmodmap;
use crate::xkb::radio_group::RadioGroup;
use crate::xkb::resolved::ActionDefaults;
use crate::xkb::resolved::BuiltInKeytype;
use crate::xkb::resolved::Filter;
use crate::xkb::resolved::KeyTypeRef;
use crate::xkb::resolved::ModMapField;
use crate::xkb::resolved::Predicate;
use crate::xkb::resolved::ResolvedAction;
use crate::xkb::resolved::ResolvedActionMods;
use crate::xkb::resolved::ResolvedGroupLatch;
use crate::xkb::resolved::ResolvedGroupLock;
use crate::xkb::resolved::ResolvedGroupSet;
use crate::xkb::resolved::ResolvedKeyKind;
use crate::xkb::resolved::ResolvedKeycodes;
use crate::xkb::resolved::ResolvedLockControls;
use crate::xkb::resolved::ResolvedModsLatch;
use crate::xkb::resolved::ResolvedModsLock;
use crate::xkb::resolved::ResolvedModsSet;
use crate::xkb::resolved::ResolvedNoAction;
use crate::xkb::resolved::ResolvedRedirectKey;
use crate::xkb::resolved::ResolvedSetControls;
use crate::xkb::resolved::ResolvedTypes;
use crate::xkb::resolved::ResolvedVoidAction;
use crate::xkb::span::Span;
use crate::xkb::span::SpanExt;
use crate::xkb::span::SpanResult1;
use crate::xkb::span::SpanResult2;
use crate::xkb::span::Spanned;
use crate::xkb::string_cooker::StringCooker;
use EvalError::*;
use console_core_never::Never;
use console_core_number_conversion::FromBytes;
use console_core_words::Words;
use std::ops::BitAnd;
use std::ops::BitOr;
use std::ops::Not;

#[derive(Copy, Clone, Debug, Words)]
pub(crate) enum EvalError {
  #[words(message = "modifier calculation overflowed")]
  ModsCalculationOverflow,
  #[words(message = "keysym calculation overflowed")]
  KeysymCalculationOverflow,
  #[words(message = "level calculation overflowed")]
  LevelCalculationOverflow,
  #[words(message = "radio group calculation overflowed")]
  RadioGroupCalculationOverflow,
  #[words(message = "unknown group")]
  UnknownGroup,
  #[words(message = "unsupported expression for group")]
  UnsupportedExpressionForGroup,
  #[words(message = "unsupported expression for group change")]
  UnsupportedExpressionForGroupChange,
  #[words(message = "group calculation overflowed")]
  GroupCalculationOverflow,
  #[words(message = "unknown level")]
  UnknownLevel,
  #[words(message = "unknown radio group")]
  UnknownRadioGroup,
  #[words(message = "unsupported expression for level")]
  UnsupportedExpressionForLevel,
  #[words(message = "unsupported expression for radio group")]
  UnsupportedExpressionForRadioGroup,
  #[words(message = "unsupported expression for string")]
  UnsupportedExpressionForString,
  #[words(message = "unsupported expression for boolean")]
  UnsupportedExpressionForBoolean,
  #[words(message = "unsupported expression for keysym")]
  UnsupportedExpressionForKeysym,
  #[words(message = "unsupported expression for keycode")]
  UnsupportedExpressionForKeycode,
  #[words(message = "unsupported expression for mod mask")]
  UnsupportedExpressionForModMask,
  #[words(message = "unsupported expression for key repeat")]
  UnsupportedExpressionForKeyRepeats,
  #[words(message = "unsupported expression for LockMods(affect)")]
  UnsupportedExpressionForLockModsAffect,
  #[words(message = "unsupported expression for action")]
  UnsupportedExpressionForAction,
  #[words(message = "unsupported expression for symbols/actions")]
  UnsupportedExpressionForSymbolsOrActions,
  #[words(message = "unknown boolean value")]
  UnknownBooleanValue,
  #[words(message = "unknown keysym")]
  UnknownKeysym,
  #[words(message = "unknown action")]
  UnknownAction,
  #[words(message = "unimplemented action")]
  UnimplementedAction,
  #[words(message = "unknown NoAction parameter")]
  UnknownParameterForNoAction,
  #[words(message = "unknown VoidAction parameter")]
  UnknownParameterForVoidAction,
  #[words(message = "unknown SetMods parameter")]
  UnknownParameterForSetMods,
  #[words(message = "unknown LatchMods parameter")]
  UnknownParameterForLatchMods,
  #[words(message = "unknown LockMods parameter")]
  UnknownParameterForLockMods,
  #[words(message = "unknown SetGroup parameter")]
  UnknownParameterForSetGroup,
  #[words(message = "unknown LatchGroup parameter")]
  UnknownParameterForLatchGroup,
  #[words(message = "unknown LockGroup parameter")]
  UnknownParameterForLockGroup,
  #[words(message = "missing value for SetMods(mods)")]
  MissingValueForSetModsMods,
  #[words(message = "missing value for LatchMods(mods)")]
  MissingValueForLatchModsMods,
  #[words(message = "missing value for LockMods(mods)")]
  MissingValueForLockModsMods,
  #[words(message = "missing value for LockMods(affect)")]
  MissingValueForLockModsAffect,
  #[words(message = "missing value for SetGroup(group)")]
  MissingValueForSetGroupGroup,
  #[words(message = "missing value for LatchGroup(group)")]
  MissingValueForLatchGroupGroup,
  #[words(message = "missing value for LockGroup(group)")]
  MissingValueForLockGroupGroup,
  #[words(message = "missing value for RedirectKey(key)")]
  MissingValueForRedirectKeyKey,
  #[words(message = "missing value for RedirectKey(clearMods)")]
  MissingValueForRedirectKeyClearmods,
  #[words(message = "missing value for RedirectKey(mods)")]
  MissingValueForRedirectKeyMods,
  #[words(message = "missing value for SetControls(controls)")]
  MissingValueForSetControlsControls,
  #[words(message = "missing value for LockControls(controls)")]
  MissingValueForLockControlsControls,
  #[words(message = "missing value for LockControls(affect)")]
  MissingValueForLockControlsAffect,
  #[words(message = "unknown modifier")]
  UnknownModifier,
  #[words(message = "unknown LockMods(affect) value")]
  UnknownLockModsAffect,
  #[words(message = "interpret filter must have exactly one argument")]
  NotOneInterpretFilterArgument,
  #[words(message = "interpret filter argument must not have a name")]
  NamedFilterArgArgument,
  #[words(message = "unknown interpret filter predicate")]
  UnknownFilterPredicate,
  #[words(message = "unknown `interpret` field")]
  UnknownInterpretField,
  #[words(message = "interpret `action` argument must have a value")]
  MissingInterpretActionValue,
  #[words(message = "interpret `virtualmodifier` argument must have a value")]
  MissingInterpretVirtualmodValue,
  #[words(message = "unknown virtual modifier")]
  UnknownInterpretVirtualModifier,
  #[words(message = "unknown `type` field")]
  UnknownTypeField,
  #[words(message = "type `modifiers` argument must have a value")]
  MissingTypeModifiersValue,
  #[words(message = "type `map` argument must have a value")]
  MissingTypeMapValue,
  #[words(message = "type `preserve` argument must have a value")]
  MissingTypePreserveValue,
  #[words(message = "type `level_name` argument must have a value")]
  MissingTypeLevelNameValue,
  #[words(message = "interpret `useModMap` argument must have a value")]
  MissingInterpretUseModMapModValue,
  #[words(message = "invalid interpret `useModMap` argument")]
  InvalidInterpretUseModMapModValue,
  #[words(message = "unknown `indicator` field")]
  UnknownIndicatorField,
  #[words(message = "unimplemented `indicator` field")]
  UnimplementedIndicatorField,
  #[words(message = "indicator `modifiers` argument must have a value")]
  MissingIndicatorModifiersValue,
  #[words(message = "out of bounds group value")]
  GroupOutOfBounds,
  #[words(message = "out of bounds level value")]
  LevelOutOfBounds,
  #[words(message = "out of bounds radio group value")]
  RadioGroupOutOfBounds,
  #[words(message = "indicator `groups` argument must have a value")]
  MissingIndicatorGroupsValue,
  #[words(message = "unknown control")]
  UnknownControl,
  #[words(message = "indicator `control` argument must have a value")]
  MissingIndicatorControlValue,
  #[words(message = "unknown mod component")]
  UnknownModComponent,
  #[words(message = "unsupported expression for mod component")]
  UnsupportedExpressionForModComponent,
  #[words(message = "unsupported expression for control")]
  UnsupportedExpressionForControl,
  #[words(message = "indicator `whichModState` argument must have a value")]
  MissingIndicatorWhichModStateValue,
  #[words(message = "unknown group component")]
  UnknownGroupComponent,
  #[words(message = "unsupported expression for group component")]
  UnsupportedExpressionForGroupComponent,
  #[words(message = "indicator `whichGroupState` argument must have a value")]
  MissingIndicatorWhichGroupStateValue,
  #[words(message = "unknown keycode")]
  UnknownKeycode,
  #[words(message = "unknown `key` field")]
  UnknownKeyField,
  #[words(message = "key `groupsRedirect` argument must have a value")]
  MissingKeyGroupsRedirectValue,
  #[words(message = "key `symbols` argument must have a value")]
  MissingKeySymbolsValue,
  #[words(message = "key `actions` argument must have a value")]
  MissingKeyActionsValue,
  #[words(message = "key `overlay1`/`overlay2` argument must have a value")]
  MissingKeyOverlayValue,
  #[words(message = "key `radiogroup` argument must have a value")]
  MissingKeyRadiogroupValue,
  #[words(message = "key `virtual_modifiers` argument must have a value")]
  MissingKeyVirtualModifiersValue,
  #[words(message = "unknown key `repeating` value")]
  UnknownKeyRepeatingValue,
  #[words(message = "key `type` argument must have a value")]
  MissingKeyTypeValue,
  #[words(message = "unknown key type")]
  UnknownKeyType,
  #[words(message = "unknown RedirectKey parameter")]
  UnknownParameterForRedirectKey,
  #[words(message = "unknown SetControls parameter")]
  UnknownParameterForSetControls,
  #[words(message = "unknown LockControls parameter")]
  UnknownParameterForLockControls,
  #[words(message = "keysym string does not contain valid UTF-8")]
  KeysymStringNotUtf8,
  #[words(message = "interpret declaration contains multiple keysyms")]
  MultipleKeysymsInInterpret,
}

impl std::fmt::Display for EvalError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let Ok(message) = self.message();

    f.write_str(message)
  }
}

impl std::error::Error for EvalError {}

impl EvalError {
  pub(crate) fn diagnostic_kind(&self) -> Result<DiagnosticKind, Never> {
    Ok(match self {
      ModsCalculationOverflow => DiagnosticKind::ModsCalculationOverflow,
      KeysymCalculationOverflow => DiagnosticKind::KeysymCalculationOverflow,
      LevelCalculationOverflow => DiagnosticKind::LevelCalculationOverflow,
      RadioGroupCalculationOverflow => DiagnosticKind::RadioGroupCalculationOverflow,
      UnknownGroup => DiagnosticKind::UnknownGroup,
      UnsupportedExpressionForGroup => DiagnosticKind::UnsupportedExpressionForGroup,
      UnsupportedExpressionForGroupChange => DiagnosticKind::UnsupportedExpressionForGroupChange,
      GroupCalculationOverflow => DiagnosticKind::GroupCalculationOverflow,
      UnknownLevel => DiagnosticKind::UnknownLevel,
      UnknownRadioGroup => DiagnosticKind::UnknownRadioGroup,
      UnsupportedExpressionForLevel => DiagnosticKind::UnsupportedExpressionForLevel,
      UnsupportedExpressionForRadioGroup => DiagnosticKind::UnsupportedExpressionForRadioGroup,
      UnsupportedExpressionForString => DiagnosticKind::UnsupportedExpressionForString,
      UnsupportedExpressionForBoolean => DiagnosticKind::UnsupportedExpressionForBoolean,
      UnsupportedExpressionForKeysym => DiagnosticKind::UnsupportedExpressionForKeysym,
      UnsupportedExpressionForModMask => DiagnosticKind::UnsupportedExpressionForModMask,
      UnsupportedExpressionForKeyRepeats => DiagnosticKind::UnsupportedExpressionForKeyRepeats,
      UnsupportedExpressionForLockModsAffect => {
        DiagnosticKind::UnsupportedExpressionForLockModsAffect
      }
      UnsupportedExpressionForAction => DiagnosticKind::UnsupportedExpressionForAction,
      UnsupportedExpressionForSymbolsOrActions => {
        DiagnosticKind::UnsupportedExpressionForSymbolsOrActions
      }
      UnsupportedExpressionForKeycode => DiagnosticKind::UnsupportedExpressionForKeycode,
      UnknownBooleanValue => DiagnosticKind::UnknownBooleanValue,
      UnknownKeysym => DiagnosticKind::UnknownKeysym,
      UnknownAction => DiagnosticKind::UnknownAction,
      UnimplementedAction => DiagnosticKind::UnimplementedAction,
      UnknownParameterForNoAction => DiagnosticKind::UnknownParameterForNoAction,
      UnknownParameterForVoidAction => DiagnosticKind::UnknownParameterForVoidAction,
      UnknownParameterForSetMods => DiagnosticKind::UnknownParameterForSetMods,
      UnknownParameterForLatchMods => DiagnosticKind::UnknownParameterForLatchMods,
      UnknownParameterForLockMods => DiagnosticKind::UnknownParameterForLockMods,
      UnknownParameterForSetGroup => DiagnosticKind::UnknownParameterForSetGroup,
      UnknownParameterForLatchGroup => DiagnosticKind::UnknownParameterForLatchGroup,
      UnknownParameterForLockGroup => DiagnosticKind::UnknownParameterForLockGroup,
      UnknownParameterForRedirectKey => DiagnosticKind::UnknownParameterForRedirectKey,
      UnknownParameterForSetControls => DiagnosticKind::UnknownParameterForSetControls,
      UnknownParameterForLockControls => DiagnosticKind::UnknownParameterForLockControls,
      MissingValueForSetModsMods => DiagnosticKind::MissingValueForSetModsMods,
      MissingValueForLatchModsMods => DiagnosticKind::MissingValueForLatchModsMods,
      MissingValueForLockModsMods => DiagnosticKind::MissingValueForLockModsMods,
      MissingValueForLockModsAffect => DiagnosticKind::MissingValueForLockModsAffect,
      MissingValueForSetGroupGroup => DiagnosticKind::MissingValueForSetGroupGroup,
      MissingValueForLatchGroupGroup => DiagnosticKind::MissingValueForLatchGroupGroup,
      MissingValueForLockGroupGroup => DiagnosticKind::MissingValueForLockGroupGroup,
      MissingValueForRedirectKeyKey => DiagnosticKind::MissingValueForRedirectKeyKey,
      MissingValueForRedirectKeyClearmods => DiagnosticKind::MissingValueForRedirectKeyClearmods,
      MissingValueForRedirectKeyMods => DiagnosticKind::MissingValueForRedirectKeyMods,
      MissingValueForSetControlsControls => DiagnosticKind::MissingValueForSetControlsControls,
      MissingValueForLockControlsControls => DiagnosticKind::MissingValueForLockControlsControls,
      MissingValueForLockControlsAffect => DiagnosticKind::MissingValueForLockControlsAffect,
      UnknownModifier => DiagnosticKind::UnknownModifier,
      UnknownLockModsAffect => DiagnosticKind::UnknownLockModsAffect,
      NotOneInterpretFilterArgument => DiagnosticKind::NotOneInterpretFilterArgument,
      NamedFilterArgArgument => DiagnosticKind::NamedFilterArgArgument,
      UnknownFilterPredicate => DiagnosticKind::UnknownFilterPredicate,
      UnknownInterpretField => DiagnosticKind::UnknownInterpretField,
      MissingInterpretActionValue => DiagnosticKind::MissingInterpretActionValue,
      MissingInterpretVirtualmodValue => DiagnosticKind::MissingInterpretVirtualmodValue,
      UnknownInterpretVirtualModifier => DiagnosticKind::UnknownInterpretVirtualModifier,
      UnknownTypeField => DiagnosticKind::UnknownTypeField,
      MissingTypeModifiersValue => DiagnosticKind::MissingTypeModifiersValue,
      MissingTypeMapValue => DiagnosticKind::MissingTypeMapValue,
      MissingTypePreserveValue => DiagnosticKind::MissingTypePreserveValue,
      MissingTypeLevelNameValue => DiagnosticKind::MissingTypeLevelNameValue,
      MissingInterpretUseModMapModValue => DiagnosticKind::MissingInterpretUseModMapModValue,
      InvalidInterpretUseModMapModValue => DiagnosticKind::InvalidInterpretUseModMapModValue,
      UnknownIndicatorField => DiagnosticKind::UnknownIndicatorField,
      UnimplementedIndicatorField => DiagnosticKind::UnimplementedIndicatorField,
      MissingIndicatorModifiersValue => DiagnosticKind::MissingIndicatorModifiersValue,
      GroupOutOfBounds => DiagnosticKind::GroupOutOfBounds,
      LevelOutOfBounds => DiagnosticKind::LevelOutOfBounds,
      RadioGroupOutOfBounds => DiagnosticKind::RadioGroupOutOfBounds,
      MissingIndicatorGroupsValue => DiagnosticKind::MissingIndicatorGroupsValue,
      UnknownControl => DiagnosticKind::UnknownControl,
      MissingIndicatorControlValue => DiagnosticKind::MissingIndicatorControlValue,
      UnknownModComponent => DiagnosticKind::UnknownModComponent,
      UnsupportedExpressionForModComponent => {
        DiagnosticKind::UnsupportedExpressionForModComponent
      }
      UnsupportedExpressionForControl => DiagnosticKind::UnsupportedExpressionForControl,
      MissingIndicatorWhichModStateValue => DiagnosticKind::MissingIndicatorWhichModStateValue,
      UnknownGroupComponent => DiagnosticKind::UnknownGroupComponent,
      UnsupportedExpressionForGroupComponent => {
        DiagnosticKind::UnsupportedExpressionForGroupComponent
      }
      MissingIndicatorWhichGroupStateValue => {
        DiagnosticKind::MissingIndicatorWhichGroupStateValue
      }
      UnknownKeycode => DiagnosticKind::UnknownKeycode,
      UnknownKeyField => DiagnosticKind::UnknownKeyField,
      MissingKeyGroupsRedirectValue => DiagnosticKind::MissingKeyGroupsRedirectValue,
      MissingKeySymbolsValue => DiagnosticKind::MissingKeySymbolsValue,
      MissingKeyActionsValue => DiagnosticKind::MissingKeyActionsValue,
      MissingKeyVirtualModifiersValue => DiagnosticKind::MissingKeyVirtualModifiersValue,
      UnknownKeyRepeatingValue => DiagnosticKind::UnknownKeyRepeatingValue,
      MissingKeyTypeValue => DiagnosticKind::MissingKeyTypeValue,
      UnknownKeyType => DiagnosticKind::UnknownKeyType,
      MissingKeyOverlayValue => DiagnosticKind::MissingKeyOverlayValue,
      MissingKeyRadiogroupValue => DiagnosticKind::MissingKeyRadiogroupValue,
      KeysymStringNotUtf8 => DiagnosticKind::KeysymStringNotUtf8,
      MultipleKeysymsInInterpret => DiagnosticKind::MultipleKeysymsInInterpret,
    })
  }
}

#[derive(Copy, Clone)]
struct IntegerFaults {
  unknown: EvalError,
  unsupported: EvalError,
  overflow: EvalError,
}

const GROUP_FAULTS: IntegerFaults = IntegerFaults {
  unknown: UnknownGroup,
  unsupported: UnsupportedExpressionForGroup,
  overflow: GroupCalculationOverflow,
};
const GROUP_CHANGE_FAULTS: IntegerFaults = IntegerFaults {
  unknown: UnknownGroup,
  unsupported: UnsupportedExpressionForGroupChange,
  overflow: GroupCalculationOverflow,
};
const LEVEL_FAULTS: IntegerFaults = IntegerFaults {
  unknown: UnknownLevel,
  unsupported: UnsupportedExpressionForLevel,
  overflow: LevelCalculationOverflow,
};
const RADIO_GROUP_FAULTS: IntegerFaults = IntegerFaults {
  unknown: UnknownRadioGroup,
  unsupported: UnsupportedExpressionForRadioGroup,
  overflow: RadioGroupCalculationOverflow,
};
const GROUP_PREFIXES: &[&str] = &["group"];
const LEVEL_PREFIXES: &[&str] = &["level"];
const RADIO_GROUP_PREFIXES: &[&str] = &["group", "radiogroup", "rg"];

const GROUP_MASKS: Keywords<u32> = Keywords {
  words: &[(Meaning::All, u32::MAX), (Meaning::None, 0)],
};
const GROUP_COMPONENTS: Keywords<GroupComponent> = Keywords {
  words: &[
    (Meaning::None, GroupComponent::None),
    (Meaning::Base, GroupComponent::Base),
    (Meaning::Latched, GroupComponent::Latched),
    (Meaning::Locked, GroupComponent::Locked),
    (Meaning::Effective, GroupComponent::Effective),
  ],
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum BooleanValue {
  On,
  Off,
}

const BOOLEANS: Keywords<BooleanValue> = Keywords {
  words: &[
    (Meaning::True, BooleanValue::On),
    (Meaning::Yes, BooleanValue::On),
    (Meaning::On, BooleanValue::On),
    (Meaning::False, BooleanValue::Off),
    (Meaning::No, BooleanValue::Off),
    (Meaning::Off, BooleanValue::Off),
  ],
};
const NAMED_KEYSYMS: Keywords<Keysym> = Keywords {
  words: &[
    (Meaning::Any, syms::NoSymbol),
    (Meaning::Nosymbol, syms::NoSymbol),
    (Meaning::None, syms::VoidSymbol),
    (Meaning::Voidsymbol, syms::VoidSymbol),
  ],
};
const REAL_MODIFIERS: Keywords<u32> = Keywords {
  words: &[
    (Meaning::Shift, 0),
    (Meaning::Lock, 1),
    (Meaning::Control, 2),
    (Meaning::Mod1, 3),
    (Meaning::Mod2, 4),
    (Meaning::Mod3, 5),
    (Meaning::Mod4, 6),
    (Meaning::Mod5, 7),
  ],
};
const MODIFIER_MASKS: Keywords<u32> = Keywords {
  words: &[(Meaning::None, 0x00), (Meaning::All, 0xff)],
};
const PREDICATES: Keywords<Predicate> = Keywords {
  words: &[
    (Meaning::NoneOf, Predicate::NoneOf),
    (Meaning::AnyOfOrNone, Predicate::AnyOfOrNone),
    (Meaning::AnyOf, Predicate::AnyOf),
    (Meaning::AllOf, Predicate::AllOf),
    (Meaning::Exactly, Predicate::Exactly),
  ],
};

fn path_of(expr: &Expr) -> Result<Option<&Path>, Never> {
  Ok(match expr {
    Expr::Path(path) => Some(path),
    Expr::UnMinus(_)
    | Expr::UnPlus(_)
    | Expr::UnNot(_)
    | Expr::UnInverse(_)
    | Expr::Call(_)
    | Expr::String(_)
    | Expr::Integer(..)
    | Expr::Float(..)
    | Expr::KeyName(_)
    | Expr::Parenthesized(_)
    | Expr::Mul(..)
    | Expr::Div(..)
    | Expr::Add(..)
    | Expr::Sub(..)
    | Expr::BracketList(_)
    | Expr::BraceList(_) => None,
  })
}

fn call_of(expr: &Expr) -> Result<Option<&Call>, Never> {
  Ok(match expr {
    Expr::Call(call) => Some(call),
    Expr::UnMinus(_)
    | Expr::UnPlus(_)
    | Expr::UnNot(_)
    | Expr::UnInverse(_)
    | Expr::Path(_)
    | Expr::String(_)
    | Expr::Integer(..)
    | Expr::Float(..)
    | Expr::KeyName(_)
    | Expr::Parenthesized(_)
    | Expr::Mul(..)
    | Expr::Div(..)
    | Expr::Add(..)
    | Expr::Sub(..)
    | Expr::BracketList(_)
    | Expr::BraceList(_) => None,
  })
}

fn key_name_of(expr: &Expr) -> Result<Option<Interned>, Never> {
  Ok(match expr {
    Expr::KeyName(name) => Some(*name),
    Expr::UnMinus(_)
    | Expr::UnPlus(_)
    | Expr::UnNot(_)
    | Expr::UnInverse(_)
    | Expr::Path(_)
    | Expr::Call(_)
    | Expr::String(_)
    | Expr::Integer(..)
    | Expr::Float(..)
    | Expr::Parenthesized(_)
    | Expr::Mul(..)
    | Expr::Div(..)
    | Expr::Add(..)
    | Expr::Sub(..)
    | Expr::BracketList(_)
    | Expr::BraceList(_) => None,
  })
}

fn string_of(expr: &Expr) -> Result<Option<Interned>, Never> {
  Ok(match expr {
    Expr::String(string) => Some(*string),
    Expr::UnMinus(_)
    | Expr::UnPlus(_)
    | Expr::UnNot(_)
    | Expr::UnInverse(_)
    | Expr::Path(_)
    | Expr::Call(_)
    | Expr::Integer(..)
    | Expr::Float(..)
    | Expr::KeyName(_)
    | Expr::Parenthesized(_)
    | Expr::Mul(..)
    | Expr::Div(..)
    | Expr::Add(..)
    | Expr::Sub(..)
    | Expr::BracketList(_)
    | Expr::BraceList(_) => None,
  })
}

fn brace_list_of(expr: &Expr) -> Result<Option<&[Spanned<Expr>]>, Never> {
  Ok(match expr {
    Expr::BraceList(members) => Some(members),
    Expr::UnMinus(_)
    | Expr::UnPlus(_)
    | Expr::UnNot(_)
    | Expr::UnInverse(_)
    | Expr::Path(_)
    | Expr::Call(_)
    | Expr::String(_)
    | Expr::Integer(..)
    | Expr::Float(..)
    | Expr::KeyName(_)
    | Expr::Parenthesized(_)
    | Expr::Mul(..)
    | Expr::Div(..)
    | Expr::Add(..)
    | Expr::Sub(..)
    | Expr::BracketList(_) => None,
  })
}

fn bracket_list_of(expr: &Expr) -> Result<Option<&[Spanned<Expr>]>, Never> {
  Ok(match expr {
    Expr::BracketList(members) => Some(members),
    Expr::UnMinus(_)
    | Expr::UnPlus(_)
    | Expr::UnNot(_)
    | Expr::UnInverse(_)
    | Expr::Path(_)
    | Expr::Call(_)
    | Expr::String(_)
    | Expr::Integer(..)
    | Expr::Float(..)
    | Expr::KeyName(_)
    | Expr::Parenthesized(_)
    | Expr::Mul(..)
    | Expr::Div(..)
    | Expr::Add(..)
    | Expr::Sub(..)
    | Expr::BraceList(_) => None,
  })
}

fn unique_ident_of(expr: &Expr) -> Result<Option<Interned>, Never> {
  let Ok(found) = path_of(expr);

  match found {
    Some(path) => path.unique_ident(),
    None => Ok(None),
  }
}

fn meaning_of(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  expr: &Expr,
) -> Result<Option<Meaning>, Never> {
  let Ok(ident) = unique_ident_of(expr);

  match ident {
    Some(name) => meaning_cache.get_case_insensitive(interner, name).map(Some),
    None => Ok(None),
  }
}

enum Shape<'a, V, U, B> {
  Leaf(Spanned<V>),
  Unary(U, &'a Spanned<Expr>),
  Binary(B, &'a Spanned<Expr>, &'a Spanned<Expr>),
}

struct Operands<V> {
  left: Spanned<V>,
  right: Spanned<V>,
}

type ArithmeticShape<'a, A> = Shape<
  'a,
  <A as Arithmetic<'a>>::Value,
  <A as Arithmetic<'a>>::Unary,
  <A as Arithmetic<'a>>::Binary,
>;

trait Arithmetic<'a> {
  type Value;
  type Unary;
  type Binary;

  fn unsupported(&self) -> Result<EvalError, Never>;

  fn shape(
    &mut self,
    expr: Spanned<&'a Expr>,
  ) -> Result<ArithmeticShape<'a, Self>, Spanned<EvalError>>;

  fn unary(
    &mut self,
    operator: Self::Unary,
    operand: Spanned<Self::Value>,
    span: Span,
  ) -> Result<Spanned<Self::Value>, Spanned<EvalError>>;

  fn binary(
    &mut self,
    operator: Self::Binary,
    operands: Operands<Self::Value>,
    span: Span,
  ) -> Result<Spanned<Self::Value>, Spanned<EvalError>>;
}

enum Pending<'a, V, U, B> {
  Unary(U, Span),
  Right(B, Span, &'a Spanned<Expr>),
  Left(B, Span, Spanned<V>),
}

enum Next<'a, V> {
  Evaluate(Spanned<&'a Expr>),
  Return(Spanned<V>),
}

enum Progress<V> {
  Working,
  Finished(Spanned<V>),
}

struct Evaluation<'a, 'm, A: Arithmetic<'a>> {
  arithmetic: &'m mut A,
  pending: Vec<Pending<'a, A::Value, A::Unary, A::Binary>>,
  next: Option<Next<'a, A::Value>>,
}

impl<'a, A: Arithmetic<'a>> Evaluation<'a, '_, A> {
  fn proceed(
    &mut self,
    next: Next<'a, A::Value>,
  ) -> Result<Progress<A::Value>, Spanned<EvalError>> {
    self.next = Some(next);
    Ok(Progress::Working)
  }

  fn enter(&mut self, expr: Spanned<&'a Expr>) -> Result<Progress<A::Value>, Spanned<EvalError>> {
    let shape = self.arithmetic.shape(expr)?;

    match shape {
      Shape::Leaf(value) => self.proceed(Next::Return(value)),
      Shape::Unary(operator, operand) => {
        self.pending.push(Pending::Unary(operator, expr.span));
        let Ok(borrowed) = operand.as_ref();
        self.proceed(Next::Evaluate(borrowed))
      }
      Shape::Binary(operator, left, right) => {
        self
          .pending
          .push(Pending::Right(operator, expr.span, right));
        let Ok(borrowed) = left.as_ref();
        self.proceed(Next::Evaluate(borrowed))
      }
    }
  }

  fn leave(
    &mut self,
    value: Spanned<A::Value>,
  ) -> Result<Progress<A::Value>, Spanned<EvalError>> {
    match self.pending.pop() {
      None => Ok(Progress::Finished(value)),
      Some(Pending::Unary(operator, span)) => {
        let result = self.arithmetic.unary(operator, value, span)?;
        self.proceed(Next::Return(result))
      }
      Some(Pending::Right(operator, span, right)) => {
        self.pending.push(Pending::Left(operator, span, value));
        let Ok(borrowed) = right.as_ref();
        self.proceed(Next::Evaluate(borrowed))
      }
      Some(Pending::Left(operator, span, left)) => {
        let operands = Operands { left, right: value };
        let result = self.arithmetic.binary(operator, operands, span)?;
        self.proceed(Next::Return(result))
      }
    }
  }
}

impl<'a, A: Arithmetic<'a>> Iterator for Evaluation<'a, '_, A> {
  type Item = Result<Progress<A::Value>, Spanned<EvalError>>;

  fn next(&mut self) -> Option<Self::Item> {
    let next = self.next.take()?;

    Some(match next {
      Next::Evaluate(expr) => self.enter(expr),
      Next::Return(value) => self.leave(value),
    })
  }
}

fn evaluate<'a, A: Arithmetic<'a>>(
  arithmetic: &mut A,
  expr: Spanned<&'a Expr>,
) -> Result<Spanned<A::Value>, Spanned<EvalError>> {
  let Ok(unsupported) = arithmetic.unsupported();
  let mut evaluation = Evaluation {
    arithmetic,
    pending: Vec::new(),
    next: Some(Next::Evaluate(expr)),
  };
  let finished = evaluation.find_map(|progress| match progress {
    Ok(Progress::Working) => None,
    Ok(Progress::Finished(value)) => Some(Ok(value)),
    Err(fault) => Some(Err(fault)),
  });

  match finished {
    Some(result) => result,
    None => Err(unsupported.spanned2(expr.span)),
  }
}

enum IntegerNames<'n> {
  Numbered(&'n [&'n str]),
  Groups(&'n mut MeaningCache),
}

struct Integers<'n> {
  interner: &'n Interner,
  names: IntegerNames<'n>,
  faults: IntegerFaults,
}

#[derive(Copy, Clone)]
enum IntegerUnary {
  Negate,
  Plus,
  Not,
  Invert,
  Parenthesized,
}

#[derive(Copy, Clone)]
enum IntegerBinary {
  Multiply,
  Divide,
  Add,
  Subtract,
}

impl Integers<'_> {
  fn number(&mut self, name: Interned) -> Result<i64, EvalError> {
    match &mut self.names {
      IntegerNames::Numbered(prefixes) => {
        let Ok(parsed) = parse_u32_1_to_32_str_prefix(self.interner, name, prefixes);
        parsed.map(i64::from).ok_or(self.faults.unknown)
      }
      IntegerNames::Groups(meaning_cache) => {
        let Ok(meaning) = meaning_cache.get_case_insensitive(self.interner, name);
        let Ok(spoken) = GROUP_MASKS.get(meaning);
        let Ok(parsed) = parse_u32_1_to_32_str_prefix(self.interner, name, GROUP_PREFIXES);

        match spoken {
          Some(mask) => Ok(i64::from(mask)),
          None => parsed
            .and_then(|group| group.checked_sub(1))
            .and_then(|shift| 1_i64.checked_shl(shift))
            .ok_or(UnknownGroup),
        }
      }
    }
  }

  fn named(&mut self, path: &Path) -> Result<i64, EvalError> {
    let Ok(ident) = path.unique_ident();

    match ident {
      Some(name) => self.number(name),
      None => Err(self.faults.unknown),
    }
  }
}

impl<'a> Arithmetic<'a> for Integers<'_> {
  type Value = i64;
  type Unary = IntegerUnary;
  type Binary = IntegerBinary;

  fn unsupported(&self) -> Result<EvalError, Never> {
    Ok(self.faults.unsupported)
  }

  fn shape(
    &mut self,
    expr: Spanned<&'a Expr>,
  ) -> Result<Shape<'a, i64, IntegerUnary, IntegerBinary>, Spanned<EvalError>> {
    match expr.val {
      Expr::UnMinus(operand) => Ok(Shape::Unary(IntegerUnary::Negate, operand)),
      Expr::UnPlus(operand) => Ok(Shape::Unary(IntegerUnary::Plus, operand)),
      Expr::UnNot(operand) => Ok(Shape::Unary(IntegerUnary::Not, operand)),
      Expr::UnInverse(operand) => Ok(Shape::Unary(IntegerUnary::Invert, operand)),
      Expr::Parenthesized(operand) => Ok(Shape::Unary(IntegerUnary::Parenthesized, operand)),
      Expr::Mul(left, right) => Ok(Shape::Binary(IntegerBinary::Multiply, left, right)),
      Expr::Div(left, right) => Ok(Shape::Binary(IntegerBinary::Divide, left, right)),
      Expr::Add(left, right) => Ok(Shape::Binary(IntegerBinary::Add, left, right)),
      Expr::Sub(left, right) => Ok(Shape::Binary(IntegerBinary::Subtract, left, right)),
      Expr::Integer(_, integer) => Ok(Shape::Leaf((*integer).spanned2(expr.span))),
      Expr::Path(path) => self.named(path).span_either(expr.span).map(Shape::Leaf),
      Expr::Call(_)
      | Expr::String(_)
      | Expr::Float(..)
      | Expr::KeyName(_)
      | Expr::BracketList(_)
      | Expr::BraceList(_) => Err(self.faults.unsupported.spanned2(expr.span)),
    }
  }

  fn unary(
    &mut self,
    operator: IntegerUnary,
    operand: Spanned<i64>,
    span: Span,
  ) -> Result<Spanned<i64>, Spanned<EvalError>> {
    let value = operand.val;
    let result = match operator {
      IntegerUnary::Negate => value.checked_neg().ok_or(self.faults.overflow),
      IntegerUnary::Plus | IntegerUnary::Parenthesized => Ok(value),
      IntegerUnary::Not => Ok(i64::from(value == 0)),
      IntegerUnary::Invert => Ok(!value & i64::from(u32::MAX)),
    };
    result.span_either(span)
  }

  fn binary(
    &mut self,
    operator: IntegerBinary,
    operands: Operands<i64>,
    span: Span,
  ) -> Result<Spanned<i64>, Spanned<EvalError>> {
    let left = operands.left.val;
    let right = operands.right.val;
    let result = match operator {
      IntegerBinary::Multiply => left.checked_mul(right),
      IntegerBinary::Divide => left.checked_div(right),
      IntegerBinary::Add => left.checked_add(right),
      IntegerBinary::Subtract => left.checked_sub(right),
    };
    result.ok_or(self.faults.overflow).span_either(span)
  }
}

pub(crate) fn eval_group(
  interner: &Interner,
  expr: Spanned<&Expr>,
) -> Result<Spanned<GroupIdx>, Spanned<EvalError>> {
  let res = eval_u32_str_prefix(interner, expr, GROUP_PREFIXES, GROUP_FAULTS)?;
  let Ok(group) = GroupIdx::new(res.val);
  group.ok_or(GroupOutOfBounds).span_either(res.span)
}

pub(crate) fn eval_group_mask(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  expr: Spanned<&Expr>,
) -> Result<Spanned<GroupMask>, Spanned<EvalError>> {
  let integers = Integers {
    interner,
    names: IntegerNames::Groups(meaning_cache),
    faults: GROUP_FAULTS,
  };
  eval_u32(integers, expr).span_map(GroupMask)
}

struct Masks<'n, T> {
  interner: &'n Interner,
  meaning_cache: &'n mut MeaningCache,
  read: fn(Meaning) -> Result<Option<T>, Never>,
  unknown: EvalError,
  unsupported: EvalError,
}

#[derive(Copy, Clone)]
enum MaskUnary {
  Invert,
  Parenthesized,
}

#[derive(Copy, Clone)]
enum MaskBinary {
  Union,
  Difference,
}

impl<T> Masks<'_, T> {
  fn named(&mut self, path: &Path) -> Result<T, EvalError> {
    let Ok(ident) = path.unique_ident();

    match ident {
      Some(name) => {
        let Ok(meaning) = self.meaning_cache.get_case_insensitive(self.interner, name);
        let Ok(spoken) = (self.read)(meaning);
        spoken.ok_or(self.unknown)
      }
      None => Err(self.unknown),
    }
  }
}

impl<'a, T> Arithmetic<'a> for Masks<'_, T>
where
  T: BitAnd<Output = T> + BitOr<Output = T> + Not<Output = T>,
{
  type Value = T;
  type Unary = MaskUnary;
  type Binary = MaskBinary;

  fn unsupported(&self) -> Result<EvalError, Never> {
    Ok(self.unsupported)
  }

  fn shape(
    &mut self,
    expr: Spanned<&'a Expr>,
  ) -> Result<Shape<'a, T, MaskUnary, MaskBinary>, Spanned<EvalError>> {
    match expr.val {
      Expr::UnNot(operand) | Expr::UnInverse(operand) => {
        Ok(Shape::Unary(MaskUnary::Invert, operand))
      }
      Expr::Parenthesized(operand) => Ok(Shape::Unary(MaskUnary::Parenthesized, operand)),
      Expr::Add(left, right) => Ok(Shape::Binary(MaskBinary::Union, left, right)),
      Expr::Sub(left, right) => Ok(Shape::Binary(MaskBinary::Difference, left, right)),
      Expr::Path(path) => self.named(path).span_either(expr.span).map(Shape::Leaf),
      Expr::UnMinus(_)
      | Expr::UnPlus(_)
      | Expr::Call(_)
      | Expr::String(_)
      | Expr::Integer(..)
      | Expr::Float(..)
      | Expr::KeyName(_)
      | Expr::Mul(..)
      | Expr::Div(..)
      | Expr::BracketList(_)
      | Expr::BraceList(_) => Err(self.unsupported.spanned2(expr.span)),
    }
  }

  fn unary(
    &mut self,
    operator: MaskUnary,
    operand: Spanned<T>,
    span: Span,
  ) -> Result<Spanned<T>, Spanned<EvalError>> {
    Ok(match operator {
      MaskUnary::Invert => (!operand.val).spanned2(span),
      MaskUnary::Parenthesized => operand.val.spanned2(span),
    })
  }

  fn binary(
    &mut self,
    operator: MaskBinary,
    operands: Operands<T>,
    span: Span,
  ) -> Result<Spanned<T>, Spanned<EvalError>> {
    let left = operands.left.val;
    let right = operands.right.val;

    Ok(match operator {
      MaskBinary::Union => (left | right).spanned2(span),
      MaskBinary::Difference => (left & !right).spanned2(span),
    })
  }
}

pub(crate) fn eval_control_mask(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  expr: Spanned<&Expr>,
) -> Result<Spanned<ControlMask>, Spanned<EvalError>> {
  let mut masks = Masks {
    interner,
    meaning_cache,
    read: ControlMask::from_meaning,
    unknown: UnknownControl,
    unsupported: UnsupportedExpressionForControl,
  };
  evaluate(&mut masks, expr)
}

pub(crate) fn eval_mod_component_mask(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  expr: Spanned<&Expr>,
) -> Result<Spanned<ModComponentMask>, Spanned<EvalError>> {
  let mut masks = Masks {
    interner,
    meaning_cache,
    read: ModComponentMask::from_meaning,
    unknown: UnknownModComponent,
    unsupported: UnsupportedExpressionForModComponent,
  };
  evaluate(&mut masks, expr)
}

pub(crate) fn eval_group_component(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  expr: Spanned<&Expr>,
) -> Result<Spanned<GroupComponent>, Spanned<EvalError>> {
  let interned = eval_interned(expr, UnsupportedExpressionForGroupComponent)?;
  let Ok(meaning) = meaning_cache.get_case_insensitive(interner, interned.val);
  let Ok(component) = GROUP_COMPONENTS.get(meaning);
  component
    .ok_or(UnknownGroupComponent)
    .span_either(expr.span)
}

pub(crate) fn eval_level(
  interner: &Interner,
  expr: Spanned<&Expr>,
) -> Result<Spanned<Level>, Spanned<EvalError>> {
  let res = eval_u32_str_prefix(interner, expr, LEVEL_PREFIXES, LEVEL_FAULTS)?;
  let Ok(level) = Level::new(res.val);
  level.ok_or(LevelOutOfBounds).span_either(res.span)
}

fn eval_radio_group(
  interner: &Interner,
  expr: Spanned<&Expr>,
) -> Result<Spanned<RadioGroup>, Spanned<EvalError>> {
  let rg = eval_radio_group_or_0(interner, expr)?;
  rg.val.ok_or(RadioGroupOutOfBounds).span_either(rg.span)
}

fn eval_radio_group_or_0(
  interner: &Interner,
  expr: Spanned<&Expr>,
) -> Result<Spanned<Option<RadioGroup>>, Spanned<EvalError>> {
  let res = eval_u32_str_prefix(interner, expr, RADIO_GROUP_PREFIXES, RADIO_GROUP_FAULTS)?;

  match ::core::primitive::bool::then_some(res.val == 0, ()) {
    ::core::option::Option::Some(()) => Ok(None.spanned2(res.span)),
    ::core::option::Option::None => {
      let Ok(radio_group) = RadioGroup::new(res.val);
      radio_group
        .map(Some)
        .ok_or(RadioGroupOutOfBounds)
        .span_either(res.span)
    }
  }
}

fn parse_u32_1_to_32_str_prefix(
  interner: &Interner,
  name: Interned,
  prefixes: &[&str],
) -> Result<Option<u32>, Never> {
  let Ok(spelled) = interner.get(name);
  let number = prefixes
    .iter()
    .find_map(|prefix| {
      spelled
        .split_at_checked(prefix.len())
        .filter(|(head, _digits)| head.eq_ignore_ascii_case(prefix.as_bytes()))
        .map(|(_head, digits)| digits)
    })
    .and_then(u32::from_bytes_dec);
  Ok(number.filter(|group| (1..=u32::BITS).contains(group)))
}

fn eval_u32_str_prefix(
  interner: &Interner,
  expr: Spanned<&Expr>,
  prefixes: &[&str],
  faults: IntegerFaults,
) -> Result<Spanned<u32>, Spanned<EvalError>> {
  let integers = Integers {
    interner,
    names: IntegerNames::Numbered(prefixes),
    faults,
  };
  eval_u32(integers, expr)
}

fn eval_i32_str_prefix(
  interner: &Interner,
  expr: Spanned<&Expr>,
  prefixes: &[&str],
  faults: IntegerFaults,
) -> Result<Spanned<i32>, Spanned<EvalError>> {
  let mut integers = Integers {
    interner,
    names: IntegerNames::Numbered(prefixes),
    faults,
  };
  let res = evaluate(&mut integers, expr)?;
  i32::try_from(res.val)
    .map_err(|_| faults.overflow)
    .span_either(res.span)
}

fn eval_u32(
  mut integers: Integers<'_>,
  expr: Spanned<&Expr>,
) -> Result<Spanned<u32>, Spanned<EvalError>> {
  let overflow = integers.faults.overflow;
  let res = evaluate(&mut integers, expr)?;
  u32::try_from(res.val)
    .map_err(|_| overflow)
    .span_either(expr.span)
}

fn eval_keycode(
  keycodes: &ResolvedKeycodes,
  expr: Spanned<&Expr>,
) -> Result<Spanned<(Interned, Keycode)>, Spanned<EvalError>> {
  let Ok(key_name) = key_name_of(expr.val);
  let name = key_name.ok_or(UnsupportedExpressionForKeycode.spanned2(expr.span))?;
  let key = keycodes
    .name_to_key
    .get(&name)
    .ok_or(UnknownKeycode.spanned2(expr.span))?;
  let resolved = match key.kind {
    ResolvedKeyKind::Real(code) => (name, code.val),
    ResolvedKeyKind::Alias(alias, code, target) => match target {
      Some(target_name) => (target_name.val, code),
      None => (alias.val, code),
    },
  };
  Ok(resolved.spanned2(expr.span))
}

#[derive(Copy, Clone)]
enum Sign {
  Positive,
  Negative,
}

pub(crate) fn eval_group_change(
  interner: &Interner,
  expr: Spanned<&Expr>,
) -> Result<Spanned<GroupChange>, Spanned<EvalError>> {
  match expr.val {
    Expr::UnPlus(operand) => eval_relative_group(interner, operand, Sign::Positive),
    Expr::UnMinus(operand) => eval_relative_group(interner, operand, Sign::Negative),
    Expr::UnNot(_)
    | Expr::UnInverse(_)
    | Expr::Path(_)
    | Expr::Call(_)
    | Expr::String(_)
    | Expr::Integer(..)
    | Expr::Float(..)
    | Expr::KeyName(_)
    | Expr::Parenthesized(_)
    | Expr::Mul(..)
    | Expr::Div(..)
    | Expr::Add(..)
    | Expr::Sub(..)
    | Expr::BracketList(_)
    | Expr::BraceList(_) => eval_group(interner, expr).span_map(GroupChange::Absolute),
  }
}

fn eval_relative_group(
  interner: &Interner,
  operand: &Spanned<Expr>,
  sign: Sign,
) -> Result<Spanned<GroupChange>, Spanned<EvalError>> {
  let Ok(borrowed) = operand.as_ref();
  let res = eval_i32_str_prefix(interner, borrowed, GROUP_PREFIXES, GROUP_CHANGE_FAULTS)?;
  let signed = match sign {
    Sign::Positive => Some(res.val),
    Sign::Negative => res.val.checked_neg(),
  };

  match signed {
    Some(change) => Ok(GroupChange::Rel(change).spanned2(res.span)),
    None => Err(GroupCalculationOverflow.spanned2(operand.span)),
  }
}

pub(crate) fn eval_string(
  map: &mut CodeMap,
  diagnostics: &mut DiagnosticSink,
  interner: &mut Interner,
  cooker: &mut StringCooker,
  expr: Spanned<&Expr>,
) -> Result<Spanned<Interned>, Spanned<EvalError>> {
  let Ok(string) = string_of(expr.val);
  let raw = string.ok_or(UnsupportedExpressionForString.spanned2(expr.span))?;
  let Ok(cooked) = cooker.cook(map, diagnostics, interner, raw.spanned2(expr.span));
  Ok(cooked.spanned2(expr.span))
}

struct Booleans<'n> {
  interner: &'n Interner,
  meaning_cache: &'n mut MeaningCache,
}

#[derive(Copy, Clone)]
enum BooleanUnary {
  Not,
}

impl<'a> Arithmetic<'a> for Booleans<'_> {
  type Value = BooleanValue;
  type Unary = BooleanUnary;
  type Binary = Never;

  fn unsupported(&self) -> Result<EvalError, Never> {
    Ok(UnsupportedExpressionForBoolean)
  }

  fn shape(
    &mut self,
    expr: Spanned<&'a Expr>,
  ) -> Result<Shape<'a, BooleanValue, BooleanUnary, Never>, Spanned<EvalError>> {
    match expr.val {
      Expr::UnNot(operand) | Expr::UnInverse(operand) => {
        Ok(Shape::Unary(BooleanUnary::Not, operand))
      }
      Expr::Path(path) => {
        let Ok(ident) = path.unique_ident();
        let Ok(meaning) = match ident {
          Some(name) => self
            .meaning_cache
            .get_case_insensitive(self.interner, name)
            .map(Some),
          None => Ok(None),
        };
        let Ok(spoken) = match meaning {
          Some(word) => BOOLEANS.get(word),
          None => Ok(None),
        };

        match spoken {
          Some(value) => Ok(Shape::Leaf(value.spanned2(expr.span))),
          None => Err(UnknownBooleanValue.spanned2(expr.span)),
        }
      }
      Expr::UnMinus(_)
      | Expr::UnPlus(_)
      | Expr::Call(_)
      | Expr::String(_)
      | Expr::Integer(..)
      | Expr::Float(..)
      | Expr::KeyName(_)
      | Expr::Parenthesized(_)
      | Expr::Mul(..)
      | Expr::Div(..)
      | Expr::Add(..)
      | Expr::Sub(..)
      | Expr::BracketList(_)
      | Expr::BraceList(_) => Err(UnsupportedExpressionForBoolean.spanned2(expr.span)),
    }
  }

  fn unary(
    &mut self,
    operator: BooleanUnary,
    operand: Spanned<BooleanValue>,
    span: Span,
  ) -> Result<Spanned<BooleanValue>, Spanned<EvalError>> {
    Ok(match operator {
      BooleanUnary::Not => match operand.val {
        BooleanValue::On => BooleanValue::Off.spanned2(span),
        BooleanValue::Off => BooleanValue::On.spanned2(span),
      },
    })
  }

  fn binary(
    &mut self,
    operator: Never,
    _operands: Operands<BooleanValue>,
    _span: Span,
  ) -> Result<Spanned<BooleanValue>, Spanned<EvalError>> {
    match operator {}
  }
}

fn eval_boolean(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  expr: Spanned<&Expr>,
) -> Result<BooleanValue, Spanned<EvalError>> {
  let mut booleans = Booleans {
    interner,
    meaning_cache,
  };
  let value = evaluate(&mut booleans, expr)?;
  Ok(value.val)
}

fn flag(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  operand: Option<Spanned<&Expr>>,
  not: Option<Span>,
) -> Result<BooleanValue, Spanned<EvalError>> {
  match operand {
    Some(expr) => eval_boolean(interner, meaning_cache, expr),
    None => Ok(match not {
      Some(_) => BooleanValue::Off,
      None => BooleanValue::On,
    }),
  }
}

pub(crate) fn keysym_from_ident(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  ident: Spanned<Interned>,
) -> Result<Spanned<Keysym>, Spanned<EvalError>> {
  let Ok(meaning) = meaning_cache.get_case_insensitive(interner, ident.val);
  let Ok(spoken) = NAMED_KEYSYMS.get(meaning);

  match spoken {
    Some(sym) => Ok(sym.spanned2(ident.span)),
    None => {
      let Ok(name) = interner.get(ident.val);
      let Ok(keysym) = Keysym::from_str(name);
      keysym.ok_or(UnknownKeysym).span_either(ident.span)
    }
  }
}

type Keysyms = Vec<Keysym>;

fn keysyms(
  map: &mut CodeMap,
  diagnostics: &mut DiagnosticSink<'_, '_>,
  interner: &mut Interner,
  cooker: &mut StringCooker,
  meaning_cache: &mut MeaningCache,
  expr: Spanned<&Expr>,
) -> Result<Keysyms, Spanned<EvalError>> {
  match expr.val {
    Expr::Path(path) => {
      let Ok(unique_ident) = path.unique_ident();
      let ident = unique_ident.ok_or(UnknownKeysym.spanned2(expr.span))?;
      let sym = keysym_from_ident(interner, meaning_cache, ident.spanned2(expr.span))?;
      Ok(vec![sym.val])
    }
    Expr::Integer(_, integer) => {
      let sym = match (*integer, u32::try_from(*integer)) {
        (0..=9, Ok(digit)) => syms::_0.0.checked_add(digit).map(Keysym),
        (_, Ok(code)) => Some(Keysym(code)),
        (_, Err(_negative_or_wider_than_a_keysym)) => None,
      };
      let found = sym.ok_or(KeysymCalculationOverflow.spanned2(expr.span))?;
      Ok(vec![found])
    }
    Expr::String(string) => {
      let Ok(cooked) = cooker.cook(map, diagnostics, interner, (*string).spanned2(expr.span));
      let Ok(bytes) = interner.get(cooked);

      match std::str::from_utf8(bytes) {
        Ok(text) => Ok(
          text
            .chars()
            .map(|c| {
              let Ok(sym) = Keysym::from_char(c);
              sym
            })
            .collect(),
        ),
        Err(_) => Err(KeysymStringNotUtf8.spanned2(expr.span)),
      }
    }
    Expr::UnMinus(_)
    | Expr::UnPlus(_)
    | Expr::UnNot(_)
    | Expr::UnInverse(_)
    | Expr::Call(_)
    | Expr::Float(..)
    | Expr::KeyName(_)
    | Expr::Parenthesized(_)
    | Expr::Mul(..)
    | Expr::Div(..)
    | Expr::Add(..)
    | Expr::Sub(..)
    | Expr::BracketList(_)
    | Expr::BraceList(_) => Err(UnsupportedExpressionForKeysym.spanned2(expr.span)),
  }
}

pub(crate) fn eval_keysyms<M>(
  tables: &mut Tables<'_, '_, '_>,
  expr: Spanned<&Expr>,
  handed: &mut M,
  handle: impl Fn(&mut M, Keysym) -> Result<(), EvalError>,
) -> Result<(), Spanned<EvalError>> {
  let Tables {
    map,
    diagnostics,
    interner,
    cooker,
    meaning_cache,
  } = tables;
  let found = keysyms(map, diagnostics, interner, cooker, meaning_cache, expr)?;

  for sym in found {
    handle(handed, sym).map_err(|fault| fault.spanned2(expr.span))?;
  }

  Ok(())
}

fn meaning_to_real_mod_index(meaning: Meaning) -> Result<Option<ModifierIndex>, Never> {
  let Ok(spoken) = REAL_MODIFIERS.get(meaning);

  match spoken {
    Some(position) => ModifierIndex::new(position),
    None => Ok(None),
  }
}

pub(crate) fn ident_to_real_mod_index(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  ident: Interned,
) -> Result<Option<ModifierIndex>, Never> {
  let Ok(meaning) = meaning_cache.get_case_insensitive(interner, ident);
  meaning_to_real_mod_index(meaning)
}

fn ident_to_real_mod_mask(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  ident: Interned,
) -> Result<Option<u32>, Never> {
  let Ok(meaning) = meaning_cache.get_case_insensitive(interner, ident);
  let Ok(spoken) = MODIFIER_MASKS.get(meaning);
  let Ok(index) = meaning_to_real_mod_index(meaning);

  Ok(match spoken {
    Some(mask) => Some(mask),
    None => index.map(|found| {
      let Ok(mask) = found.to_mask();
      mask.0
    }),
  })
}

enum ModifierNames<'n> {
  Real {
    interner: &'n Interner,
    meaning_cache: &'n mut MeaningCache,
  },
  RealOrVirtual {
    interner: &'n Interner,
    meaning_cache: &'n mut MeaningCache,
    vmods: &'n Vmodmap,
  },
  Virtual(&'n Vmodmap),
}

#[derive(Copy, Clone)]
enum ModifierUnary {
  Invert,
  Parenthesized,
}

#[derive(Copy, Clone)]
enum ModifierBinary {
  Add,
  Subtract,
}

fn virtual_modifier(vmods: &Vmodmap, name: Interned) -> Result<Option<ModifierTree>, Never> {
  let Ok(vmod) = vmods.get(name);
  Ok(vmod.map(|held| {
    let Ok(tree) = ModifierTree::virtual_modifier(held.idx);
    tree
  }))
}

impl ModifierNames<'_> {
  fn modifier(&mut self, name: Interned) -> Result<Option<ModifierTree>, Never> {
    match self {
      ModifierNames::Real {
        interner,
        meaning_cache,
      } => {
        let Ok(mask) = ident_to_real_mod_mask(interner, meaning_cache, name);
        Ok(mask.map(|real| {
          let Ok(tree) = ModifierTree::number(ModifierMask(real));
          tree
        }))
      }
      ModifierNames::RealOrVirtual {
        interner,
        meaning_cache,
        vmods,
      } => {
        let Ok(mask) = ident_to_real_mod_mask(interner, meaning_cache, name);

        match mask {
          Some(real) => {
            let Ok(tree) = ModifierTree::number(ModifierMask(real));
            Ok(Some(tree))
          }
          None => virtual_modifier(vmods, name),
        }
      }
      ModifierNames::Virtual(vmods) => virtual_modifier(vmods, name),
    }
  }

  fn named(&mut self, path: &Path) -> Result<ModifierTree, EvalError> {
    let Ok(ident) = path.unique_ident();
    let Ok(modifier) = match ident {
      Some(name) => self.modifier(name),
      None => Ok(None),
    };
    modifier.ok_or(UnknownModifier)
  }

  fn integer(&self, integer: i64) -> Result<ModifierTree, EvalError> {
    match self {
      ModifierNames::Real { .. } | ModifierNames::RealOrVirtual { .. } => {
        let parsed = u32::try_from(integer).map_err(|_overflow| ModsCalculationOverflow)?;
        let Ok(tree) = ModifierTree::number(ModifierMask(parsed));
        Ok(tree)
      }
      ModifierNames::Virtual(_) => Err(UnsupportedExpressionForModMask),
    }
  }
}

impl<'a> Arithmetic<'a> for ModifierNames<'_> {
  type Value = ModifierTree;
  type Unary = ModifierUnary;
  type Binary = ModifierBinary;

  fn unsupported(&self) -> Result<EvalError, Never> {
    Ok(UnsupportedExpressionForModMask)
  }

  fn shape(
    &mut self,
    expr: Spanned<&'a Expr>,
  ) -> Result<Shape<'a, ModifierTree, ModifierUnary, ModifierBinary>, Spanned<EvalError>> {
    match expr.val {
      Expr::UnInverse(operand) => Ok(Shape::Unary(ModifierUnary::Invert, operand)),
      Expr::Parenthesized(operand) => Ok(Shape::Unary(ModifierUnary::Parenthesized, operand)),
      Expr::Add(left, right) => Ok(Shape::Binary(ModifierBinary::Add, left, right)),
      Expr::Sub(left, right) => Ok(Shape::Binary(ModifierBinary::Subtract, left, right)),
      Expr::Path(path) => self.named(path).span_either(expr.span).map(Shape::Leaf),
      Expr::Integer(_, integer) => self
        .integer(*integer)
        .span_either(expr.span)
        .map(Shape::Leaf),
      Expr::UnMinus(_)
      | Expr::UnPlus(_)
      | Expr::UnNot(_)
      | Expr::Call(_)
      | Expr::String(_)
      | Expr::Float(..)
      | Expr::KeyName(_)
      | Expr::Mul(..)
      | Expr::Div(..)
      | Expr::BracketList(_)
      | Expr::BraceList(_) => Err(UnsupportedExpressionForModMask.spanned2(expr.span)),
    }
  }

  fn unary(
    &mut self,
    operator: ModifierUnary,
    operand: Spanned<ModifierTree>,
    span: Span,
  ) -> Result<Spanned<ModifierTree>, Spanned<EvalError>> {
    Ok(match operator {
      ModifierUnary::Invert => {
        let Ok(inverted) = ModifierTree::invert(operand);
        inverted.spanned2(span)
      }
      ModifierUnary::Parenthesized => operand,
    })
  }

  fn binary(
    &mut self,
    operator: ModifierBinary,
    operands: Operands<ModifierTree>,
    span: Span,
  ) -> Result<Spanned<ModifierTree>, Spanned<EvalError>> {
    let combine = match operator {
      ModifierBinary::Add => Combine::Join,
      ModifierBinary::Subtract => Combine::Remove,
    };
    let Ok(combined) = ModifierTree::combine(combine, operands.left, operands.right);

    Ok(combined.spanned2(span))
  }
}

pub(crate) fn eval_real_mods(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  expr: Spanned<&Expr>,
) -> Result<Spanned<ModifierMask>, Spanned<EvalError>> {
  let mut names = ModifierNames::Real {
    interner,
    meaning_cache,
  };
  evaluate(&mut names, expr).span_map(|tree| {
    let Ok((mask, _)) = tree.eval_real(None);
    mask
  })
}

pub(crate) fn eval_mods(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  vmods: &Vmodmap,
  expr: Spanned<&Expr>,
) -> Result<Spanned<ModifierTree>, Spanned<EvalError>> {
  let mut names = ModifierNames::RealOrVirtual {
    interner,
    meaning_cache,
    vmods,
  };
  evaluate(&mut names, expr)
}

pub(crate) fn eval_virtual_mods(
  vmods: &Vmodmap,
  expr: Spanned<&Expr>,
) -> Result<Spanned<ModifierMask>, Spanned<EvalError>> {
  let mut names = ModifierNames::Virtual(vmods);
  let res = evaluate(&mut names, expr)?;
  let Ok((mask, _)) = res.val.eval_virtual();
  Ok(mask.spanned2(res.span))
}

pub(crate) fn eval_interned(
  expr: Spanned<&Expr>,
  unsupported_expression_type: EvalError,
) -> Result<Spanned<Interned>, Spanned<EvalError>> {
  let Ok(ident) = unique_ident_of(expr.val);

  match ident {
    Some(name) => Ok(name.spanned2(expr.span)),
    None => Err(unsupported_expression_type.spanned2(expr.span)),
  }
}

pub(crate) fn eval_filter(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  expr: Spanned<&Expr>,
) -> Result<Spanned<Filter>, Spanned<EvalError>> {
  let Ok(meaning) = meaning_of(interner, meaning_cache, expr.val);

  match ::core::primitive::bool::then_some(meaning == Some(Meaning::Any), ()) {
    ::core::option::Option::Some(()) => Ok(
      Filter {
        predicate: Predicate::AnyOf,
        mask: ModifierMask(!0).spanned2(expr.span),
      }
      .spanned2(expr.span),
    ),
    ::core::option::Option::None => eval_filter_call(interner, meaning_cache, expr),
  }
}

struct Filtered<'f> {
  predicate: Predicate,
  operand: Spanned<&'f Expr>,
}

fn eval_filter_call(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  expr: Spanned<&Expr>,
) -> Result<Spanned<Filter>, Spanned<EvalError>> {
  let Ok(called) = call_of(expr.val);
  let filtered = match called {
    Some(call) => filter_predicate(interner, meaning_cache, call, expr.span)?,
    None => Filtered {
      predicate: Predicate::Exactly,
      operand: expr,
    },
  };
  let mask = eval_real_mods(interner, meaning_cache, filtered.operand)?;
  Ok(
    Filter {
      predicate: filtered.predicate,
      mask,
    }
    .spanned2(filtered.operand.span),
  )
}

fn filter_predicate<'f>(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  call: &'f Call,
  span: Span,
) -> Result<Filtered<'f>, Spanned<EvalError>> {
  let Ok(unique_ident) = call.path.val.unique_ident();
  let ident = unique_ident.ok_or(UnknownFilterPredicate.spanned2(call.path.span))?;
  let Ok(meaning) = meaning_cache.get_case_insensitive(interner, ident);
  let Ok(spoken) = PREDICATES.get(meaning);
  let predicate = spoken.ok_or(UnknownFilterPredicate.spanned2(call.path.span))?;

  match &*call.args {
    [argument] => match &argument.val {
      CallArg::Expr(operand) => Ok(Filtered {
        predicate,
        operand: operand.spanned2(argument.span),
      }),
      CallArg::NamedParam(named) => Err(NamedFilterArgArgument.spanned2(named.name.span)),
    },
    [] | [_, _, ..] => Err(NotOneInterpretFilterArgument.spanned2(span)),
  }
}

#[derive(Copy, Clone)]
enum ActionParameterValue<'a> {
  Boolean(BooleanValue),
  Expr(&'a Expr),
}

struct ActionContext<'c> {
  interner: &'c Interner,
  meaning_cache: &'c mut MeaningCache,
  keycodes: &'c ResolvedKeycodes,
  vmods: &'c Vmodmap,
}

#[derive(Copy, Clone)]
enum ActionParameter {
  ClearLocks,
  LatchToLock,
  Modifiers,
  Affect,
  Group,
  Keycode,
  ClearModifiers,
  Controls,
}

const ACTION_PARAMETERS: Keywords<ActionParameter> = Keywords {
  words: &[
    (Meaning::ClearLocks, ActionParameter::ClearLocks),
    (Meaning::LatchToLock, ActionParameter::LatchToLock),
    (Meaning::Mods, ActionParameter::Modifiers),
    (Meaning::Modifiers, ActionParameter::Modifiers),
    (Meaning::Affect, ActionParameter::Affect),
    (Meaning::Group, ActionParameter::Group),
    (Meaning::Kc, ActionParameter::Keycode),
    (Meaning::Keycode, ActionParameter::Keycode),
    (Meaning::Key, ActionParameter::Keycode),
    (Meaning::Clearmods, ActionParameter::ClearModifiers),
    (Meaning::Clearmodifiers, ActionParameter::ClearModifiers),
    (Meaning::Controls, ActionParameter::Controls),
    (Meaning::Ctrls, ActionParameter::Controls),
  ],
};
const AFFECTS: Keywords<Affect> = Keywords {
  words: &[
    (Meaning::Lock, Affect::Lock),
    (Meaning::Unlock, Affect::Unlock),
    (Meaning::Both, Affect::Both),
    (Meaning::Neither, Affect::Neither),
  ],
};

trait ActionParameters: Clone {
  const UNKNOWN_PARAMETER: EvalError;

  fn handle_field(
    &mut self,
    context: &mut ActionContext<'_>,
    parameter: Spanned<Option<ActionParameter>>,
    value: Spanned<ActionParameterValue<'_>>,
  ) -> Result<(), Spanned<EvalError>>;
}

fn boolean_value(
  context: &mut ActionContext<'_>,
  value: Spanned<ActionParameterValue<'_>>,
) -> Result<Spanned<BooleanValue>, Spanned<EvalError>> {
  match value.val {
    ActionParameterValue::Boolean(flag) => Ok(flag.spanned2(value.span)),
    ActionParameterValue::Expr(operand) => {
      let flag = eval_boolean(
        context.interner,
        context.meaning_cache,
        operand.spanned2(value.span),
      )?;
      Ok(flag.spanned2(value.span))
    }
  }
}

fn expression_value(
  value: Spanned<ActionParameterValue<'_>>,
  missing: EvalError,
) -> Result<Spanned<&Expr>, Spanned<EvalError>> {
  match value.val {
    ActionParameterValue::Boolean(_) => Err(missing.spanned2(value.span)),
    ActionParameterValue::Expr(operand) => Ok(operand.spanned2(value.span)),
  }
}

fn eval_action_mods(
  context: &mut ActionContext<'_>,
  expr: Spanned<&Expr>,
) -> Result<Spanned<ResolvedActionMods>, Spanned<EvalError>> {
  let Ok(meaning) = meaning_of(context.interner, context.meaning_cache, expr.val);

  match meaning.filter(|found| matches!(found, Meaning::Usemodmapmods | Meaning::Modmapmods)) {
    Some(_modmap) => Ok(ResolvedActionMods::ModMap.spanned2(expr.span)),
    None => eval_mods(context.interner, context.meaning_cache, context.vmods, expr)
      .span_map(ResolvedActionMods::Explicit),
  }
}

fn eval_action_affect(
  context: &mut ActionContext<'_>,
  expr: Spanned<&Expr>,
) -> Result<Affect, Spanned<EvalError>> {
  let Ok(ident) = unique_ident_of(expr.val);

  match ident {
    Some(name) => {
      let Ok(meaning) = context
        .meaning_cache
        .get_case_insensitive(context.interner, name);
      let Ok(spoken) = AFFECTS.get(meaning);
      spoken.ok_or(UnknownLockModsAffect.spanned2(expr.span))
    }
    None => Err(UnsupportedExpressionForLockModsAffect.spanned2(expr.span)),
  }
}

impl ActionParameters for ResolvedNoAction {
  const UNKNOWN_PARAMETER: EvalError = UnknownParameterForNoAction;

  fn handle_field(
    &mut self,
    _context: &mut ActionContext<'_>,
    parameter: Spanned<Option<ActionParameter>>,
    _value: Spanned<ActionParameterValue<'_>>,
  ) -> Result<(), Spanned<EvalError>> {
    Err(Self::UNKNOWN_PARAMETER.spanned2(parameter.span))
  }
}

impl ActionParameters for ResolvedVoidAction {
  const UNKNOWN_PARAMETER: EvalError = UnknownParameterForVoidAction;

  fn handle_field(
    &mut self,
    _context: &mut ActionContext<'_>,
    parameter: Spanned<Option<ActionParameter>>,
    _value: Spanned<ActionParameterValue<'_>>,
  ) -> Result<(), Spanned<EvalError>> {
    Err(Self::UNKNOWN_PARAMETER.spanned2(parameter.span))
  }
}

impl ActionParameters for ResolvedModsSet {
  const UNKNOWN_PARAMETER: EvalError = UnknownParameterForSetMods;

  fn handle_field(
    &mut self,
    context: &mut ActionContext<'_>,
    parameter: Spanned<Option<ActionParameter>>,
    value: Spanned<ActionParameterValue<'_>>,
  ) -> Result<(), Spanned<EvalError>> {
    match parameter.val {
      Some(ActionParameter::ClearLocks) => {
        let clear_locks = boolean_value(context, value)?;
        let Ok(setting) = clear_locks.map(|value_2| value_2 == BooleanValue::On);
        self.clear_locks = Some(setting);
        Ok(())
      }
      Some(ActionParameter::Modifiers) => {
        let operand = expression_value(value, MissingValueForSetModsMods)?;
        let modifiers = eval_action_mods(context, operand)?;
        self.modifiers = Some(modifiers);
        Ok(())
      }
      Some(
        ActionParameter::LatchToLock
        | ActionParameter::Affect
        | ActionParameter::Group
        | ActionParameter::Keycode
        | ActionParameter::ClearModifiers
        | ActionParameter::Controls,
      )
      | None => Err(Self::UNKNOWN_PARAMETER.spanned2(parameter.span)),
    }
  }
}

impl ActionParameters for ResolvedModsLatch {
  const UNKNOWN_PARAMETER: EvalError = UnknownParameterForLatchMods;

  fn handle_field(
    &mut self,
    context: &mut ActionContext<'_>,
    parameter: Spanned<Option<ActionParameter>>,
    value: Spanned<ActionParameterValue<'_>>,
  ) -> Result<(), Spanned<EvalError>> {
    match parameter.val {
      Some(ActionParameter::ClearLocks) => {
        let clear_locks = boolean_value(context, value)?;
        let Ok(setting) = clear_locks.map(|value_2| value_2 == BooleanValue::On);
        self.clear_locks = Some(setting);
        Ok(())
      }
      Some(ActionParameter::LatchToLock) => {
        let latch_to_lock = boolean_value(context, value)?;
        let Ok(setting) = latch_to_lock.map(|value_3| value_3 == BooleanValue::On);
        self.latch_to_lock = Some(setting);
        Ok(())
      }
      Some(ActionParameter::Modifiers) => {
        let operand = expression_value(value, MissingValueForLatchModsMods)?;
        let modifiers = eval_action_mods(context, operand)?;
        self.modifiers = Some(modifiers);
        Ok(())
      }
      Some(
        ActionParameter::Affect
        | ActionParameter::Group
        | ActionParameter::Keycode
        | ActionParameter::ClearModifiers
        | ActionParameter::Controls,
      )
      | None => Err(Self::UNKNOWN_PARAMETER.spanned2(parameter.span)),
    }
  }
}

impl ActionParameters for ResolvedModsLock {
  const UNKNOWN_PARAMETER: EvalError = UnknownParameterForLockMods;

  fn handle_field(
    &mut self,
    context: &mut ActionContext<'_>,
    parameter: Spanned<Option<ActionParameter>>,
    value: Spanned<ActionParameterValue<'_>>,
  ) -> Result<(), Spanned<EvalError>> {
    match parameter.val {
      Some(ActionParameter::Modifiers) => {
        let operand = expression_value(value, MissingValueForLockModsMods)?;
        let modifiers = eval_action_mods(context, operand)?;
        self.modifiers = Some(modifiers);
        Ok(())
      }
      Some(ActionParameter::Affect) => {
        let operand = expression_value(value, MissingValueForLockModsAffect)?;
        let affect = eval_action_affect(context, operand)?;
        self.affect = Some(affect.spanned2(value.span));
        Ok(())
      }
      Some(
        ActionParameter::ClearLocks
        | ActionParameter::LatchToLock
        | ActionParameter::Group
        | ActionParameter::Keycode
        | ActionParameter::ClearModifiers
        | ActionParameter::Controls,
      )
      | None => Err(Self::UNKNOWN_PARAMETER.spanned2(parameter.span)),
    }
  }
}

impl ActionParameters for ResolvedGroupSet {
  const UNKNOWN_PARAMETER: EvalError = UnknownParameterForSetGroup;

  fn handle_field(
    &mut self,
    context: &mut ActionContext<'_>,
    parameter: Spanned<Option<ActionParameter>>,
    value: Spanned<ActionParameterValue<'_>>,
  ) -> Result<(), Spanned<EvalError>> {
    match parameter.val {
      Some(ActionParameter::Group) => {
        let operand = expression_value(value, MissingValueForSetGroupGroup)?;
        let group = eval_group_change(context.interner, operand)?;
        self.group = Some(group);
        Ok(())
      }
      Some(ActionParameter::ClearLocks) => {
        let clear_locks = boolean_value(context, value)?;
        let Ok(setting) = clear_locks.map(|value_2| value_2 == BooleanValue::On);
        self.clear_locks = Some(setting);
        Ok(())
      }
      Some(
        ActionParameter::LatchToLock
        | ActionParameter::Modifiers
        | ActionParameter::Affect
        | ActionParameter::Keycode
        | ActionParameter::ClearModifiers
        | ActionParameter::Controls,
      )
      | None => Err(Self::UNKNOWN_PARAMETER.spanned2(parameter.span)),
    }
  }
}

impl ActionParameters for ResolvedGroupLatch {
  const UNKNOWN_PARAMETER: EvalError = UnknownParameterForLatchGroup;

  fn handle_field(
    &mut self,
    context: &mut ActionContext<'_>,
    parameter: Spanned<Option<ActionParameter>>,
    value: Spanned<ActionParameterValue<'_>>,
  ) -> Result<(), Spanned<EvalError>> {
    match parameter.val {
      Some(ActionParameter::Group) => {
        let operand = expression_value(value, MissingValueForLatchGroupGroup)?;
        let group = eval_group_change(context.interner, operand)?;
        self.group = Some(group);
        Ok(())
      }
      Some(ActionParameter::ClearLocks) => {
        let clear_locks = boolean_value(context, value)?;
        let Ok(setting) = clear_locks.map(|value_2| value_2 == BooleanValue::On);
        self.clear_locks = Some(setting);
        Ok(())
      }
      Some(ActionParameter::LatchToLock) => {
        let latch_to_lock = boolean_value(context, value)?;
        let Ok(setting) = latch_to_lock.map(|value_3| value_3 == BooleanValue::On);
        self.latch_to_lock = Some(setting);
        Ok(())
      }
      Some(
        ActionParameter::Modifiers
        | ActionParameter::Affect
        | ActionParameter::Keycode
        | ActionParameter::ClearModifiers
        | ActionParameter::Controls,
      )
      | None => Err(Self::UNKNOWN_PARAMETER.spanned2(parameter.span)),
    }
  }
}

impl ActionParameters for ResolvedGroupLock {
  const UNKNOWN_PARAMETER: EvalError = UnknownParameterForLockGroup;

  fn handle_field(
    &mut self,
    context: &mut ActionContext<'_>,
    parameter: Spanned<Option<ActionParameter>>,
    value: Spanned<ActionParameterValue<'_>>,
  ) -> Result<(), Spanned<EvalError>> {
    match parameter.val {
      Some(ActionParameter::Group) => {
        let operand = expression_value(value, MissingValueForLockGroupGroup)?;
        let group = eval_group_change(context.interner, operand)?;
        self.group = Some(group);
        Ok(())
      }
      Some(
        ActionParameter::ClearLocks
        | ActionParameter::LatchToLock
        | ActionParameter::Modifiers
        | ActionParameter::Affect
        | ActionParameter::Keycode
        | ActionParameter::ClearModifiers
        | ActionParameter::Controls,
      )
      | None => Err(Self::UNKNOWN_PARAMETER.spanned2(parameter.span)),
    }
  }
}

impl ActionParameters for ResolvedRedirectKey {
  const UNKNOWN_PARAMETER: EvalError = UnknownParameterForRedirectKey;

  fn handle_field(
    &mut self,
    context: &mut ActionContext<'_>,
    parameter: Spanned<Option<ActionParameter>>,
    value: Spanned<ActionParameterValue<'_>>,
  ) -> Result<(), Spanned<EvalError>> {
    match parameter.val {
      Some(ActionParameter::Keycode) => {
        let operand = expression_value(value, MissingValueForRedirectKeyKey)?;
        let keycode = eval_keycode(context.keycodes, operand)?;
        self.keycode = Some(keycode);
        Ok(())
      }
      Some(ActionParameter::ClearModifiers) => {
        let operand = expression_value(value, MissingValueForRedirectKeyClearmods)?;
        let mods_to_clear = eval_action_mods(context, operand)?;
        self.mods_to_clear = Some(mods_to_clear);
        Ok(())
      }
      Some(ActionParameter::Modifiers) => {
        let operand = expression_value(value, MissingValueForRedirectKeyMods)?;
        let mods_to_set = eval_action_mods(context, operand)?;
        self.mods_to_set = Some(mods_to_set);
        Ok(())
      }
      Some(
        ActionParameter::ClearLocks
        | ActionParameter::LatchToLock
        | ActionParameter::Affect
        | ActionParameter::Group
        | ActionParameter::Controls,
      )
      | None => Err(Self::UNKNOWN_PARAMETER.spanned2(parameter.span)),
    }
  }
}

impl ActionParameters for ResolvedSetControls {
  const UNKNOWN_PARAMETER: EvalError = UnknownParameterForSetControls;

  fn handle_field(
    &mut self,
    context: &mut ActionContext<'_>,
    parameter: Spanned<Option<ActionParameter>>,
    value: Spanned<ActionParameterValue<'_>>,
  ) -> Result<(), Spanned<EvalError>> {
    match parameter.val {
      Some(ActionParameter::Controls) => {
        let operand = expression_value(value, MissingValueForSetControlsControls)?;
        let controls = eval_control_mask(context.interner, context.meaning_cache, operand)?;
        self.controls = Some(controls);
        Ok(())
      }
      Some(
        ActionParameter::ClearLocks
        | ActionParameter::LatchToLock
        | ActionParameter::Modifiers
        | ActionParameter::Affect
        | ActionParameter::Group
        | ActionParameter::Keycode
        | ActionParameter::ClearModifiers,
      )
      | None => Err(Self::UNKNOWN_PARAMETER.spanned2(parameter.span)),
    }
  }
}

impl ActionParameters for ResolvedLockControls {
  const UNKNOWN_PARAMETER: EvalError = UnknownParameterForLockControls;

  fn handle_field(
    &mut self,
    context: &mut ActionContext<'_>,
    parameter: Spanned<Option<ActionParameter>>,
    value: Spanned<ActionParameterValue<'_>>,
  ) -> Result<(), Spanned<EvalError>> {
    match parameter.val {
      Some(ActionParameter::Controls) => {
        let operand = expression_value(value, MissingValueForLockControlsControls)?;
        let controls = eval_control_mask(context.interner, context.meaning_cache, operand)?;
        self.controls = Some(controls);
        Ok(())
      }
      Some(ActionParameter::Affect) => {
        let operand = expression_value(value, MissingValueForLockControlsAffect)?;
        let affect = eval_action_affect(context, operand)?;
        self.affect = Some(affect.spanned2(value.span));
        Ok(())
      }
      Some(
        ActionParameter::ClearLocks
        | ActionParameter::LatchToLock
        | ActionParameter::Modifiers
        | ActionParameter::Group
        | ActionParameter::Keycode
        | ActionParameter::ClearModifiers,
      )
      | None => Err(Self::UNKNOWN_PARAMETER.spanned2(parameter.span)),
    }
  }
}

struct ActionArgument<'a> {
  path: Spanned<&'a Path>,
  value: Spanned<ActionParameterValue<'a>>,
}

fn action_argument(
  argument: &Spanned<CallArg>,
  unknown: EvalError,
) -> Result<ActionArgument<'_>, Spanned<EvalError>> {
  match &argument.val {
    CallArg::Expr(operand) => match operand {
      Expr::Path(path) => Ok(ActionArgument {
        path: path.spanned2(argument.span),
        value: ActionParameterValue::Boolean(BooleanValue::On).spanned2(argument.span),
      }),
      Expr::UnNot(negated) | Expr::UnInverse(negated) => {
        let Ok(found) = path_of(&negated.val);

        match found {
          Some(path) => Ok(ActionArgument {
            path: path.spanned2(negated.span),
            value: ActionParameterValue::Boolean(BooleanValue::Off).spanned2(argument.span),
          }),
          None => Err(unknown.spanned2(argument.span)),
        }
      }
      Expr::UnMinus(_)
      | Expr::UnPlus(_)
      | Expr::Call(_)
      | Expr::String(_)
      | Expr::Integer(..)
      | Expr::Float(..)
      | Expr::KeyName(_)
      | Expr::Parenthesized(_)
      | Expr::Mul(..)
      | Expr::Div(..)
      | Expr::Add(..)
      | Expr::Sub(..)
      | Expr::BracketList(_)
      | Expr::BraceList(_) => Err(unknown.spanned2(argument.span)),
    },
    CallArg::NamedParam(named) => Ok(ActionArgument {
      path: (&named.name.val).spanned2(named.name.span),
      value: ActionParameterValue::Expr(&named.expr.val).spanned2(named.expr.span),
    }),
  }
}

fn create_action<T: ActionParameters>(
  context: &mut ActionContext<'_>,
  default: &T,
  args: &[Spanned<CallArg>],
) -> Result<T, Spanned<EvalError>> {
  let mut action = default.clone();

  for argument in args {
    let parsed = action_argument(argument, T::UNKNOWN_PARAMETER)?;
    let Ok(unique_ident) = parsed.path.val.unique_ident();
    let ident = unique_ident.ok_or(T::UNKNOWN_PARAMETER.spanned2(parsed.path.span))?;
    let Ok(meaning) = context
      .meaning_cache
      .get_case_insensitive(context.interner, ident);
    let Ok(parameter) = ACTION_PARAMETERS.get(meaning);
    action.handle_field(context, parameter.spanned2(parsed.path.span), parsed.value)?;
  }

  Ok(action)
}

fn handle_action_global<T: ActionParameters>(
  context: &mut ActionContext<'_>,
  action: &mut T,
  var: &Var,
  span: Span,
) -> Result<(), Spanned<EvalError>> {
  let Ok(components) = var.path.val.components();
  let field = match components.as_deref() {
    Some(
      [
        PathComponentRef { index: None, .. },
        field @ PathComponentRef { index: None, .. },
      ],
    ) => *field,
    Some([] | [_] | [_, _] | [_, _, _, ..]) | None => {
      return Err(T::UNKNOWN_PARAMETER.spanned2(span));
    }
  };
  let value = match &var.expr {
    Some(operand) => ActionParameterValue::Expr(&operand.val).spanned2(operand.span),
    None => ActionParameterValue::Boolean(match var.not {
      Some(_) => BooleanValue::Off,
      None => BooleanValue::On,
    }).spanned2(span),
  };
  let Ok(meaning) = context
    .meaning_cache
    .get_case_insensitive(context.interner, field.ident);
  let Ok(parameter) = ACTION_PARAMETERS.get(meaning);
  action.handle_field(context, parameter.spanned2(var.path.span), value)
}

#[derive(Copy, Clone)]
enum ActionName {
  NoAction,
  VoidAction,
  SetMods,
  LatchMods,
  LockMods,
  SetGroup,
  LatchGroup,
  LockGroup,
  RedirectKey,
  SetControls,
  LockControls,
  Unimplemented,
}

const ACTION_NAMES: Keywords<ActionName> = Keywords {
  words: &[
    (Meaning::NoAction, ActionName::NoAction),
    (Meaning::VoidAction, ActionName::VoidAction),
    (Meaning::SetMods, ActionName::SetMods),
    (Meaning::LatchMods, ActionName::LatchMods),
    (Meaning::LockMods, ActionName::LockMods),
    (Meaning::SetGroup, ActionName::SetGroup),
    (Meaning::LatchGroup, ActionName::LatchGroup),
    (Meaning::LockGroup, ActionName::LockGroup),
    (Meaning::RedirectKey, ActionName::RedirectKey),
    (Meaning::Redirect, ActionName::RedirectKey),
    (Meaning::SetControls, ActionName::SetControls),
    (Meaning::LockControls, ActionName::LockControls),
    (Meaning::MovePtr, ActionName::Unimplemented),
    (Meaning::MovePointer, ActionName::Unimplemented),
    (Meaning::PtrBtn, ActionName::Unimplemented),
    (Meaning::PointerButton, ActionName::Unimplemented),
    (Meaning::LockPtrBtn, ActionName::Unimplemented),
    (Meaning::LockPointerBtn, ActionName::Unimplemented),
    (Meaning::LockPtrButton, ActionName::Unimplemented),
    (Meaning::LockPointerButton, ActionName::Unimplemented),
    (Meaning::SetPtrDflt, ActionName::Unimplemented),
    (Meaning::SetPointerDefault, ActionName::Unimplemented),
    (Meaning::Terminate, ActionName::Unimplemented),
    (Meaning::TerminateServer, ActionName::Unimplemented),
    (Meaning::SwitchScreen, ActionName::Unimplemented),
    (Meaning::Private, ActionName::Unimplemented),
    (Meaning::ISOLock, ActionName::Unimplemented),
    (Meaning::ActionMessage, ActionName::Unimplemented),
    (Meaning::MessageAction, ActionName::Unimplemented),
    (Meaning::Message, ActionName::Unimplemented),
    (Meaning::DeviceBtn, ActionName::Unimplemented),
    (Meaning::DevBtn, ActionName::Unimplemented),
    (Meaning::DevButton, ActionName::Unimplemented),
    (Meaning::DeviceButton, ActionName::Unimplemented),
    (Meaning::LockDeviceBtn, ActionName::Unimplemented),
    (Meaning::LockDevBtn, ActionName::Unimplemented),
    (Meaning::LockDevButton, ActionName::Unimplemented),
    (Meaning::LockDeviceButton, ActionName::Unimplemented),
    (Meaning::DeviceValuator, ActionName::Unimplemented),
    (Meaning::DevVal, ActionName::Unimplemented),
    (Meaning::DeviceVal, ActionName::Unimplemented),
    (Meaning::DevValuator, ActionName::Unimplemented),
  ],
};

fn created_action(
  context: &mut ActionContext<'_>,
  defaults: &ActionDefaults,
  name: Option<ActionName>,
  call: &Call,
) -> Result<ResolvedAction, Spanned<EvalError>> {
  let args = &call.args;

  match name {
    Some(ActionName::NoAction) => {
      create_action(context, &defaults.no_action, args).map(ResolvedAction::ResolvedNoAction)
    }
    Some(ActionName::VoidAction) => {
      create_action(context, &defaults.void_action, args).map(ResolvedAction::ResolvedVoidAction)
    }
    Some(ActionName::SetMods) => {
      create_action(context, &defaults.mods_set, args).map(ResolvedAction::ResolvedModsSet)
    }
    Some(ActionName::LatchMods) => {
      create_action(context, &defaults.mods_latch, args).map(ResolvedAction::ResolvedModsLatch)
    }
    Some(ActionName::LockMods) => {
      create_action(context, &defaults.mods_lock, args).map(ResolvedAction::ResolvedModsLock)
    }
    Some(ActionName::SetGroup) => {
      create_action(context, &defaults.group_set, args).map(ResolvedAction::ResolvedGroupSet)
    }
    Some(ActionName::LatchGroup) => {
      create_action(context, &defaults.group_latch, args).map(ResolvedAction::ResolvedGroupLatch)
    }
    Some(ActionName::LockGroup) => {
      create_action(context, &defaults.group_lock, args).map(ResolvedAction::ResolvedGroupLock)
    }
    Some(ActionName::RedirectKey) => create_action(context, &defaults.redirect_key, args)
      .map(ResolvedAction::ResolvedRedirectKey),
    Some(ActionName::SetControls) => create_action(context, &defaults.set_controls, args)
      .map(ResolvedAction::ResolvedSetControls),
    Some(ActionName::LockControls) => create_action(context, &defaults.lock_controls, args)
      .map(ResolvedAction::ResolvedLockControls),
    Some(ActionName::Unimplemented) => Err(UnimplementedAction.spanned2(call.path.span)),
    None => Err(UnknownAction.spanned2(call.path.span)),
  }
}

fn eval_action(
  context: &mut ActionContext<'_>,
  action_defaults: &ActionDefaults,
  expr: Spanned<&Expr>,
) -> Result<Spanned<ResolvedAction>, Spanned<EvalError>> {
  let Ok(called) = call_of(expr.val);
  let call = called.ok_or(UnsupportedExpressionForAction.spanned2(expr.span))?;
  let Ok(unique_ident) = call.path.val.unique_ident();
  let name = unique_ident.ok_or(UnknownAction.spanned2(call.path.span))?;
  let Ok(meaning) = context
    .meaning_cache
    .get_case_insensitive(context.interner, name);
  let Ok(action_name) = ACTION_NAMES.get(meaning);
  let action = created_action(context, action_defaults, action_name, call)?;
  Ok(action.spanned2(expr.span))
}

fn eval_action_or_no_action(
  context: &mut ActionContext<'_>,
  action_defaults: &ActionDefaults,
  expr: Spanned<&Expr>,
) -> Result<Option<Spanned<ResolvedAction>>, Spanned<EvalError>> {
  let action = eval_action(context, action_defaults, expr)?;

  Ok(match action.val {
    ResolvedAction::ResolvedNoAction(_) => None,
    ResolvedAction::ResolvedVoidAction(_)
    | ResolvedAction::ResolvedModsSet(_)
    | ResolvedAction::ResolvedModsLatch(_)
    | ResolvedAction::ResolvedModsLock(_)
    | ResolvedAction::ResolvedGroupSet(_)
    | ResolvedAction::ResolvedGroupLatch(_)
    | ResolvedAction::ResolvedGroupLock(_)
    | ResolvedAction::ResolvedRedirectKey(_)
    | ResolvedAction::ResolvedSetControls(_)
    | ResolvedAction::ResolvedLockControls(_) => Some(action),
  })
}

pub(crate) fn eval_action_default(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  keycodes: &ResolvedKeycodes,
  vmods: &Vmodmap,
  decl: &Var,
  span: Span,
  defaults: &mut ActionDefaults,
) -> Result<(), Spanned<EvalError>> {
  let Ok(first) = decl.path.val.first_ident();

  let action_name = match first {
    Some(first_ident) => {
      let Ok(meaning) = meaning_cache.get_case_insensitive(interner, first_ident);
      let Ok(named) = ACTION_NAMES.get(meaning);
      named
    }
    None => None,
  };

  let mut context = ActionContext {
    interner,
    meaning_cache,
    keycodes,
    vmods,
  };

  match action_name {
    Some(ActionName::NoAction) => {
      handle_action_global(&mut context, &mut defaults.no_action, decl, span)
    }
    Some(ActionName::VoidAction) => {
      handle_action_global(&mut context, &mut defaults.void_action, decl, span)
    }
    Some(ActionName::SetMods) => {
      handle_action_global(&mut context, &mut defaults.mods_set, decl, span)
    }
    Some(ActionName::LatchMods) => {
      handle_action_global(&mut context, &mut defaults.mods_latch, decl, span)
    }
    Some(ActionName::LockMods) => {
      handle_action_global(&mut context, &mut defaults.mods_lock, decl, span)
    }
    Some(ActionName::SetGroup) => {
      handle_action_global(&mut context, &mut defaults.group_set, decl, span)
    }
    Some(ActionName::LatchGroup) => {
      handle_action_global(&mut context, &mut defaults.group_latch, decl, span)
    }
    Some(ActionName::LockGroup) => {
      handle_action_global(&mut context, &mut defaults.group_lock, decl, span)
    }
    Some(ActionName::RedirectKey) => {
      handle_action_global(&mut context, &mut defaults.redirect_key, decl, span)
    }
    Some(ActionName::SetControls) => {
      handle_action_global(&mut context, &mut defaults.set_controls, decl, span)
    }
    Some(ActionName::LockControls) => {
      handle_action_global(&mut context, &mut defaults.lock_controls, decl, span)
    }
    Some(ActionName::Unimplemented) => Err(UnimplementedAction.spanned2(span)),
    None => Err(UnknownAction.spanned2(span)),
  }
}

#[derive(Copy, Clone)]
pub(crate) enum FieldPath {
  Bare,
  Qualified,
}

pub(crate) struct Tables<'t, 'b, 'c> {
  pub(crate) map: &'t mut CodeMap,
  pub(crate) diagnostics: &'t mut DiagnosticSink<'b, 'c>,
  pub(crate) interner: &'t mut Interner,
  pub(crate) cooker: &'t mut StringCooker,
  pub(crate) meaning_cache: &'t mut MeaningCache,
}

fn field_component<'p>(
  components: Option<&[PathComponentRef<'p>]>,
  form: FieldPath,
) -> Result<Option<PathComponentRef<'p>>, Never> {
  Ok(match (form, components) {
    (FieldPath::Qualified, Some([_, field])) | (FieldPath::Bare, Some([field])) => Some(*field),
    (FieldPath::Qualified, Some([] | [_] | [_, _, _, ..]))
    | (FieldPath::Bare, Some([] | [_, _, ..]))
    | (_, None) => None,
  })
}

fn index_of(component: PathComponentRef<'_>) -> Result<Option<Spanned<&Expr>>, Never> {
  Ok(component.index.map(|index| {
    let Ok(borrowed) = index.index.as_ref();
    borrowed
  }))
}

fn operand_of(var: &Var) -> Result<Option<Spanned<&Expr>>, Never> {
  Ok(var.expr.as_ref().map(|expr| {
    let Ok(borrowed) = expr.as_ref();
    borrowed
  }))
}

trait Element {
  type Item;

  fn gather(
    &mut self,
    map: &mut CodeMap,
    interner: &mut Interner,
    diagnostics: &mut DiagnosticSink<'_, '_>,
    expr: Spanned<&Expr>,
    elements: &mut Vec<Spanned<Self::Item>>,
  ) -> Result<(), Spanned<EvalError>>;
}

struct Actions<'e> {
  meaning_cache: &'e mut MeaningCache,
  keycodes: &'e ResolvedKeycodes,
  vmods: &'e Vmodmap,
  action_defaults: &'e ActionDefaults,
}

impl Element for Actions<'_> {
  type Item = ResolvedAction;

  fn gather(
    &mut self,
    _map: &mut CodeMap,
    interner: &mut Interner,
    _diagnostics: &mut DiagnosticSink<'_, '_>,
    expr: Spanned<&Expr>,
    elements: &mut Vec<Spanned<ResolvedAction>>,
  ) -> Result<(), Spanned<EvalError>> {
    let mut context = ActionContext {
      interner,
      meaning_cache: self.meaning_cache,
      keycodes: self.keycodes,
      vmods: self.vmods,
    };
    let action = eval_action_or_no_action(&mut context, self.action_defaults, expr)?;
    elements.extend(action);
    Ok(())
  }
}

struct Symbols<'e> {
  cooker: &'e mut StringCooker,
  meaning_cache: &'e mut MeaningCache,
}

impl Element for Symbols<'_> {
  type Item = Keysym;

  fn gather(
    &mut self,
    map: &mut CodeMap,
    interner: &mut Interner,
    diagnostics: &mut DiagnosticSink<'_, '_>,
    expr: Spanned<&Expr>,
    elements: &mut Vec<Spanned<Keysym>>,
  ) -> Result<(), Spanned<EvalError>> {
    let found = keysyms(
      map,
      diagnostics,
      interner,
      self.cooker,
      self.meaning_cache,
      expr,
    )?;
    elements.extend(
      found
        .into_iter()
        .filter(|sym| *sym != syms::NoSymbol)
        .map(|sym| sym.spanned2(expr.span)),
    );
    Ok(())
  }
}

fn gather<E: Element>(
  element: &mut E,
  map: &mut CodeMap,
  interner: &mut Interner,
  diagnostics: &mut DiagnosticSink<'_, '_>,
  expr: Spanned<&Expr>,
  elements: &mut Vec<Spanned<E::Item>>,
) -> Result<(), Never> {
  match element.gather(map, interner, diagnostics, expr, elements) {
    Ok(()) => Ok(()),
    Err(fault) => {
      let Ok(diagnostic_kind) = fault.val.diagnostic_kind();
      diagnostics.push(map, diagnostic_kind, fault)
    }
  }
}

fn eval_brace_list<E: Element>(
  expr: Spanned<&Expr>,
  map: &mut CodeMap,
  interner: &mut Interner,
  diagnostics: &mut DiagnosticSink<'_, '_>,
  element: &mut E,
) -> Result<Vec<Spanned<E::Item>>, Never> {
  let mut elements = Vec::new();
  let Ok(listed) = brace_list_of(expr.val);

  match listed {
    Some(members) => {
      elements.reserve_exact(members.len());

      for member in members {
        let Ok(borrowed) = member.as_ref();
        let Ok(()) = gather(element, map, interner, diagnostics, borrowed, &mut elements);
      }
    }
    None => {
      let Ok(()) = gather(element, map, interner, diagnostics, expr, &mut elements);
    }
  }

  Ok(elements)
}

fn eval_symbols_list<E: Element>(
  group_expr: Option<Spanned<&Expr>>,
  map: &mut CodeMap,
  interner: &mut Interner,
  diagnostics: &mut DiagnosticSink<'_, '_>,
  expr: Spanned<&Expr>,
  element: &mut E,
) -> Result<PlacedGroupList<E::Item>, Spanned<EvalError>> {
  let group = match group_expr {
    Some(index) => {
      let evaluated = eval_group(interner, index)?;
      Some(evaluated.val)
    }
    None => None,
  };
  let Ok(listed) = bracket_list_of(expr.val);
  let members = listed.ok_or(UnsupportedExpressionForSymbolsOrActions.spanned2(expr.span))?;
  let mut levels = Vec::with_capacity(members.len());

  for member in members {
    let Ok(borrowed) = member.as_ref();
    let Ok(elements) = eval_brace_list(borrowed, map, interner, diagnostics, element);
    levels.push(elements);
  }

  match levels.iter().rposition(|level| !level.is_empty()) {
    Some(last) => levels.truncate(last.saturating_add(1)),
    None => levels.clear(),
  }

  Ok((group, levels))
}

pub(crate) enum InterpField {
  Actions(Vec<Spanned<ResolvedAction>>),
  VirtualModifier(ModifierIndex),
  Repeat(bool),
  Locking(bool),
  LevelOneOnly(bool),
}

#[derive(Copy, Clone)]
enum InterpretName {
  Action,
  VirtualModifier,
  UseModMap,
  Repeat,
  Locking,
}

const INTERPRET_FIELDS: Keywords<InterpretName> = Keywords {
  words: &[
    (Meaning::Action, InterpretName::Action),
    (Meaning::Virtualmodifier, InterpretName::VirtualModifier),
    (Meaning::Virtualmod, InterpretName::VirtualModifier),
    (Meaning::Usemodmapmods, InterpretName::UseModMap),
    (Meaning::Usemodmap, InterpretName::UseModMap),
    (Meaning::Repeat, InterpretName::Repeat),
    (Meaning::Locking, InterpretName::Locking),
  ],
};
const LEVEL_ONE_ONLY: Keywords<bool> = Keywords {
  words: &[
    (Meaning::Levelone, true),
    (Meaning::Level1, true),
    (Meaning::Anylevel, false),
    (Meaning::Any, false),
  ],
};

fn interpret_virtual_modifier(
  vmods: &Vmodmap,
  operand: Spanned<&Expr>,
) -> Result<InterpField, Spanned<EvalError>> {
  let Ok(ident) = unique_ident_of(operand.val);
  let Ok(vmod) = match ident {
    Some(name) => vmods.get(name),
    None => Ok(None),
  };

  match vmod {
    Some(found) => Ok(InterpField::VirtualModifier(found.idx)),
    None => Err(UnknownInterpretVirtualModifier.spanned2(operand.span)),
  }
}

fn interpret_level_one_only(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  operand: Spanned<&Expr>,
  span: Span,
) -> Result<InterpField, Spanned<EvalError>> {
  let Ok(meaning) = meaning_of(interner, meaning_cache, operand.val);
  let Ok(spoken) = match meaning {
    Some(word) => LEVEL_ONE_ONLY.get(word),
    None => Ok(None),
  };

  match spoken {
    Some(level_one_only) => Ok(InterpField::LevelOneOnly(level_one_only)),
    None => Err(InvalidInterpretUseModMapModValue.spanned2(span)),
  }
}

pub(crate) fn eval_interp_field(
  tables: &mut Tables<'_, '_, '_>,
  keycodes: &ResolvedKeycodes,
  vmods: &Vmodmap,
  action_defaults: &ActionDefaults,
  var: &Var,
  span: Span,
  form: FieldPath,
) -> Result<Spanned<InterpField>, Spanned<EvalError>> {
  let Tables {
    map,
    diagnostics,
    interner,
    meaning_cache,
    ..
  } = tables;
  let Ok(components) = var.path.val.components();
  let Ok(component) = field_component(components.as_deref(), form);
  let field = match component {
    Some(PathComponentRef { ident, index: None }) => ident,
    Some(PathComponentRef { index: Some(_), .. }) | None => {
      return Err(UnknownInterpretField.spanned2(var.path.span));
    }
  };
  let Ok(meaning) = meaning_cache.get_case_insensitive(interner, field);
  let Ok(name) = INTERPRET_FIELDS.get(meaning);
  let Ok(operand) = operand_of(var);
  let interpreted = match name {
    Some(InterpretName::Action) => {
      let value = operand.ok_or(MissingInterpretActionValue.spanned2(span))?;
      let mut element = Actions {
        meaning_cache,
        keycodes,
        vmods,
        action_defaults,
      };
      let Ok(actions) = eval_brace_list(value, map, interner, diagnostics, &mut element);
      InterpField::Actions(actions)
    }
    Some(InterpretName::VirtualModifier) => {
      let value = operand.ok_or(MissingInterpretVirtualmodValue.spanned2(span))?;
      interpret_virtual_modifier(vmods, value)?
    }
    Some(InterpretName::UseModMap) => {
      let value = operand.ok_or(MissingInterpretUseModMapModValue.spanned2(span))?;
      interpret_level_one_only(interner, meaning_cache, value, span)?
    }
    Some(InterpretName::Repeat) => {
      let repeats = flag(interner, meaning_cache, operand, var.not)?;
      InterpField::Repeat(repeats == BooleanValue::On)
    }
    Some(InterpretName::Locking) => {
      let locks = flag(interner, meaning_cache, operand, var.not)?;
      InterpField::Locking(locks == BooleanValue::On)
    }
    None => return Err(UnknownInterpretField.spanned2(var.path.span)),
  };
  Ok(interpreted.spanned2(span))
}

pub(crate) enum TypeField {
  Modifiers(Spanned<ModifierTree>),
  Map(Spanned<ModifierTree>, Spanned<Level>),
  Preserve(Spanned<ModifierTree>, Spanned<ModifierTree>),
  LevelName(Spanned<Level>, Spanned<Interned>),
}

#[derive(Copy, Clone)]
enum TypeName {
  Modifiers,
  Map,
  Preserve,
  LevelName,
}

const TYPE_FIELDS: Keywords<TypeName> = Keywords {
  words: &[
    (Meaning::Modifiers, TypeName::Modifiers),
    (Meaning::Map, TypeName::Map),
    (Meaning::Preserve, TypeName::Preserve),
    (Meaning::LevelName, TypeName::LevelName),
    (Meaning::Levelname, TypeName::LevelName),
  ],
};

pub(crate) fn eval_type_field(
  tables: &mut Tables<'_, '_, '_>,
  mods: &Vmodmap,
  var: &Var,
  span: Span,
  form: FieldPath,
) -> Result<Spanned<TypeField>, Spanned<EvalError>> {
  let Tables {
    map,
    diagnostics,
    interner,
    cooker,
    meaning_cache,
  } = tables;
  let Ok(components) = var.path.val.components();
  let Ok(component) = field_component(components.as_deref(), form);
  let field = component.ok_or(UnknownTypeField.spanned2(var.path.span))?;
  let Ok(meaning) = meaning_cache.get_case_insensitive(interner, field.ident);
  let Ok(name) = TYPE_FIELDS.get(meaning);
  let Ok(operand) = operand_of(var);
  let Ok(indexed) = index_of(field);
  let typed = match (name, indexed) {
    (Some(TypeName::Modifiers), None) => {
      let value = operand.ok_or(MissingTypeModifiersValue.spanned2(var.path.span))?;
      let modifiers = eval_mods(interner, meaning_cache, mods, value)?;
      TypeField::Modifiers(modifiers)
    }
    (Some(TypeName::Map), Some(index)) => {
      let mask = eval_mods(interner, meaning_cache, mods, index)?;
      let value = operand.ok_or(MissingTypeMapValue.spanned2(var.path.span))?;
      let level = eval_level(interner, value)?;
      TypeField::Map(mask, level)
    }
    (Some(TypeName::Preserve), Some(index)) => {
      let mask = eval_mods(interner, meaning_cache, mods, index)?;
      let value = operand.ok_or(MissingTypePreserveValue.spanned2(var.path.span))?;
      let preserved = eval_mods(interner, meaning_cache, mods, value)?;
      TypeField::Preserve(mask, preserved)
    }
    (Some(TypeName::LevelName), Some(index)) => {
      let level = eval_level(interner, index)?;
      let value = operand.ok_or(MissingTypeLevelNameValue.spanned2(var.path.span))?;
      let level_name = eval_string(map, diagnostics, interner, cooker, value)?;
      TypeField::LevelName(level, level_name)
    }
    (Some(TypeName::Modifiers), Some(_))
    | (Some(TypeName::Map | TypeName::Preserve | TypeName::LevelName), None)
    | (None, _) => return Err(UnknownTypeField.spanned2(var.path.span)),
  };
  Ok(typed.spanned2(span))
}

pub(crate) enum IndicatorMapField {
  Modifiers(Spanned<ModifierTree>),
  Groups(Spanned<GroupMask>),
  Controls(Spanned<ControlMask>),
  WhichModifierState(Spanned<ModComponentMask>),
  WhichGroupState(Spanned<GroupComponent>),
}

#[derive(Copy, Clone)]
enum IndicatorName {
  Modifiers,
  Groups,
  Controls,
  WhichModifierState,
  WhichGroupState,
  Unimplemented,
}

const INDICATOR_FIELDS: Keywords<IndicatorName> = Keywords {
  words: &[
    (Meaning::Modifiers, IndicatorName::Modifiers),
    (Meaning::Mods, IndicatorName::Modifiers),
    (Meaning::Groups, IndicatorName::Groups),
    (Meaning::Controls, IndicatorName::Controls),
    (Meaning::Ctrls, IndicatorName::Controls),
    (
      Meaning::Whichmodifierstate,
      IndicatorName::WhichModifierState,
    ),
    (Meaning::Whichmodstate, IndicatorName::WhichModifierState),
    (Meaning::Whichgroupstate, IndicatorName::WhichGroupState),
    (Meaning::Allowexplicit, IndicatorName::Unimplemented),
    (Meaning::Indicatordriveskbd, IndicatorName::Unimplemented),
    (
      Meaning::Indicatordriveskeyboard,
      IndicatorName::Unimplemented,
    ),
  ],
};

pub(crate) fn eval_indicator_map_field(
  interner: &mut Interner,
  meaning_cache: &mut MeaningCache,
  mods: &Vmodmap,
  var: &Var,
  span: Span,
  form: FieldPath,
) -> Result<Spanned<IndicatorMapField>, Spanned<EvalError>> {
  let Ok(components) = var.path.val.components();
  let Ok(component) = field_component(components.as_deref(), form);
  let field = match component {
    Some(PathComponentRef { ident, index: None }) => ident,
    Some(PathComponentRef { index: Some(_), .. }) | None => {
      return Err(UnknownIndicatorField.spanned2(var.path.span));
    }
  };
  let Ok(meaning) = meaning_cache.get_case_insensitive(interner, field);
  let Ok(name) = INDICATOR_FIELDS.get(meaning);
  let Ok(operand) = operand_of(var);
  let indicated = match name {
    Some(IndicatorName::Modifiers) => {
      let value = operand.ok_or(MissingIndicatorModifiersValue.spanned2(span))?;
      let modifiers = eval_mods(interner, meaning_cache, mods, value)?;
      IndicatorMapField::Modifiers(modifiers)
    }
    Some(IndicatorName::Groups) => {
      let value = operand.ok_or(MissingIndicatorGroupsValue.spanned2(span))?;
      let groups = eval_group_mask(interner, meaning_cache, value)?;
      IndicatorMapField::Groups(groups)
    }
    Some(IndicatorName::Controls) => {
      let value = operand.ok_or(MissingIndicatorControlValue.spanned2(span))?;
      let controls = eval_control_mask(interner, meaning_cache, value)?;
      IndicatorMapField::Controls(controls)
    }
    Some(IndicatorName::WhichModifierState) => {
      let value = operand.ok_or(MissingIndicatorWhichModStateValue.spanned2(span))?;
      let which = eval_mod_component_mask(interner, meaning_cache, value)?;
      IndicatorMapField::WhichModifierState(which)
    }
    Some(IndicatorName::WhichGroupState) => {
      let value = operand.ok_or(MissingIndicatorWhichGroupStateValue.spanned2(span))?;
      let which = eval_group_component(interner, meaning_cache, value)?;
      IndicatorMapField::WhichGroupState(which)
    }
    Some(IndicatorName::Unimplemented) => {
      return Err(UnimplementedIndicatorField.spanned2(var.path.span));
    }
    None => return Err(UnknownIndicatorField.spanned2(var.path.span)),
  };
  Ok(indicated.spanned2(span))
}

pub(crate) fn eval_mod_map_field<M>(
  tables: &mut Tables<'_, '_, '_>,
  keycodes: &ResolvedKeycodes,
  expr: &Expr,
  span: Span,
  handed: &mut M,
  handler: impl Fn(&mut M, Spanned<ModMapField>),
) -> Result<(), Spanned<EvalError>> {
  let Tables {
    map,
    diagnostics,
    interner,
    cooker,
    meaning_cache,
  } = tables;
  let Ok(key_name) = key_name_of(expr);

  match key_name {
    Some(name) => match keycodes.name_to_key.get(&name) {
      Some(key) => {
        let code = match key.kind {
          ResolvedKeyKind::Real(real) => real.val,
          ResolvedKeyKind::Alias(_, aliased, _) => aliased,
        };
        handler(handed, ModMapField::Keycode(code).spanned2(span));
        Ok(())
      }
      None => Err(UnknownKeycode.spanned2(span)),
    },
    None => {
      let found = keysyms(
        map,
        diagnostics,
        interner,
        cooker,
        meaning_cache,
        expr.spanned2(span),
      )?;

      for sym in found {
        handler(handed, ModMapField::Keysym(sym, None).spanned2(span));
      }

      Ok(())
    }
  }
}

pub(crate) enum KeyRepeat {
  LayoutDefault,
  Enabled,
  Disabled,
}

pub(crate) enum SymbolsField {
  GroupKeyType(GroupIdx, KeyTypeRef),
  DefaultKeyType(KeyTypeRef),
  Symbols((Option<GroupIdx>, GroupList<Keysym>)),
  Actions((Option<GroupIdx>, GroupList<ResolvedAction>)),
  Virtualmodifiers(ModifierMask),
  Repeating(KeyRepeat),
  Locks(bool),
  Groupswrap,
  Groupsclamp,
  Groupsredirect(GroupIdx),
  Overlay((KeyOverlay, Interned, Keycode)),
  AllowNone(bool, u32),
  RadioGroup(Option<RadioGroup>),
}

pub(crate) type GroupList<T> = Vec<Vec<Spanned<T>>>;

type PlacedGroupList<T> = (Option<GroupIdx>, GroupList<T>);

#[derive(Copy, Clone)]
enum KeyFieldName {
  Type,
  Symbols,
  Actions,
  VirtualModifiers,
  Repeating,
  Locks,
  GroupsWrap,
  GroupsClamp,
  GroupsRedirect,
  Overlay(KeyOverlay),
  AllowNone,
  RadioGroup,
}

const KEY_FIELDS: Keywords<KeyFieldName> = Keywords {
  words: &[
    (Meaning::Type, KeyFieldName::Type),
    (Meaning::Symbols, KeyFieldName::Symbols),
    (Meaning::Actions, KeyFieldName::Actions),
    (Meaning::Vmods, KeyFieldName::VirtualModifiers),
    (Meaning::Virtualmods, KeyFieldName::VirtualModifiers),
    (Meaning::Virtualmodifiers, KeyFieldName::VirtualModifiers),
    (Meaning::Repeating, KeyFieldName::Repeating),
    (Meaning::Repeats, KeyFieldName::Repeating),
    (Meaning::Repeat, KeyFieldName::Repeating),
    (Meaning::Locking, KeyFieldName::Locks),
    (Meaning::Lock, KeyFieldName::Locks),
    (Meaning::Locks, KeyFieldName::Locks),
    (Meaning::Groupswrap, KeyFieldName::GroupsWrap),
    (Meaning::Wrapgroups, KeyFieldName::GroupsWrap),
    (Meaning::Groupsclamp, KeyFieldName::GroupsClamp),
    (Meaning::Clampgroups, KeyFieldName::GroupsClamp),
    (Meaning::Groupsredirect, KeyFieldName::GroupsRedirect),
    (Meaning::Redirectgroups, KeyFieldName::GroupsRedirect),
    (
      Meaning::Overlay,
      KeyFieldName::Overlay(KeyOverlay::Overlay1),
    ),
    (
      Meaning::Overlay1,
      KeyFieldName::Overlay(KeyOverlay::Overlay1),
    ),
    (
      Meaning::Overlay2,
      KeyFieldName::Overlay(KeyOverlay::Overlay2),
    ),
    (Meaning::Allownone, KeyFieldName::AllowNone),
    (Meaning::Radiogroup, KeyFieldName::RadioGroup),
  ],
};
const BUILT_IN_KEY_TYPES: Keywords<BuiltInKeytype> = Keywords {
  words: &[
    (Meaning::ONE_LEVEL, BuiltInKeytype::OneLevel),
    (Meaning::ALPHABETIC, BuiltInKeytype::Alphabetic),
    (Meaning::KEYPAD, BuiltInKeytype::Keypad),
    (Meaning::TWO_LEVEL, BuiltInKeytype::TwoLevel),
    (
      Meaning::FOUR_LEVEL_ALPHABETIC,
      BuiltInKeytype::FourLevelAlphabetic,
    ),
    (
      Meaning::FOUR_LEVEL_SEMIALPHABETIC,
      BuiltInKeytype::FourLevelSemialphabetic,
    ),
    (Meaning::FOUR_LEVEL_KEYPAD, BuiltInKeytype::FourLevelKeypad),
    (Meaning::FOUR_LEVEL, BuiltInKeytype::FourLevel),
  ],
};

struct KeyField<'k> {
  name: Option<KeyFieldName>,
  index: Option<Spanned<&'k Expr>>,
}

fn unnamed_key_field(expr: Option<Spanned<&Expr>>) -> Result<KeyFieldName, Never> {
  let first_level = match expr {
    Some(value) => {
      let Ok(levels) = bracket_list_of(value.val);
      levels.and_then(|members| members.first())
    }
    None => None,
  };
  let first_element = match first_level {
    Some(level) => {
      let Ok(elements) = brace_list_of(&level.val);

      match elements {
        Some(members) => members.first(),
        None => Some(level),
      }
    }
    None => None,
  };
  let Ok(call) = match first_element {
    Some(element) => call_of(&element.val),
    None => Ok(None),
  };

  Ok(match call {
    Some(_) => KeyFieldName::Actions,
    None => KeyFieldName::Symbols,
  })
}

fn key_field<'k>(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  path: Option<Spanned<&'k Path>>,
  expr: Option<Spanned<&Expr>>,
  form: FieldPath,
) -> Result<KeyField<'k>, Spanned<EvalError>> {
  match path {
    Some(named) => {
      let Ok(components) = named.val.components();
      let Ok(component) = field_component(components.as_deref(), form);
      let field = component.ok_or(UnknownKeyField.spanned2(named.span))?;
      let Ok(meaning) = meaning_cache.get_case_insensitive(interner, field.ident);
      let Ok(name) = KEY_FIELDS.get(meaning);
      let Ok(index) = index_of(field);
      Ok(KeyField { name, index })
    }
    None => {
      let Ok(name) = unnamed_key_field(expr);
      Ok(KeyField {
        name: Some(name),
        index: None,
      })
    }
  }
}

fn unindexed(field: &KeyField<'_>, span: Span) -> Result<(), Spanned<EvalError>> {
  match field.index {
    Some(_) => Err(UnknownKeyField.spanned2(span)),
    None => Ok(()),
  }
}

fn key_type_named(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  key_types: &ResolvedTypes,
  name: Spanned<Interned>,
) -> Result<KeyTypeRef, Spanned<EvalError>> {
  let Ok(meaning) = meaning_cache.get_case_sensitive(interner, name.val);
  let Ok(built_in) = BUILT_IN_KEY_TYPES.get(meaning);

  match built_in {
    Some(kind) => Ok(KeyTypeRef::BuiltIn(kind)),
    None => match ::core::primitive::bool::then_some(key_types.key_types.contains_key(&name.val), ()) {
      ::core::option::Option::Some(()) => Ok(KeyTypeRef::Named(name)),
      ::core::option::Option::None => Err(UnknownKeyType.spanned2(name.span)),
    },
  }
}

fn key_repeats(
  interner: &Interner,
  meaning_cache: &mut MeaningCache,
  expr: Option<Spanned<&Expr>>,
  not: Option<Span>,
) -> Result<KeyRepeat, Spanned<EvalError>> {
  match expr {
    Some(value) => {
      let name = eval_interned(value, UnsupportedExpressionForKeyRepeats)?;
      let Ok(meaning) = meaning_cache.get_case_insensitive(interner, name.val);
      let Ok(spoken) = BOOLEANS.get(meaning);

      match (spoken, meaning) {
        (Some(BooleanValue::On), _) => Ok(KeyRepeat::Enabled),
        (Some(BooleanValue::Off), _) => Ok(KeyRepeat::Disabled),
        (None, Meaning::Default) => Ok(KeyRepeat::LayoutDefault),
        (None, _) => Err(UnknownKeyRepeatingValue.spanned2(name.span)),
      }
    }
    None => Ok(match not {
      Some(_) => KeyRepeat::Disabled,
      None => KeyRepeat::Enabled,
    }),
  }
}

#[derive(Copy, Clone)]
pub(crate) struct KeyVar<'v> {
  pub(crate) not: Option<Span>,
  pub(crate) path: Option<Spanned<&'v Path>>,
  pub(crate) expr: Option<Spanned<&'v Expr>>,
}

pub(crate) fn eval_symbols_field(
  tables: &mut Tables<'_, '_, '_>,
  action_defaults: &ActionDefaults,
  keycodes: &ResolvedKeycodes,
  mods: &Vmodmap,
  key_types: &ResolvedTypes,
  var: Spanned<KeyVar<'_>>,
  form: FieldPath,
) -> Result<Spanned<SymbolsField>, Spanned<EvalError>> {
  let Tables {
    map,
    diagnostics,
    interner,
    cooker,
    meaning_cache,
  } = tables;
  let span = var.span;
  let KeyVar { not, path, expr } = var.val;
  let field = key_field(interner, meaning_cache, path, expr, form)?;
  let symbols = match field.name {
    Some(KeyFieldName::Type) => {
      let value = expr.ok_or(MissingKeyTypeValue.spanned2(span))?;
      let name = eval_string(map, diagnostics, interner, cooker, value)?;
      let key_type = key_type_named(interner, meaning_cache, key_types, name)?;

      match field.index {
        None => SymbolsField::DefaultKeyType(key_type),
        Some(index) => {
          let group = eval_group(interner, index)?;
          SymbolsField::GroupKeyType(group.val, key_type)
        }
      }
    }
    Some(KeyFieldName::Symbols) => {
      let value = expr.ok_or(MissingKeySymbolsValue.spanned2(span))?;
      let mut element = Symbols {
        cooker,
        meaning_cache,
      };
      let listed =
        eval_symbols_list(field.index, map, interner, diagnostics, value, &mut element)?;
      SymbolsField::Symbols(listed)
    }
    Some(KeyFieldName::Actions) => {
      let value = expr.ok_or(MissingKeyActionsValue.spanned2(span))?;
      let mut element = Actions {
        meaning_cache,
        keycodes,
        vmods: mods,
        action_defaults,
      };
      let listed =
        eval_symbols_list(field.index, map, interner, diagnostics, value, &mut element)?;
      SymbolsField::Actions(listed)
    }
    Some(KeyFieldName::VirtualModifiers) => {
      unindexed(&field, span)?;
      let value = expr.ok_or(MissingKeyVirtualModifiersValue.spanned2(span))?;
      let virtual_modifiers = eval_virtual_mods(mods, value)?;
      SymbolsField::Virtualmodifiers(virtual_modifiers.val)
    }
    Some(KeyFieldName::Repeating) => {
      unindexed(&field, span)?;
      let repeating = key_repeats(interner, meaning_cache, expr, not)?;
      SymbolsField::Repeating(repeating)
    }
    Some(KeyFieldName::Locks) => {
      unindexed(&field, span)?;
      let locks = flag(interner, meaning_cache, expr, not)?;
      SymbolsField::Locks(locks == BooleanValue::On)
    }
    Some(KeyFieldName::GroupsWrap) => {
      unindexed(&field, span)?;
      let wraps = flag(interner, meaning_cache, expr, not)?;

      match wraps {
        BooleanValue::On => SymbolsField::Groupswrap,
        BooleanValue::Off => SymbolsField::Groupsclamp,
      }
    }
    Some(KeyFieldName::GroupsClamp) => {
      unindexed(&field, span)?;
      let clamps = flag(interner, meaning_cache, expr, not)?;

      match clamps {
        BooleanValue::On => SymbolsField::Groupsclamp,
        BooleanValue::Off => SymbolsField::Groupswrap,
      }
    }
    Some(KeyFieldName::GroupsRedirect) => {
      unindexed(&field, span)?;
      let value = expr.ok_or(MissingKeyGroupsRedirectValue.spanned2(span))?;
      let group = eval_group(interner, value)?;
      SymbolsField::Groupsredirect(group.val)
    }
    Some(KeyFieldName::Overlay(overlay)) => {
      unindexed(&field, span)?;
      let value = expr.ok_or(MissingKeyOverlayValue.spanned2(span))?;
      let key = eval_keycode(keycodes, value)?;
      let (name, code) = key.val;
      SymbolsField::Overlay((overlay, name, code))
    }
    Some(KeyFieldName::AllowNone) => {
      let groups = match field.index {
        None => !0,
        Some(index) => {
          let radio_group = eval_radio_group(interner, index)?;
          let Ok(mask) = radio_group.val.to_mask();
          mask
        }
      };
      let allowed = flag(interner, meaning_cache, expr, not)?;
      SymbolsField::AllowNone(allowed == BooleanValue::On, groups)
    }
    Some(KeyFieldName::RadioGroup) => {
      let value = expr.ok_or(MissingKeyRadiogroupValue.spanned2(span))?;
      let Ok(meaning) = meaning_of(interner, meaning_cache, value.val);

      match ::core::primitive::bool::then_some(meaning == Some(Meaning::None), ()) {
        ::core::option::Option::Some(()) => SymbolsField::RadioGroup(None),
        ::core::option::Option::None => {
          let evaluated = eval_radio_group_or_0(interner, value)?;
          SymbolsField::RadioGroup(evaluated.val)
        }
      }
    }
    None => return Err(UnknownKeyField.spanned2(span)),
  };
  Ok(symbols.spanned2(span))
}
