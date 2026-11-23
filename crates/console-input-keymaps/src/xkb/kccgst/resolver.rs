use crate::Keycode;
use crate::Keysym;
use crate::Lettercase;
use crate::ModifierIndex;
use crate::ModifierMask;
use crate::Region;
use crate::modifier::Presence;
use crate::syms;
use crate::xkb::code_map::CodeMap;
use crate::xkb::diagnostic::DiagnosticKind;
use crate::xkb::diagnostic::DiagnosticSink;
use crate::xkb::group::GroupIdx;
use crate::xkb::indicator::IndicatorIdx;
use crate::xkb::interner::Interned;
use crate::xkb::interner::Interner;
use crate::xkb::kccgst::MergeMode;
use crate::xkb::kccgst::ast::CompatmapDecl;
use crate::xkb::kccgst::ast::Component;
use crate::xkb::kccgst::ast::ConfigItemType;
use crate::xkb::kccgst::ast::Decl;
use crate::xkb::kccgst::ast::Decls;
use crate::xkb::kccgst::ast::DirectOrIncluded;
use crate::xkb::kccgst::ast::Expr;
use crate::xkb::kccgst::ast::IndicatorMapDecl;
use crate::xkb::kccgst::ast::IndicatorNameDecl;
use crate::xkb::kccgst::ast::InterpretDecl;
use crate::xkb::kccgst::ast::InterpretSym;
use crate::xkb::kccgst::ast::Item;
use crate::xkb::kccgst::ast::ItemType;
use crate::xkb::kccgst::ast::KeySymbolsDecl;
use crate::xkb::kccgst::ast::KeyTypeDecl;
use crate::xkb::kccgst::ast::KeycodeDecl;
use crate::xkb::kccgst::ast::ModMapDecl;
use crate::xkb::kccgst::ast::Path;
use crate::xkb::kccgst::ast::PathIndex;
use crate::xkb::kccgst::ast::SymbolsDecl;
use crate::xkb::kccgst::ast::TypesDecl;
use crate::xkb::kccgst::ast::VModDecl;
use crate::xkb::kccgst::ast::VarDecl;
use crate::xkb::kccgst::ast::VarOrExpr;
use crate::xkb::kccgst::expr::EvalError;
use crate::xkb::kccgst::expr::FieldPath;
use crate::xkb::kccgst::expr::GroupList;
use crate::xkb::kccgst::expr::IndicatorMapField;
use crate::xkb::kccgst::expr::InterpField;
use crate::xkb::kccgst::expr::KeyVar;
use crate::xkb::kccgst::expr::SymbolsField;
use crate::xkb::kccgst::expr::Tables;
use crate::xkb::kccgst::expr::TypeField;
use crate::xkb::kccgst::expr::eval_action_default;
use crate::xkb::kccgst::expr::eval_filter;
use crate::xkb::kccgst::expr::eval_group;
use crate::xkb::kccgst::expr::eval_indicator_map_field;
use crate::xkb::kccgst::expr::eval_interp_field;
use crate::xkb::kccgst::expr::eval_keysyms;
use crate::xkb::kccgst::expr::eval_mod_map_field;
use crate::xkb::kccgst::expr::eval_real_mods;
use crate::xkb::kccgst::expr::eval_string;
use crate::xkb::kccgst::expr::eval_symbols_field;
use crate::xkb::kccgst::expr::eval_type_field;
use crate::xkb::kccgst::expr::ident_to_real_mod_index;
use crate::xkb::level::Level;
use crate::xkb::meaning::Meaning;
use crate::xkb::meaning::MeaningCache;
use crate::xkb::modmap::Vmodmap;
use crate::xkb::resolved::BuiltInKeytype;
use crate::xkb::resolved::Filter;
use crate::xkb::resolved::GroupsRedirect;
use crate::xkb::resolved::Indicator;
use crate::xkb::resolved::IndicatorMap;
use crate::xkb::resolved::IndicatorMapWithKey;
use crate::xkb::resolved::IndicatorSlot;
use crate::xkb::resolved::Interp;
use crate::xkb::resolved::InterpWithKey;
use crate::xkb::resolved::KeyTypeRef;
use crate::xkb::resolved::ModMapEntryWithKey;
use crate::xkb::resolved::ModMapField;
use crate::xkb::resolved::Predicate;
use crate::xkb::resolved::Reachability;
use crate::xkb::resolved::Resolved;
use crate::xkb::resolved::ResolvedAction;
use crate::xkb::resolved::ResolvedCompat;
use crate::xkb::resolved::ResolvedKey;
use crate::xkb::resolved::ResolvedKeyKind;
use crate::xkb::resolved::ResolvedKeyType;
use crate::xkb::resolved::ResolvedKeyTypeWithName;
use crate::xkb::resolved::ResolvedKeycodes;
use crate::xkb::resolved::ResolvedSymbols;
use crate::xkb::resolved::ResolvedTypes;
use crate::xkb::resolved::SymbolsKey;
use crate::xkb::resolved::SymbolsKeyBehavior;
use crate::xkb::resolved::SymbolsKeyGroup;
use crate::xkb::resolved::SymbolsKeyLevel;
use crate::xkb::resolved::SymbolsKeyWithKey;
use crate::xkb::resolved::Written;
use crate::xkb::span::Span;
use crate::xkb::span::SpanExt;
use crate::xkb::span::Spanned;
use crate::xkb::string_cooker::StringCooker;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
use console_input_keymaps_derive::ad_hoc_display;
use hashbrown::DefaultHashBuilder;
use hashbrown::HashMap;
use hashbrown::HashSet;
use hashbrown::hash_map::Entry;
use indexmap::IndexMap;
use std::fmt::Display;
use std::mem;
use std::slice;

pub(crate) fn resolve(
  map: &mut CodeMap,
  diagnostics: &mut DiagnosticSink<'_, '_>,
  interner: &mut Interner,
  meaning_cache: &mut MeaningCache,
  cooker: &mut StringCooker,
  item: &Item,
) -> Result<Resolved, Never> {
  let mut resolver = Resolver {
    map,
    interner,
    meaning_cache,
    diagnostics,
    cooker,
  };

  let Ok(mods) = VmodResolver {
    r: &mut resolver,
    data: Default::default(),
  }
  .resolve(item);

  let Ok(mut keycodes) = KeycodesResolver {
    r: &mut resolver,
    data: Default::default(),
  }
  .resolve(item);
  let Ok(()) = fix_resolved_keycodes(&mut resolver, &mut keycodes);

  let Ok(mut types) = TypesResolver {
    r: &mut resolver,
    mods: &mods,
    data: Default::default(),
  }
  .resolve(item);
  let Ok(()) = fix_resolved_types(&mut types);

  let Ok(mut compat) = CompatResolver {
    r: &mut resolver,
    keycodes: &keycodes,
    mods: &mods,
    data: Default::default(),
  }
  .resolve(item);
  let Ok(()) = fix_resolved_compat(&mut resolver, &mut keycodes, &mut compat);

  let Ok(mut symbols) = SymbolsResolver {
    r: &mut resolver,
    mods: &mods,
    keycodes: &keycodes,
    types: &types,
    data: Default::default(),
  }
  .resolve(item);
  let Ok(()) = fix_resolved_symbols(&mut resolver, &types, &mut symbols);

  let Ok(()) = fix_combined_properties(&mods, &mut types, &compat, &mut symbols);
  let name = match &item.ty {
    ItemType::Composite(c) => match c.name {
      Some(given) => {
        let Ok(cooked) = resolver.cook(given);

        Some(cooked)
      }
      None => None,
    },
    ItemType::Configuration(_) => None,
  };

  Ok(Resolved {
    name,
    mods,
    keycodes,
    types,
    compat,
    symbols,
  })
}

struct Resolver<'a, 'b, 'c> {
  map: &'a mut CodeMap,
  interner: &'a mut Interner,
  meaning_cache: &'a mut MeaningCache,
  diagnostics: &'a mut DiagnosticSink<'b, 'c>,
  cooker: &'a mut StringCooker,
}

struct VmodResolver<'a, 'b, 'c, 'd> {
  r: &'a mut Resolver<'b, 'c, 'd>,
  data: Vmodmap,
}

struct KeycodesResolver<'a, 'b, 'c, 'd> {
  r: &'a mut Resolver<'b, 'c, 'd>,
  data: ResolvedKeycodes,
}

struct TypesResolver<'a, 'b, 'c, 'd> {
  r: &'a mut Resolver<'b, 'c, 'd>,
  mods: &'a Vmodmap,
  data: ResolvedTypes,
}

struct CompatResolver<'a, 'b, 'c, 'd> {
  r: &'a mut Resolver<'b, 'c, 'd>,
  keycodes: &'a ResolvedKeycodes,
  mods: &'a Vmodmap,
  data: ResolvedCompat,
}

struct SymbolsResolver<'a, 'b, 'c, 'd> {
  r: &'a mut Resolver<'b, 'c, 'd>,
  mods: &'a Vmodmap,
  keycodes: &'a ResolvedKeycodes,
  types: &'a ResolvedTypes,
  data: ResolvedSymbols,
}

fn fix_resolved_keycodes(
  r: &mut Resolver<'_, '_, '_>,
  keycodes: &mut ResolvedKeycodes,
) -> Result<(), Never> {
  let mut res = ResolvedKeycodes {
    keycode_to_name: mem::take(&mut keycodes.keycode_to_name),
    indicators: mem::take(&mut keycodes.indicators),
    ..Default::default()
  };
  let rounds = Aliasing {
    pending: &mut keycodes.name_to_key,
    resolved: &mut res.name_to_key,
    ready: vec![],
  };

  for () in rounds {}

  for key in keycodes.name_to_key.values() {
    match &key.kind {
      ResolvedKeyKind::Real(_) => {}
      ResolvedKeyKind::Alias(a, _, _) => {
        let Ok(()) = r.diag(
          DiagnosticKind::UnknownKeyAlias,
          ad_hoc_display!("ignoring unknown key alias").spanned2(a.span),
        );
      }
    }
  }

  *keycodes = res;

  Ok(())
}

struct Aliasing<'a> {
  pending: &'a mut HashMap<Interned, ResolvedKey>,
  resolved: &'a mut HashMap<Interned, ResolvedKey>,
  ready: Vec<(Interned, ResolvedKey)>,
}

impl Iterator for Aliasing<'_> {
  type Item = ();

  fn next(&mut self) -> Option<()> {
    let resolved = &*self.resolved;
    self
      .ready
      .extend(self.pending.extract_if(|_, v| match v.kind {
        ResolvedKeyKind::Real(_) => true,
        ResolvedKeyKind::Alias(a, _, _) => resolved.contains_key(&a.val),
      }));

    match ::core::primitive::bool::then_some(self.ready.is_empty(), ()) {
      ::core::option::Option::Some(()) => None,
      ::core::option::Option::None => {
        for (name, key) in self.ready.drain(..) {
          let Ok(aliased) = aliased(self.resolved, key);

          match aliased {
            Some(placed) => {
              self.resolved.insert(name, placed);
            }
            None => {}
          }
        }

        Some(())
      }
    }
  }
}

fn aliased(
  resolved: &HashMap<Interned, ResolvedKey>,
  key: ResolvedKey,
) -> Result<Option<ResolvedKey>, Never> {
  Ok(match key.kind {
    ResolvedKeyKind::Real(_) => Some(key),
    ResolvedKeyKind::Alias(alias, _, _) => match resolved.get(&alias.val) {
      Some(target) => {
        let (real_name, code) = match target.kind {
          ResolvedKeyKind::Real(code) => (None, code.val),
          ResolvedKeyKind::Alias(_, code, Some(real)) => (Some(real), code),
          ResolvedKeyKind::Alias(target_alias, code, None) => (Some(target_alias), code),
        };

        Some(ResolvedKey {
          name: key.name,
          kind: ResolvedKeyKind::Alias(alias, code, real_name),
        })
      }
      None => None,
    },
  })
}

fn fix_resolved_types(types: &mut ResolvedTypes) -> Result<(), Never> {
  for ty in types.key_types.values_mut() {
    let highest = ty
      .ty
      .map
      .iter()
      .map(|(_, level)| {
        let Ok(raw) = level.val.raw();
        raw
      })
      .max();
    ty.ty.num_levels = match highest {
      Some(raw) => raw,
      None => 0,
    };
  }

  Ok(())
}

fn fix_resolved_compat(
  resolver: &mut Resolver<'_, '_, '_>,
  keycodes: &mut ResolvedKeycodes,
  compat: &mut ResolvedCompat,
) -> Result<(), Never> {
  compat
    .interps_sorted
    .extend(compat.interps.drain(..).map(|(_, v)| v));
  compat.interps_sorted.sort_by_key(|i| {
    let k1 = match i.keysym {
      Some(_) => 0,
      None => 1,
    };
    let k2 = match i.filter {
      None => 4,
      Some(f) => match f.val.predicate {
        Predicate::Exactly => 0,
        Predicate::AllOf => 1,
        Predicate::NoneOf => 2,
        Predicate::AnyOf => 3,
        Predicate::AnyOfOrNone => 4,
      },
    };
    (k1, k2)
  });
  let mut used = 0_u32;

  for indicator in &keycodes.indicators {
    match compat.indicator_maps.get_mut(&indicator.name.val) {
      Some(i) => {
        let slot = match indicator.virt {
          Some(_) => IndicatorSlot::Virtual(indicator.idx.val),
          None => IndicatorSlot::Physical(indicator.idx.val),
        };
        i.indicator_map.slot = Some(slot);
      }
      None => {}
    }

    let Ok(bit) = indicator_bit(indicator.idx.val);
    used |= bit;
  }

  for indicator in compat.indicator_maps.values_mut() {
    match indicator.indicator_map.slot {
      Some(_) => {}
      None => {
        let Ok(free) = IndicatorIdx::new((!used).trailing_zeros().saturating_add(1));

        match free {
          Some(idx) => {
            let Ok(bit) = indicator_bit(idx);
            used |= bit;
            indicator.indicator_map.slot = Some(IndicatorSlot::Virtual(idx));
          }
          None => {
            let Ok(()) = resolver.diag(
              DiagnosticKind::TooManyIndicators,
              ad_hoc_display!("ignoring more than 32 indicators").spanned2(indicator.name.span),
            );
          }
        }
      }
    }
  }

  compat
    .indicator_maps
    .retain(|_, indicator| indicator.indicator_map.slot.is_some());

  Ok(())
}

fn indicator_bit(idx: IndicatorIdx) -> Result<u32, Never> {
  let Ok(raw) = idx.raw();

  Ok(match 1_u32.checked_shl(raw.saturating_sub(1)) {
    Some(bit) => bit,
    None => 0,
  })
}

fn fix_resolved_symbols(
  resolver: &mut Resolver,
  types: &ResolvedTypes,
  symbols: &mut ResolvedSymbols,
) -> Result<(), Never> {
  for key in symbols.keys.values_mut() {
    let default_key_type = key.key.default_key_type;

    for group in &mut key.key.groups {
      let Ok(levels) = reachable_levels(resolver.interner, types, default_key_type, group);
      group.reachable_levels = levels;
    }
  }

  let mut mod_map_keys = HashSet::new();

  for entry in symbols.mod_map_entries.values() {
    match entry.modifier {
      Some(_) => match entry.key.val {
        ModMapField::Keycode(keycode) => {
          mod_map_keys.insert(keycode);
        }
        ModMapField::Keysym(_, _) => {}
      },
      None => {}
    }
  }

  let Ok(first_with) = first_with_each(symbols);

  for entry in symbols.mod_map_entries.values_mut() {
    match entry.modifier {
      Some(modifier) => {
        let Ok(keycode) = mod_map_keycode(&first_with, &mut mod_map_keys, &mut entry.key.val);
        let Ok(()) = apply_modmap(&mut symbols.keys, keycode, modifier);
      }
      None => {}
    }
  }

  Ok(())
}

fn apply_modmap(
  keys: &mut HashMap<Keycode, SymbolsKeyWithKey>,
  keycode: Option<Keycode>,
  modifier: Spanned<ModifierIndex>,
) -> Result<(), Never> {
  match keycode {
    Some(kc) => match keys.get_mut(&kc) {
      Some(key) => {
        let Ok(to_mask) = modifier.val.to_mask();
        key.key.modmap |= to_mask;
      }
      None => {}
    },
    None => {}
  }

  Ok(())
}

fn reachable_levels(
  interner: &Interner,
  types: &ResolvedTypes,
  default_key_type: Option<Spanned<KeyTypeRef>>,
  group: &mut SymbolsKeyGroup,
) -> Result<u32, Never> {
  let key_type = match group.key_type {
    Some(chosen) => chosen,
    None => match default_key_type {
      Some(given) => given.val,
      None => {
        let Ok(inferred) = infer_key_type(group);
        KeyTypeRef::BuiltIn(inferred)
      }
    },
  };
  group.key_type = Some(key_type);

  Ok(match key_type {
    KeyTypeRef::BuiltIn(built_in) => {
      let Ok(levels) = built_in_levels(interner, types, built_in);
      levels
    }
    KeyTypeRef::Named(name) => match types.key_types.get(&name.val) {
      Some(named) => named.ty.num_levels,
      None => 0,
    },
  })
}

fn built_in_levels(
  interner: &Interner,
  types: &ResolvedTypes,
  built_in: BuiltInKeytype,
) -> Result<u32, Never> {
  let least = match built_in {
    BuiltInKeytype::OneLevel => 1,
    BuiltInKeytype::Alphabetic | BuiltInKeytype::Keypad | BuiltInKeytype::TwoLevel => 2,
    BuiltInKeytype::FourLevelAlphabetic
    | BuiltInKeytype::FourLevelSemialphabetic
    | BuiltInKeytype::FourLevelKeypad
    | BuiltInKeytype::FourLevel => 4,
  };
  let Ok(name) = built_in.name();
  let Ok(existing) = interner.get_existing(name.as_bytes());
  let defined = match existing {
    Some(interned) => match types.key_types.get(&interned) {
      Some(named) => named.ty.num_levels,
      None => 0,
    },
    None => 0,
  };

  Ok(least.max(defined))
}

#[derive(Copy, Clone, PartialEq, PartialOrd)]
struct Placed {
  group: u32,
  level: u32,
  code: Keycode,
}

fn first_with_each(symbols: &ResolvedSymbols) -> Result<HashMap<Keysym, Placed>, Never> {
  let mut first_with = HashMap::new();

  for key in symbols.keys.values() {
    for (group, group_at) in key.key.groups.iter().zip(0_u32..) {
      let reachable = group
        .levels
        .iter()
        .zip(0_u32..)
        .take_while(|(_, at)| *at < group.reachable_levels);

      for (level, level_at) in reachable {
        let placed = Placed {
          group: group_at,
          level: level_at,
          code: key.code,
        };

        for symbol in &level.symbols {
          let Ok(()) = offer(&mut first_with, symbol.val, placed);
        }
      }
    }
  }

  Ok(first_with)
}

fn offer(
  first_with: &mut HashMap<Keysym, Placed>,
  keysym: Keysym,
  placed: Placed,
) -> Result<(), Never> {
  match first_with.entry(keysym) {
    Entry::Vacant(vacant) => {
      vacant.insert(placed);
    }
    Entry::Occupied(mut occupied) => {
      match ::core::primitive::bool::then_some(placed < *occupied.get(), ()) {
        ::core::option::Option::Some(()) => {
          occupied.insert(placed);
        }
        ::core::option::Option::None => {}
      }
    }
  }

  Ok(())
}

fn mod_map_keycode(
  first_with: &HashMap<Keysym, Placed>,
  mod_map_keys: &mut HashSet<Keycode>,
  field: &mut ModMapField,
) -> Result<Option<Keycode>, Never> {
  Ok(match *field {
    ModMapField::Keycode(code) => Some(code),
    ModMapField::Keysym(keysym, _) => {
      let found = first_with.get(&keysym).map(|first| first.code);
      *field = ModMapField::Keysym(keysym, found);

      match found {
        Some(code) => match ::core::primitive::bool::then_some(mod_map_keys.insert(code), ()) {
          ::core::option::Option::Some(()) => {
            *field = ModMapField::Keycode(code);
          }
          ::core::option::Option::None => {}
        },
        None => {}
      }

      found
    }
  })
}

fn infer_key_type(group: &SymbolsKeyGroup) -> Result<BuiltInKeytype, Never> {
  let mut firsts = group
    .levels
    .iter()
    .map(|level| match level.symbols.first() {
      Some(sym) => sym.val,
      None => syms::NoSymbol,
    });
  let first = firsts.next();
  let second = firsts.next();
  let third = firsts.next();
  let fourth = firsts.next();
  let beyond_four = group
    .levels
    .iter()
    .skip(4)
    .any(|level| !level.symbols.is_empty());

  match first {
    None => Ok(BuiltInKeytype::OneLevel),
    Some(s0) => match second {
      None => Ok(BuiltInKeytype::OneLevel),
      Some(s1) => match ::core::primitive::bool::then_some(beyond_four, ()) {
        ::core::option::Option::Some(()) => Ok(BuiltInKeytype::OneLevel),
        ::core::option::Option::None => match third {
          None => two_level(s0, s1),
          Some(s2) => four_level(s0, s1, s2, fourth),
        },
      },
    },
  }
}

fn two_level(s0: Keysym, s1: Keysym) -> Result<BuiltInKeytype, Never> {
  let Ok(casing) = cased(s0, s1);
  let Ok(region) = keypad(s0, s1);

  Ok(match casing {
    Casing::Cased => BuiltInKeytype::Alphabetic,
    Casing::Uncased => match region {
      Region::Keypad => BuiltInKeytype::Keypad,
      Region::Main => BuiltInKeytype::TwoLevel,
    },
  })
}

fn four_level(
  s0: Keysym,
  s1: Keysym,
  s2: Keysym,
  fourth: Option<Keysym>,
) -> Result<BuiltInKeytype, Never> {
  let Ok(casing) = cased(s0, s1);
  let Ok(region) = keypad(s0, s1);
  let upper_casing = match fourth {
    Some(s3) => {
      let Ok(upper) = cased(s2, s3);
      upper
    }
    None => Casing::Uncased,
  };

  Ok(match casing {
    Casing::Cased => match upper_casing {
      Casing::Cased => BuiltInKeytype::FourLevelAlphabetic,
      Casing::Uncased => BuiltInKeytype::FourLevelSemialphabetic,
    },
    Casing::Uncased => match region {
      Region::Keypad => BuiltInKeytype::FourLevelKeypad,
      Region::Main => BuiltInKeytype::FourLevel,
    },
  })
}

#[derive(Copy, Clone)]
enum Casing {
  Cased,
  Uncased,
}

fn cased(lower: Keysym, upper: Keysym) -> Result<Casing, Never> {
  let Ok(first) = lower.lettercase();
  let Ok(second) = upper.lettercase();

  Ok(match (first, second) {
    (Lettercase::Lower, Lettercase::Upper) => Casing::Cased,
    (Lettercase::Lower, Lettercase::Lower | Lettercase::Uncased)
    | (
      Lettercase::Upper | Lettercase::Uncased,
      Lettercase::Lower | Lettercase::Upper | Lettercase::Uncased,
    ) => Casing::Uncased,
  })
}

fn keypad(one: Keysym, other: Keysym) -> Result<Region, Never> {
  let Ok(first) = one.region();
  let Ok(second) = other.region();

  Ok(match (first, second) {
    (Region::Keypad, Region::Keypad | Region::Main) | (Region::Main, Region::Keypad) => {
      Region::Keypad
    }
    (Region::Main, Region::Main) => Region::Main,
  })
}

fn fix_combined_properties(
  mods: &Vmodmap,
  types: &mut ResolvedTypes,
  compat: &ResolvedCompat,
  symbols: &mut ResolvedSymbols,
) -> Result<(), Never> {
  let Ok(interps) = Interps::of(&compat.interps_sorted);

  for key_with_key in symbols.keys.values_mut() {
    let Ok(()) = interpret_key(&interps, key_with_key);
    let Ok(()) = share_modmap(mods, &key_with_key.key);
  }

  for ty in types.key_types.values_mut() {
    let Ok(()) = effective_map(&mut ty.ty, mods);
  }

  Ok(())
}

struct Interps<'a> {
  by_keysym: HashMap<Keysym, Vec<(u32, &'a InterpWithKey)>>,
  any_keysym: Vec<(u32, &'a InterpWithKey)>,
}

impl<'a> Interps<'a> {
  fn of(sorted: &'a [InterpWithKey]) -> Result<Self, Never> {
    let mut by_keysym: HashMap<Keysym, Vec<(u32, &'a InterpWithKey)>> = HashMap::new();
    let mut any_keysym = vec![];

    for (interp, at) in sorted.iter().zip(0_u32..) {
      match interp.keysym {
        Some(keysym) => by_keysym.entry(keysym.val).or_default().push((at, interp)),
        None => any_keysym.push((at, interp)),
      }
    }

    Ok(Self {
      by_keysym,
      any_keysym,
    })
  }

  fn first(
    &self,
    keysym: Keysym,
    modmap: ModifierMask,
    level_at: u32,
    used: &HashSet<u32>,
  ) -> Result<Option<(u32, &'a InterpWithKey)>, Never> {
    let specific = match self.by_keysym.get(&keysym) {
      Some(specific) => specific.as_slice(),
      None => &[],
    };

    Ok(
      specific
        .iter()
        .chain(&self.any_keysym)
        .find(|(at, interp)| {
          let Ok(fit) = filter_fits(interp, modmap, level_at);

          match fit {
            Fit::Fitting => !used.contains(at),
            Fit::Unfitting => false,
          }
        })
        .copied(),
    )
  }
}

#[derive(Copy, Clone)]
enum Fit {
  Fitting,
  Unfitting,
}

fn filter_fits(
  interp: &InterpWithKey,
  modmap: ModifierMask,
  level_at: u32,
) -> Result<Fit, Never> {
  let filter = match &interp.filter {
    None => return Ok(Fit::Fitting),
    Some(filter) => filter,
  };
  let Ok(reach) = interp.interp.reach();
  let held = match reach {
    Reachability::LevelOne => match ::core::primitive::bool::then_some(level_at != 0, ()) {
      ::core::option::Option::Some(()) => 0,
      ::core::option::Option::None => modmap.0,
    },
    Reachability::EveryLevel => modmap.0,
  };
  let wanted = filter.val.mask.val.0;

  let fitting = match filter.val.predicate {
    Predicate::NoneOf => wanted & held == 0,
    Predicate::AnyOfOrNone => held == 0 || wanted & held != 0,
    Predicate::AnyOf => wanted & held != 0,
    Predicate::AllOf => wanted & held == wanted,
    Predicate::Exactly => wanted == held,
  };

  Ok(match ::core::primitive::bool::then_some(fitting, ()) {
    ::core::option::Option::Some(()) => Fit::Fitting,
    ::core::option::Option::None => Fit::Unfitting,
  })
}

#[derive(Copy, Clone)]
struct At {
  group: u32,
  level: u32,
}

#[derive(Copy, Clone)]
enum Slot {
  First,
  Later,
}

impl At {
  fn slot(self) -> Result<Slot, Never> {
    Ok(
      match ::core::primitive::bool::then_some(self.group == 0 && self.level == 0, ()) {
        ::core::option::Option::Some(()) => Slot::First,
        ::core::option::Option::None => Slot::Later,
      },
    )
  }
}

struct Interpreted<'k> {
  repeating: &'k mut Option<Spanned<Option<bool>>>,
  behavior: &'k mut Option<Spanned<SymbolsKeyBehavior>>,
  modmap: ModifierMask,
  name: Span,
  virtual_modifiers: Option<ModifierMask>,
}

fn interpret_key(
  interps: &Interps<'_>,
  key_with_key: &mut SymbolsKeyWithKey,
) -> Result<(), Never> {
  let key = &mut key_with_key.key;
  let mut interpreted = Interpreted {
    repeating: &mut key.repeating,
    behavior: &mut key.behavior,
    modmap: key.modmap,
    name: key_with_key.name.span,
    virtual_modifiers: None,
  };

  for (group, group_at) in key.groups.iter_mut().zip(0_u32..) {
    match group.actions_written {
      Written::Explicitly => {}
      Written::Implicitly => {
        for (level, level_at) in group.levels.iter_mut().zip(0_u32..) {
          let at = At {
            group: group_at,
            level: level_at,
          };
          let Ok(()) = interpreted.level(interps, at, level);
        }
      }
    }
  }

  let found = interpreted.virtual_modifiers;

  match key.virtualmodifiers {
    Some(_) => {}
    None => {
      key.virtualmodifiers = found.map(|m| m.spanned2(key_with_key.name.span));
    }
  }

  Ok(())
}

impl Interpreted<'_> {
  fn level(
    &mut self,
    interps: &Interps<'_>,
    at: At,
    level: &mut SymbolsKeyLevel,
  ) -> Result<(), Never> {
    let mut used = HashSet::new();

    for sym in &level.symbols {
      let Ok(found) = interps.first(sym.val, self.modmap, at.level, &used);

      match found {
        None => {
          let Ok(()) = self.repeat(at, true.spanned2(self.name));
        }
        Some((position, interp)) => {
          used.insert(position);
          let Ok(()) = self.apply(at, &interp.interp, &mut level.actions);
        }
      }
    }

    Ok(())
  }

  fn apply(
    &mut self,
    at: At,
    interp: &Interp,
    actions: &mut Vec<Spanned<ResolvedAction>>,
  ) -> Result<(), Never> {
    match &interp.actions {
      Some(a) => {
        actions.extend(a.val.iter().cloned());
      }
      None => {}
    }

    let Ok(slot) = at.slot();
    let Ok(reach) = interp.reach();

    match (slot, reach) {
      (Slot::First, Reachability::LevelOne | Reachability::EveryLevel) | (Slot::Later, Reachability::EveryLevel) => {
        match interp.virtual_modifier {
          Some(modifier) => {
            let Ok(to_mask) = modifier.val.to_mask();
            *self.virtual_modifiers.get_or_insert_default() |= to_mask;
          }
          None => {}
        }
      }
      (Slot::Later, Reachability::LevelOne) => {}
    }

    match interp.repeat {
      Some(repeat) => {
        let Ok(()) = self.repeat(at, repeat);
      }
      None => {}
    }

    match interp.locking {
      Some(locks) => match (slot, self.behavior.is_none()) {
        (Slot::First, true) => {
          let Ok(behavior) = locks.map(SymbolsKeyBehavior::Locks);
          *self.behavior = Some(behavior);
        }
        (Slot::First, false) | (Slot::Later, true | false) => {}
      },
      None => {}
    }

    Ok(())
  }

  fn repeat(&mut self, at: At, repeat: Spanned<bool>) -> Result<(), Never> {
    let Ok(slot) = at.slot();

    match (slot, self.repeating.is_none()) {
      (Slot::First, true) => {
        let Ok(repeating) = repeat.map(Some);
        *self.repeating = Some(repeating);
      }
      (Slot::First, false) | (Slot::Later, true | false) => {}
    }

    Ok(())
  }
}

fn share_modmap(mods: &Vmodmap, key: &SymbolsKey) -> Result<(), Never> {
  match key.virtualmodifiers {
    Some(mm) => {
      for m in mods {
        let Ok(modifier_mask) = m.idx.to_mask();
        let Ok(presence) = mm.val.contains(modifier_mask);

        match presence {
          Presence::Present => {
            let mut mask = m.def.get();
            mask.get_or_insert(key.modmap.spanned2(m.name.span)).val |= key.modmap;
            m.def.set(mask);
          }
          Presence::Absent => {}
        }
      }
    }
    None => {}
  }

  Ok(())
}

fn effective_map(ty: &mut ResolvedKeyType, mods: &Vmodmap) -> Result<(), Never> {
  let mut map_effective = IndexMap::with_hasher(DefaultHashBuilder::default());
  let mut preserve_effective = HashMap::new();

  for (mask, lvl) in &ty.map {
    match mask.try_get_effective(mods) {
      Ok(v) => {
        map_effective.insert(v, Some(*lvl));
      }
      Err(v) => {
        map_effective.entry(v).or_insert(None);
      }
    }
  }

  for (mask, preserve) in &ty.preserved {
    let Ok(effective) = mask.get_effective(mods);

    match map_effective.entry(effective) {
      indexmap::map::Entry::Vacant(v) => {
        v.insert(Some(Level::ONE.spanned2(preserve.span)));
      }
      indexmap::map::Entry::Occupied(_) => {}
    }

    preserve_effective.insert(effective, preserve);
  }

  for (mask, val) in &map_effective {
    match val {
      Some(level) => {
        let preserve = preserve_effective.get(mask).map(|v| {
          let Ok(effective) = v.val.get_effective(mods);
          effective.spanned2(v.span)
        });
        ty.map_preserve.push((*mask, *level, preserve));
      }
      None => {}
    }
  }

  drop(preserve_effective);
  ty.map.clear();
  ty.preserved.clear();

  Ok(())
}

impl<'b, 'c> Resolver<'_, 'b, 'c> {
  fn tables(&mut self) -> Result<Tables<'_, 'b, 'c>, Never> {
    Ok(Tables {
      map: self.map,
      diagnostics: self.diagnostics,
      interner: self.interner,
      cooker: self.cooker,
      meaning_cache: self.meaning_cache,
    })
  }

  fn diag(
    &mut self,
    kind: DiagnosticKind,
    message: Spanned<impl Display + Send + Sync + 'static>,
  ) -> Result<(), Never> {
    let Ok(()) = self.diagnostics.push(self.map, kind, message);

    Ok(())
  }

  fn cook(&mut self, interned: Spanned<Interned>) -> Result<Spanned<Interned>, Never> {
    let Ok(cooked) = self
      .cooker
      .cook(self.map, self.diagnostics, self.interner, interned);
    Ok(cooked.spanned2(interned.span))
  }

  fn reported<T>(&mut self, res: Result<T, Spanned<EvalError>>) -> Result<Option<T>, Never> {
    Ok(match res {
      Ok(value) => Some(value),
      Err(fault) => {
        let Ok(diagnostic_kind) = fault.val.diagnostic_kind();
        let Ok(()) = self.diag(diagnostic_kind, fault);
        None
      }
    })
  }

  fn report(&mut self, res: Result<(), Spanned<EvalError>>) -> Result<(), Never> {
    match res {
      Ok(()) => Ok(()),
      Err(fault) => {
        let Ok(diagnostic_kind) = fault.val.diagnostic_kind();
        self.diag(diagnostic_kind, fault)
      }
    }
  }
}

#[derive(Copy, Clone)]
enum Merging {
  Augment,
  Override,
}

#[derive(Copy, Clone)]
enum Redefinition {
  Ignored,
  Applied,
}

#[derive(Copy, Clone)]
enum Merged {
  Replaced,
  Retained,
}

impl Merging {
  fn of(mm: Option<MergeMode>) -> Result<Self, Never> {
    Ok(match mm {
      Some(MergeMode::Augment) => Merging::Augment,
      Some(
        MergeMode::Include | MergeMode::Override | MergeMode::Replace | MergeMode::Alternate,
      )
      | None => Merging::Override,
    })
  }

  fn redefines(self, earlier: Option<Span>) -> Result<Redefinition, Never> {
    Ok(match self {
      Merging::Override => Redefinition::Applied,
      Merging::Augment => match earlier {
        Some(_) => Redefinition::Ignored,
        None => Redefinition::Applied,
      },
    })
  }
}

fn redefined<T>(
  redefinition: Redefinition,
  old: &mut Option<Spanned<T>>,
  new: Option<Spanned<T>>,
) -> Result<Option<Span>, Never> {
  Ok(match new {
    None => None,
    Some(given) => match redefinition {
      Redefinition::Ignored => Some(given.span),
      Redefinition::Applied => {
        *old = Some(given);
        None
      }
    },
  })
}

fn merged<T>(
  merging: Merging,
  old: &mut Option<Spanned<T>>,
  new: Option<Spanned<T>>,
) -> Result<Option<Span>, Never> {
  let Ok(redefinition) = merging.redefines(old.as_ref().map(|earlier| earlier.span));
  redefined(redefinition, old, new)
}

trait Walker<T> {
  type Data: Default;

  fn data(&mut self) -> &mut Self::Data;

  fn merge_child(
    &mut self,
    mm: Option<MergeMode>,
    group: Option<GroupIdx>,
    data: Self::Data,
  ) -> Result<(), Never>;

  fn handle_decl(&mut self, mm: Option<MergeMode>, decl: &T, span: Span) -> Result<(), Never>;
}

trait ConfigWalker<T>: Walker<T> + Sized {
  fn accept(config: &ConfigItemType) -> Result<Option<&Decls<T>>, Never>;

  fn resolve(mut self, item: &Item) -> Result<Self::Data, Never> {
    match &item.ty {
      ItemType::Composite(composite) => {
        for nested in &composite.config_items {
          let Ok(()) = self.resolve_config_item(&nested.val.item.specific);
        }
      }
      ItemType::Configuration(config) => {
        let Ok(()) = self.resolve_config_item(config);
      }
    }

    Ok(mem::take(self.data()))
  }

  fn resolve_config_item(&mut self, config: &ConfigItemType) -> Result<(), Never> {
    let Ok(accepted) = Self::accept(config);

    match accepted {
      Some(decls) => walk(self, decls),
      None => Ok(()),
    }
  }
}

fn walk<T, W: Walker<T>>(walker: &mut W, decls: &Decls<T>) -> Result<(), Never> {
  let walking = Walk {
    walker,
    root: decls.decls.iter(),
    open: vec![],
  };

  for () in walking {}

  Ok(())
}

struct Walk<'a, 'w, T, W: Walker<T>> {
  walker: &'w mut W,
  root: slice::Iter<'a, Spanned<Decl<T>>>,
  open: Vec<Open<'a, T, W::Data>>,
}

struct Open<'a, T, D> {
  walking: Walking<'a, T>,
  merge_mode: Option<MergeMode>,
  group: Option<GroupIdx>,
  parent: D,
}

enum Walking<'a, T> {
  Components(slice::Iter<'a, Component<T>>),
  Decls(slice::Iter<'a, Spanned<Decl<T>>>),
}

enum Entered<'a, T> {
  Decl(&'a Spanned<Decl<T>>),
  Component(&'a Component<T>),
}

impl<'a, T> Walking<'a, T> {
  fn entered(&mut self) -> Result<Option<Entered<'a, T>>, Never> {
    Ok(match self {
      Walking::Components(components) => components.next().map(Entered::Component),
      Walking::Decls(decls) => decls.next().map(Entered::Decl),
    })
  }
}

impl<T, W: Walker<T>> Iterator for Walk<'_, '_, T, W> {
  type Item = ();

  fn next(&mut self) -> Option<()> {
    match self.open.pop() {
      None => {
        let decl = self.root.next()?;
        let Ok(()) = self.enter_decl(decl);

        Some(())
      }
      Some(mut top) => {
        let Ok(entered) = top.walking.entered();

        match entered {
          Some(Entered::Decl(decl)) => {
            self.open.push(top);
            let Ok(()) = self.enter_decl(decl);
          }
          Some(Entered::Component(component)) => {
            self.open.push(top);
            let Ok(()) = self.enter_component(component);
          }
          None => {
            let Ok(()) = self.close(top);
          }
        }

        Some(())
      }
    }
  }
}

impl<'a, T, W: Walker<T>> Walk<'a, '_, T, W> {
  fn enter_decl(&mut self, decl: &'a Spanned<Decl<T>>) -> Result<(), Never> {
    let mm = decl.val.merge_mode.map(|m| m.val);

    match &decl.val.ty {
      DirectOrIncluded::Direct(direct) => self.walker.handle_decl(mm, direct, decl.span),
      DirectOrIncluded::Included(included) => {
        self.open.push(Open {
          walking: Walking::Components(included.components.iter()),
          merge_mode: mm,
          group: None,
          parent: mem::take(self.walker.data()),
        });

        Ok(())
      }
    }
  }

  fn enter_component(&mut self, component: &'a Component<T>) -> Result<(), Never> {
    self.open.push(Open {
      walking: Walking::Decls(component.decls.decls.iter()),
      merge_mode: component.mm.map(|m| m.val),
      group: component.group.map(|g| g.group),
      parent: mem::take(self.walker.data()),
    });

    Ok(())
  }

  fn close(&mut self, closed: Open<'a, T, W::Data>) -> Result<(), Never> {
    let child = mem::replace(self.walker.data(), closed.parent);
    self
      .walker
      .merge_child(closed.merge_mode, closed.group, child)
  }
}

impl VmodResolver<'_, '_, '_, '_> {
  fn resolve(mut self, item: &Item) -> Result<Vmodmap, Never> {
    match &item.ty {
      ItemType::Composite(composite) => {
        for nested in &composite.config_items {
          let Ok(()) = self.resolve_config_item(&nested.val.item.specific);
        }
      }
      ItemType::Configuration(config) => {
        let Ok(()) = self.resolve_config_item(config);
      }
    }

    Ok(self.data)
  }

  fn resolve_config_item(&mut self, config: &ConfigItemType) -> Result<(), Never> {
    match config {
      ConfigItemType::Types(t) => walk(self, &t.decls),
      ConfigItemType::Compat(t) => walk(self, &t.decls),
      ConfigItemType::Symbols(t) => walk(self, &t.decls),
      ConfigItemType::Keycodes(_) | ConfigItemType::Geometry(_) => Ok(()),
    }
  }

  fn merge_vmods(&mut self, mm: Option<MergeMode>, child: Vmodmap) -> Result<(), Never> {
    for vmod in child {
      let Ok(()) = self.handle_vmod(mm, vmod.name, vmod.def.get());
    }

    Ok(())
  }

  fn handle_vmod_decl(&mut self, mm: Option<MergeMode>, decl: &VModDecl) -> Result<(), Never> {
    for def in &decl.defs {
      let Ok(real_mod_index) =
        ident_to_real_mod_index(self.r.interner, self.r.meaning_cache, def.val.name.val);

      match real_mod_index {
        Some(_) => {
          let Ok(()) = self.r.diag(
            DiagnosticKind::VirtualModifierHasRealName,
            ad_hoc_display!("virtual modifier has real-modifier name")
              .spanned2(def.val.name.span),
          );
        }
        None => {
          let Ok(()) = self.define_vmod(mm, def.val.name, def.val.val.as_ref());
        }
      }
    }

    Ok(())
  }

  fn define_vmod(
    &mut self,
    mm: Option<MergeMode>,
    name: Spanned<Interned>,
    value: Option<&Spanned<Expr>>,
  ) -> Result<(), Never> {
    match value {
      None => self.handle_vmod(mm, name, None),
      Some(expr) => {
        let Ok(as_ref) = expr.as_ref();
        let res = eval_real_mods(self.r.interner, self.r.meaning_cache, as_ref);
        let Ok(evaluated) = self.r.reported(res);

        match evaluated {
          Some(mask) => self.handle_vmod(mm, name, Some(mask)),
          None => Ok(()),
        }
      }
    }
  }

  fn handle_vmod(
    &mut self,
    mm: Option<MergeMode>,
    name: Spanned<Interned>,
    value: Option<Spanned<ModifierMask>>,
  ) -> Result<(), Never> {
    let Ok(inserted) = self.data.insert(name);
    let vmod = match inserted {
      Some(vmod) => vmod,
      None => {
        let Ok(()) = self.r.diag(
          DiagnosticKind::TooManyVirtualModifiers,
          ad_hoc_display!("too many virtual modifiers").spanned2(name.span),
        );
        return Ok(());
      }
    };

    let Ok(merging) = Merging::of(mm);
    let clashing = vmod.def.get().is_some() && vmod.def.get() != value;

    match (merging, clashing) {
      (Merging::Override, true | false) | (Merging::Augment, false) => match value {
        ::core::option::Option::Some(_) => {
          vmod.def.set(value);
        }
        ::core::option::Option::None => {}
      },
      (Merging::Augment, true) => {
        let Ok(()) = self.r.diag(
          DiagnosticKind::IgnoringVmodRedefinition,
          ad_hoc_display!("ignoring redefinition of virtual modifier").spanned2(name.span),
        );
      }
    }

    Ok(())
  }
}

impl Walker<TypesDecl> for VmodResolver<'_, '_, '_, '_> {
  type Data = Vmodmap;

  fn data(&mut self) -> &mut Vmodmap {
    &mut self.data
  }

  fn merge_child(
    &mut self,
    mm: Option<MergeMode>,
    _group: Option<GroupIdx>,
    data: Vmodmap,
  ) -> Result<(), Never> {
    self.merge_vmods(mm, data)
  }

  fn handle_decl(
    &mut self,
    mm: Option<MergeMode>,
    decl: &TypesDecl,
    _span: Span,
  ) -> Result<(), Never> {
    match decl {
      TypesDecl::VMod(vmod) => self.handle_vmod_decl(mm, vmod),
      TypesDecl::KeyType(_) | TypesDecl::Var(_) | TypesDecl::Include(_) => Ok(()),
    }
  }
}

impl Walker<CompatmapDecl> for VmodResolver<'_, '_, '_, '_> {
  type Data = Vmodmap;

  fn data(&mut self) -> &mut Vmodmap {
    &mut self.data
  }

  fn merge_child(
    &mut self,
    mm: Option<MergeMode>,
    _group: Option<GroupIdx>,
    data: Vmodmap,
  ) -> Result<(), Never> {
    self.merge_vmods(mm, data)
  }

  fn handle_decl(
    &mut self,
    mm: Option<MergeMode>,
    decl: &CompatmapDecl,
    _span: Span,
  ) -> Result<(), Never> {
    match decl {
      CompatmapDecl::VMod(vmod) => self.handle_vmod_decl(mm, vmod),
      CompatmapDecl::Interpret(_)
      | CompatmapDecl::IndicatorMap(_)
      | CompatmapDecl::Var(_)
      | CompatmapDecl::GroupCompat(_)
      | CompatmapDecl::Include(_) => Ok(()),
    }
  }
}

impl Walker<SymbolsDecl> for VmodResolver<'_, '_, '_, '_> {
  type Data = Vmodmap;

  fn data(&mut self) -> &mut Vmodmap {
    &mut self.data
  }

  fn merge_child(
    &mut self,
    mm: Option<MergeMode>,
    _group: Option<GroupIdx>,
    data: Vmodmap,
  ) -> Result<(), Never> {
    self.merge_vmods(mm, data)
  }

  fn handle_decl(
    &mut self,
    mm: Option<MergeMode>,
    decl: &SymbolsDecl,
    _span: Span,
  ) -> Result<(), Never> {
    match decl {
      SymbolsDecl::VMod(vmod) => self.handle_vmod_decl(mm, vmod),
      SymbolsDecl::Symbols(_)
      | SymbolsDecl::ModMap(_)
      | SymbolsDecl::Var(_)
      | SymbolsDecl::Include(_) => Ok(()),
    }
  }
}

impl KeycodesResolver<'_, '_, '_, '_> {
  fn handle_key(
    &mut self,
    mm: Option<MergeMode>,
    name: Spanned<Interned>,
    ty: ResolvedKeyKind,
  ) -> Result<(), Never> {
    let code = match ty {
      ResolvedKeyKind::Real(real) => Some(real.val),
      ResolvedKeyKind::Alias(..) => None,
    };
    let code_taken = match code {
      Some(given) => self.data.keycode_to_name.contains_key(&given),
      None => false,
    };
    let name_taken = self.data.name_to_key.contains_key(&name.val);
    let Ok(merging) = Merging::of(mm);

    match (merging, code_taken || name_taken) {
      (Merging::Augment, true) => {}
      (Merging::Augment, false) | (Merging::Override, true | false) => {
        let Ok(()) = self.forget_key(name.val, code);

        match code {
          Some(given) => {
            self.data.keycode_to_name.insert(given, name.val);
          }
          None => {}
        }

        self
          .data
          .name_to_key
          .insert(name.val, ResolvedKey { name, kind: ty });
      }
    }

    Ok(())
  }

  fn forget_key(&mut self, name: Interned, code: Option<Keycode>) -> Result<(), Never> {
    let code_named = match self.data.name_to_key.remove(&name) {
      Some(ResolvedKey {
        kind: ResolvedKeyKind::Real(real),
        ..
      }) => Some(real.val),
      Some(ResolvedKey {
        kind: ResolvedKeyKind::Alias(..),
        ..
      })
      | None => None,
    };
    let name_coded = match code {
      Some(given) => self.data.keycode_to_name.remove(&given),
      None => None,
    };

    match code_named {
      Some(named) => {
        self.data.keycode_to_name.remove(&named);
      }
      None => {}
    }

    match name_coded {
      Some(coded) => {
        self.data.name_to_key.remove(&coded);
      }
      None => {}
    }

    Ok(())
  }

  fn handle_indicator_name(
    &mut self,
    mm: Option<MergeMode>,
    name: Spanned<Interned>,
    idx: Spanned<IndicatorIdx>,
    virt: Option<Span>,
  ) -> Result<(), Never> {
    let Ok(merging) = Merging::of(mm);
    let indicators = &mut self.data.indicators;
    let mut clashing = (0_u32..)
      .zip(indicators.iter_mut())
      .filter(|(_, old)| old.idx == idx || old.name == name);

    match clashing.next() {
      None => {
        indicators.push(Indicator { idx, name, virt });
      }
      Some((_, first)) => match merging {
        Merging::Augment => {}
        Merging::Override => {
          *first = Indicator { idx, name, virt };
          let second = clashing.next().map(|(at, _)| at);

          match second {
            Some(at) => {
              let Ok(()) = swap_out(indicators, at);
            }
            None => {}
          }
        }
      },
    }

    Ok(())
  }

  fn handle_var(&mut self, decl: &VarDecl, span: Span) -> Result<(), Never> {
    let Ok(unique_ident) = decl.var.path.val.unique_ident();
    let bound = match unique_ident {
      Some(ident) => match ::core::primitive::bool::then_some(decl.var.expr.is_some(), ()) {
        ::core::option::Option::Some(()) => {
          let Ok(meaning) = self
            .r
            .meaning_cache
            .get_case_insensitive(self.r.interner, ident);

          matches!(meaning, Meaning::Minimum | Meaning::Maximum)
        }
        ::core::option::Option::None => false,
      },
      None => false,
    };

    match ::core::primitive::bool::then_some(bound, ()) {
      ::core::option::Option::Some(()) => Ok(()),
      ::core::option::Option::None => self.r.diag(
        DiagnosticKind::UnknownKeycodesVariable,
        ad_hoc_display!("unknown keycodes variable").spanned2(span),
      ),
    }
  }

  fn handle_indicator_decl(
    &mut self,
    mm: Option<MergeMode>,
    decl: &IndicatorNameDecl,
  ) -> Result<(), Never> {
    let Ok(indicator_idx) = IndicatorIdx::new(decl.idx);

    match indicator_idx {
      None => self.r.diag(
        DiagnosticKind::InvalidIndicatorIndex,
        ad_hoc_display!("invalid indicator index").spanned2(decl.idx_name.span),
      ),
      Some(idx) => {
        let Ok(as_ref) = decl.val.as_ref();
        let res = eval_string(
          self.r.map,
          self.r.diagnostics,
          self.r.interner,
          self.r.cooker,
          as_ref,
        );
        let Ok(evaluated) = self.r.reported(res);

        match evaluated {
          Some(name) => {
            self.handle_indicator_name(mm, name, idx.spanned2(decl.idx_name.span), decl.virt)
          }
          None => Ok(()),
        }
      }
    }
  }
}

fn swap_out(indicators: &mut Vec<Indicator>, at: u32) -> Result<(), Never> {
  let Ok(position) = index(at);

  match indicators.pop() {
    Some(last) => match indicators.get_mut(position) {
      Some(slot) => {
        *slot = last;
      }
      None => {}
    },
    None => {}
  }

  Ok(())
}

impl Walker<KeycodeDecl> for KeycodesResolver<'_, '_, '_, '_> {
  type Data = ResolvedKeycodes;

  fn data(&mut self) -> &mut ResolvedKeycodes {
    &mut self.data
  }

  fn merge_child(
    &mut self,
    mm: Option<MergeMode>,
    _group: Option<GroupIdx>,
    data: ResolvedKeycodes,
  ) -> Result<(), Never> {
    for indicator in data.indicators {
      let Ok(()) = self.handle_indicator_name(mm, indicator.name, indicator.idx, indicator.virt);
    }

    for key in data.name_to_key.into_values() {
      let Ok(()) = self.handle_key(mm, key.name, key.kind);
    }

    Ok(())
  }

  fn handle_decl(
    &mut self,
    mm: Option<MergeMode>,
    decl: &KeycodeDecl,
    span: Span,
  ) -> Result<(), Never> {
    match decl {
      KeycodeDecl::KeyName(e) => self.handle_key(
        mm,
        e.name,
        ResolvedKeyKind::Real(e.code.spanned2(e.code_str.span)),
      ),
      KeycodeDecl::KeyAlias(e) => self.handle_key(
        mm,
        e.name,
        ResolvedKeyKind::Alias(e.alias_for, Keycode(0), None),
      ),
      KeycodeDecl::Var(e) => self.handle_var(e, span),
      KeycodeDecl::IndicatorName(e) => self.handle_indicator_decl(mm, e),
      KeycodeDecl::Include(_) => Ok(()),
    }
  }
}

impl ConfigWalker<KeycodeDecl> for KeycodesResolver<'_, '_, '_, '_> {
  fn accept(config: &ConfigItemType) -> Result<Option<&Decls<KeycodeDecl>>, Never> {
    Ok(match config {
      ConfigItemType::Keycodes(c) => Some(&c.decls),
      ConfigItemType::Types(_)
      | ConfigItemType::Compat(_)
      | ConfigItemType::Symbols(_)
      | ConfigItemType::Geometry(_) => None,
    })
  }
}

impl ResolvedKeyType {
  fn apply_field(&mut self, field: TypeField) -> Result<(), Never> {
    match field {
      TypeField::Modifiers(m) => {
        self.modifiers = Some(m);
      }
      TypeField::Map(k, v) => {
        self.map.push((k.val, v));
      }
      TypeField::Preserve(k, v) => {
        self.preserved.push((k.val, v));
      }
      TypeField::LevelName(n, v) => {
        self.names.insert(n.val, v);
      }
    }

    Ok(())
  }
}

impl TypesResolver<'_, '_, '_, '_> {
  fn parse_field(
    &mut self,
    decl: &VarDecl,
    span: Span,
    path: FieldPath,
  ) -> Result<Option<TypeField>, Never> {
    let Ok(mut tables) = self.r.tables();
    let res = eval_type_field(&mut tables, self.mods, &decl.var, span, path);
    let Ok(parsed) = self.r.reported(res);

    Ok(parsed.map(|field| field.val))
  }

  fn handle_type(
    &mut self,
    mm: Option<MergeMode>,
    name: Spanned<Interned>,
    ty: ResolvedKeyType,
  ) -> Result<(), Never> {
    let Ok(merging) = Merging::of(mm);
    let entry = self.data.key_types.entry(name.val);

    match (merging, matches!(entry, Entry::Occupied(_))) {
      (Merging::Augment, true) => {
        let Ok(()) = self.r.diag(
          DiagnosticKind::DuplicateKeyTypeDefinition,
          ad_hoc_display!("ignoring duplicate key type name").spanned2(name.span),
        );
        return Ok(());
      }
      (Merging::Augment, false) | (Merging::Override, true | false) => {}
    }

    entry.insert(ResolvedKeyTypeWithName { name, ty });

    Ok(())
  }

  fn handle_key_type(&mut self, mm: Option<MergeMode>, decl: &KeyTypeDecl) -> Result<(), Never> {
    let Ok(name) = self.r.cook(decl.name);
    let mut key = self.data.default.clone();

    for field_decl in &decl.decls {
      let Ok(parsed) = self.parse_field(&field_decl.val, field_decl.span, FieldPath::Bare);

      match parsed {
        Some(field) => {
          let Ok(()) = key.apply_field(field);
        }
        None => {}
      }
    }

    self.handle_type(mm, name, key)
  }

  fn handle_default(&mut self, decl: &VarDecl, span: Span) -> Result<(), Never> {
    let Ok(components) = decl.var.path.val.components();
    let first = match &components {
      Some(cs) => cs.first(),
      None => None,
    };
    let names_types = match first {
      Some(c) => {
        let Ok(meaning) = self
          .r
          .meaning_cache
          .get_case_insensitive(self.r.interner, c.ident);
        c.index.is_none() && meaning == Meaning::Type
      }
      None => false,
    };

    match ::core::primitive::bool::then_some(names_types, ()) {
      ::core::option::Option::None => self.r.diag(
        DiagnosticKind::UnknownTypesVariable,
        ad_hoc_display!("unknown types variable").spanned2(span),
      ),
      ::core::option::Option::Some(()) => {
        let Ok(parsed) = self.parse_field(decl, span, FieldPath::Qualified);

        match parsed {
          Some(field) => self.data.default.apply_field(field),
          None => Ok(()),
        }
      }
    }
  }
}

impl Walker<TypesDecl> for TypesResolver<'_, '_, '_, '_> {
  type Data = ResolvedTypes;

  fn data(&mut self) -> &mut ResolvedTypes {
    &mut self.data
  }

  fn merge_child(
    &mut self,
    mm: Option<MergeMode>,
    _group: Option<GroupIdx>,
    data: ResolvedTypes,
  ) -> Result<(), Never> {
    for key in data.key_types.into_values() {
      let Ok(()) = self.handle_type(mm, key.name, key.ty);
    }

    Ok(())
  }

  fn handle_decl(
    &mut self,
    mm: Option<MergeMode>,
    decl: &TypesDecl,
    span: Span,
  ) -> Result<(), Never> {
    match decl {
      TypesDecl::KeyType(key_type) => self.handle_key_type(mm, key_type),
      TypesDecl::Var(var) => self.handle_default(var, span),
      TypesDecl::VMod(_) | TypesDecl::Include(_) => Ok(()),
    }
  }
}

impl ConfigWalker<TypesDecl> for TypesResolver<'_, '_, '_, '_> {
  fn accept(config: &ConfigItemType) -> Result<Option<&Decls<TypesDecl>>, Never> {
    Ok(match config {
      ConfigItemType::Types(c) => Some(&c.decls),
      ConfigItemType::Keycodes(_)
      | ConfigItemType::Compat(_)
      | ConfigItemType::Symbols(_)
      | ConfigItemType::Geometry(_) => None,
    })
  }
}

impl Interp {
  fn apply_field(&mut self, field: Spanned<InterpField>) -> Result<(), Never> {
    match field.val {
      InterpField::Actions(a) => {
        self.actions = Some(a.spanned2(field.span));
      }
      InterpField::VirtualModifier(m) => {
        self.virtual_modifier = Some(m.spanned2(field.span));
      }
      InterpField::Repeat(r) => {
        self.repeat = Some(r.spanned2(field.span));
      }
      InterpField::LevelOneOnly(u) => {
        self.level_one_only = Some(u.spanned2(field.span));
      }
      InterpField::Locking(e) => {
        self.locking = Some(e.spanned2(field.span));
      }
    }

    Ok(())
  }
}

impl IndicatorMap {
  fn apply_field(&mut self, field: Spanned<IndicatorMapField>) -> Result<(), Never> {
    match field.val {
      IndicatorMapField::Modifiers(e) => {
        self.modifiers = Some(e);
      }
      IndicatorMapField::Groups(e) => {
        self.groups = Some(e);
      }
      IndicatorMapField::Controls(e) => {
        self.controls = Some(e);
      }
      IndicatorMapField::WhichModifierState(e) => {
        self.which_modifier_state = Some(e);
      }
      IndicatorMapField::WhichGroupState(e) => {
        self.which_group_state = Some(e);
      }
    }

    Ok(())
  }
}

fn merge_interp(
  r: &mut Resolver<'_, '_, '_>,
  old: &mut Interp,
  new: Interp,
  merging: Merging,
) -> Result<(), Never> {
  let Ok(actions) = merged(merging, &mut old.actions, new.actions);
  let Ok(()) = ignored_interp_field(r, actions, "actions");
  let Ok(virtual_modifier) = merged(merging, &mut old.virtual_modifier, new.virtual_modifier);
  let Ok(()) = ignored_interp_field(r, virtual_modifier, "virtual_modifier");
  let Ok(repeat) = merged(merging, &mut old.repeat, new.repeat);
  let Ok(()) = ignored_interp_field(r, repeat, "repeat");
  let Ok(level_one_only) = merged(merging, &mut old.level_one_only, new.level_one_only);

  ignored_interp_field(r, level_one_only, "level_one_only")
}

fn ignored_interp_field(
  r: &mut Resolver<'_, '_, '_>,
  ignored: Option<Span>,
  field: &'static str,
) -> Result<(), Never> {
  match ignored {
    Some(span) => r.diag(
      DiagnosticKind::IgnoredInterpretField,
      ad_hoc_display!("ignoring redefinition of interp field `{}`", field => &'static str)
        .spanned2(span),
    ),
    None => Ok(()),
  }
}

fn merge_indicator_map(
  r: &mut Resolver<'_, '_, '_>,
  old: &mut IndicatorMap,
  new: IndicatorMap,
  merging: Merging,
) -> Result<(), Never> {
  let earlier_modifiers = old
    .which_modifier_state
    .as_ref()
    .map(|field| field.span)
    .or(old.modifiers.as_ref().map(|field| field.span));
  let Ok(modifiers) = merging.redefines(earlier_modifiers);
  let Ok(which_modifier_state) = redefined(
    modifiers,
    &mut old.which_modifier_state,
    new.which_modifier_state,
  );
  let Ok(()) = ignored_indicator_field(r, which_modifier_state);
  let Ok(modifier_mask) = redefined(modifiers, &mut old.modifiers, new.modifiers);
  let Ok(()) = ignored_indicator_field(r, modifier_mask);
  let earlier_groups = old
    .which_group_state
    .as_ref()
    .map(|field| field.span)
    .or(old.groups.as_ref().map(|field| field.span));
  let Ok(groups) = merging.redefines(earlier_groups);
  let Ok(which_group_state) =
    redefined(groups, &mut old.which_group_state, new.which_group_state);
  let Ok(()) = ignored_indicator_field(r, which_group_state);
  let Ok(group_mask) = redefined(groups, &mut old.groups, new.groups);
  let Ok(()) = ignored_indicator_field(r, group_mask);
  let Ok(controls) = merged(merging, &mut old.controls, new.controls);

  ignored_indicator_field(r, controls)
}

fn ignored_indicator_field(
  r: &mut Resolver<'_, '_, '_>,
  ignored: Option<Span>,
) -> Result<(), Never> {
  match ignored {
    Some(span) => r.diag(
      DiagnosticKind::IgnoredIndicatorField,
      ad_hoc_display!("ignoring redefinition").spanned2(span),
    ),
    None => Ok(()),
  }
}

impl CompatResolver<'_, '_, '_, '_> {
  fn parse_interp_field(
    &mut self,
    decl: &VarDecl,
    span: Span,
    path: FieldPath,
  ) -> Result<Option<Spanned<InterpField>>, Never> {
    let Ok(mut tables) = self.r.tables();
    let res = eval_interp_field(
      &mut tables,
      self.keycodes,
      self.mods,
      &self.data.action_defaults,
      &decl.var,
      span,
      path,
    );

    self.r.reported(res)
  }

  fn parse_indicator_field(
    &mut self,
    decl: &VarDecl,
    span: Span,
    path: FieldPath,
  ) -> Result<Option<Spanned<IndicatorMapField>>, Never> {
    let res = eval_indicator_map_field(
      self.r.interner,
      self.r.meaning_cache,
      self.mods,
      &decl.var,
      span,
      path,
    );

    self.r.reported(res)
  }

  fn handle_interp(
    &mut self,
    mm: Option<MergeMode>,
    keysym: Option<Spanned<Keysym>>,
    filter: Option<Spanned<Filter>>,
    interp: Interp,
  ) -> Result<(), Never> {
    let key = (keysym.map(|k| k.val), filter.map(|f| f.val));
    let given = InterpWithKey {
      keysym,
      filter,
      interp,
    };

    match self.data.interps.entry(key) {
      indexmap::map::Entry::Vacant(vacant) => {
        vacant.insert(given);
      }
      indexmap::map::Entry::Occupied(mut occupied) => match mm {
        Some(MergeMode::Replace) => {
          occupied.insert(given);
        }
        Some(
          MergeMode::Include | MergeMode::Augment | MergeMode::Override | MergeMode::Alternate,
        )
        | None => {
          let Ok(merging) = Merging::of(mm);
          let Ok(()) = merge_interp(
            self.r,
            &mut occupied.get_mut().interp,
            given.interp,
            merging,
          );
        }
      },
    }

    Ok(())
  }

  fn handle_indicator_map(
    &mut self,
    mm: Option<MergeMode>,
    name: Spanned<Interned>,
    indicator_map: IndicatorMap,
  ) -> Result<(), Never> {
    let given = IndicatorMapWithKey {
      name,
      indicator_map,
    };

    match self.data.indicator_maps.entry(name.val) {
      indexmap::map::Entry::Vacant(vacant) => {
        vacant.insert(given);
      }
      indexmap::map::Entry::Occupied(mut occupied) => match mm {
        Some(MergeMode::Replace) => {
          occupied.insert(given);
        }
        Some(
          MergeMode::Include | MergeMode::Augment | MergeMode::Override | MergeMode::Alternate,
        )
        | None => {
          let Ok(merging) = Merging::of(mm);
          let Ok(()) = merge_indicator_map(
            self.r,
            &mut occupied.get_mut().indicator_map,
            given.indicator_map,
            merging,
          );
        }
      },
    }

    Ok(())
  }

  fn handle_interpret(
    &mut self,
    mm: Option<MergeMode>,
    decl: &InterpretDecl,
  ) -> Result<(), Never> {
    let sym = &decl.match_.val.sym;
    let expr = match sym.val {
      InterpretSym::Ident(i) => Expr::Path(Path::One(i)),
      InterpretSym::String(s) => Expr::String(s),
      InterpretSym::Integer(i, v) => Expr::Integer(i, v),
    };
    let mut keysym = None;
    let Ok(mut tables) = self.r.tables();
    let res = eval_keysyms(
      &mut tables,
      (&expr).spanned2(sym.span),
      &mut keysym,
      |chosen, found| match ::core::primitive::bool::then_some(found == syms::NoSymbol, ()) {
        ::core::option::Option::Some(()) => Ok(()),
        ::core::option::Option::None => match chosen {
          Some(_) => Err(EvalError::MultipleKeysymsInInterpret),
          None => {
            *chosen = Some(found.spanned2(sym.span));
            Ok(())
          }
        },
      },
    );
    let Ok(evaluated) = self.r.reported(res);

    match evaluated {
      Some(()) => self.handle_interpret_filter(mm, decl, keysym),
      None => Ok(()),
    }
  }

  fn handle_interpret_filter(
    &mut self,
    mm: Option<MergeMode>,
    decl: &InterpretDecl,
    keysym: Option<Spanned<Keysym>>,
  ) -> Result<(), Never> {
    match &decl.match_.val.filter {
      None => self.handle_interpret_vars(mm, decl, keysym, None),
      Some(expr) => {
        let Ok(as_ref) = expr.as_ref();
        let res = eval_filter(self.r.interner, self.r.meaning_cache, as_ref);
        let Ok(evaluated) = self.r.reported(res);

        match evaluated {
          Some(filter) => self.handle_interpret_vars(mm, decl, keysym, Some(filter)),
          None => Ok(()),
        }
      }
    }
  }

  fn handle_interpret_vars(
    &mut self,
    mm: Option<MergeMode>,
    decl: &InterpretDecl,
    keysym: Option<Spanned<Keysym>>,
    filter: Option<Spanned<Filter>>,
  ) -> Result<(), Never> {
    let mut interp = self.data.interp_default.clone();

    for var in &decl.vars {
      let Ok(parsed) = self.parse_interp_field(&var.val, var.span, FieldPath::Bare);

      match parsed {
        Some(field) => {
          let Ok(()) = interp.apply_field(field);
        }
        None => {}
      }
    }

    self.handle_interp(mm, keysym, filter, interp)
  }

  fn handle_indicator_map_decl(
    &mut self,
    mm: Option<MergeMode>,
    decl: &IndicatorMapDecl,
  ) -> Result<(), Never> {
    let Ok(name) = self.r.cook(decl.name);
    let mut indicator_map = self.data.indicator_map_default.clone();

    for field_decl in &decl.decls {
      let Ok(parsed) =
        self.parse_indicator_field(&field_decl.val, field_decl.span, FieldPath::Bare);

      match parsed {
        Some(field) => {
          let Ok(()) = indicator_map.apply_field(field);
        }
        None => {}
      }
    }

    self.handle_indicator_map(mm, name, indicator_map)
  }

  fn handle_default(&mut self, decl: &VarDecl, span: Span) -> Result<(), Never> {
    let Ok(components) = decl.var.path.val.components();
    let first = match &components {
      Some(cs) => cs.first(),
      None => None,
    };
    let meaning = match first {
      Some(c) => {
        let Ok(found) = self
          .r
          .meaning_cache
          .get_case_insensitive(self.r.interner, c.ident);

        match c.index {
          Some(_) => None,
          None => Some(found),
        }
      }
      None => None,
    };

    match meaning {
      None => self.r.diag(
        DiagnosticKind::UnknownCompatVariable,
        ad_hoc_display!("unknown compat variable").spanned2(span),
      ),
      Some(named) => self.set_default(decl, span, named),
    }
  }

  fn set_default(&mut self, decl: &VarDecl, span: Span, meaning: Meaning) -> Result<(), Never> {
    match ::core::primitive::bool::then_some(meaning == Meaning::Interpret, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(parsed) = self.parse_interp_field(decl, span, FieldPath::Qualified);

        match parsed {
          Some(field) => self.data.interp_default.apply_field(field),
          None => Ok(()),
        }
      }
      ::core::option::Option::None => {
        match ::core::primitive::bool::then_some(meaning == Meaning::Indicator, ()) {
          ::core::option::Option::Some(()) => {
            let Ok(parsed) = self.parse_indicator_field(decl, span, FieldPath::Qualified);

            match parsed {
              Some(field) => self.data.indicator_map_default.apply_field(field),
              None => Ok(()),
            }
          }
          ::core::option::Option::None => {
            let res = eval_action_default(
              self.r.interner,
              self.r.meaning_cache,
              self.keycodes,
              self.mods,
              &decl.var,
              span,
              &mut self.data.action_defaults,
            );

            self.r.report(res)
          }
        }
      }
    }
  }
}

impl Walker<CompatmapDecl> for CompatResolver<'_, '_, '_, '_> {
  type Data = ResolvedCompat;

  fn data(&mut self) -> &mut ResolvedCompat {
    &mut self.data
  }

  fn merge_child(
    &mut self,
    mm: Option<MergeMode>,
    _group: Option<GroupIdx>,
    data: ResolvedCompat,
  ) -> Result<(), Never> {
    for interp in data.interps.into_values() {
      let Ok(()) = self.handle_interp(mm, interp.keysym, interp.filter, interp.interp);
    }

    for indicator_map in data.indicator_maps.into_values() {
      let Ok(()) = self.handle_indicator_map(mm, indicator_map.name, indicator_map.indicator_map);
    }

    Ok(())
  }

  fn handle_decl(
    &mut self,
    mm: Option<MergeMode>,
    decl: &CompatmapDecl,
    span: Span,
  ) -> Result<(), Never> {
    match decl {
      CompatmapDecl::Interpret(interpret) => self.handle_interpret(mm, interpret),
      CompatmapDecl::IndicatorMap(indicator_map) => {
        self.handle_indicator_map_decl(mm, indicator_map)
      }
      CompatmapDecl::Var(var) => self.handle_default(var, span),
      CompatmapDecl::GroupCompat(_) | CompatmapDecl::VMod(_) | CompatmapDecl::Include(_) => {
        Ok(())
      }
    }
  }
}

impl ConfigWalker<CompatmapDecl> for CompatResolver<'_, '_, '_, '_> {
  fn accept(config: &ConfigItemType) -> Result<Option<&Decls<CompatmapDecl>>, Never> {
    Ok(match config {
      ConfigItemType::Compat(c) => Some(&c.decls),
      ConfigItemType::Keycodes(_)
      | ConfigItemType::Types(_)
      | ConfigItemType::Symbols(_)
      | ConfigItemType::Geometry(_) => None,
    })
  }
}

impl SymbolsKey {
  fn apply_offset(
    &mut self,
    map: &mut CodeMap,
    diagnostics: &mut DiagnosticSink,
    group: GroupIdx,
  ) -> Result<(), Never> {
    let discarded = self
      .groups
      .iter()
      .skip(1)
      .flat_map(|g| g.levels.iter())
      .find_map(|l| {
        let Ok(span) = l.span();
        span
      });

    match discarded {
      Some(span) => {
        let Ok(()) = diagnostics.push(
          map,
          DiagnosticKind::DiscardingGroup,
          ad_hoc_display!(
            "discarding groups other than 1 due to explicit group specifier in inclued"
          )
          .spanned2(span),
        );
      }
      None => {}
    }

    self.groups.truncate(1);

    match self.groups.pop() {
      Some(first) => {
        let Ok(raw) = group.raw();
        let Ok(before) = index(raw.saturating_sub(1));
        let mut placed = vec![SymbolsKeyGroup::default(); before.min(32)];
        placed.push(first);
        self.groups = placed;
      }
      None => {}
    }

    Ok(())
  }
}

#[derive(Copy, Clone)]
enum Emptiness {
  Empty,
  NonEmpty,
}

impl SymbolsKeyLevel {
  fn emptiness(&self) -> Result<Emptiness, Never> {
    Ok(
      match ::core::primitive::bool::then_some(
        self.symbols.is_empty() && self.actions.is_empty(),
        (),
      ) {
        ::core::option::Option::Some(()) => Emptiness::Empty,
        ::core::option::Option::None => Emptiness::NonEmpty,
      },
    )
  }

  fn span(&self) -> Result<Option<Span>, Never> {
    Ok(
      self
        .symbols
        .iter()
        .map(|s| s.span)
        .chain(self.actions.iter().map(|s| s.span))
        .next(),
    )
  }
}

impl SymbolsKeyGroup {
  fn set_symbols(&mut self, symbols: GroupList<Keysym>) -> Result<(), Never> {
    self.symbols_written = Written::Explicitly;

    self.set_field(symbols, |l| &mut l.symbols)
  }

  fn set_actions(&mut self, actions: GroupList<ResolvedAction>) -> Result<(), Never> {
    self.actions_written = Written::Explicitly;

    self.set_field(actions, |l| &mut l.actions)
  }

  fn set_field<T>(
    &mut self,
    given: GroupList<T>,
    mut f: impl FnMut(&mut SymbolsKeyLevel) -> &mut Vec<Spanned<T>>,
  ) -> Result<(), Never> {
    for l in &mut self.levels {
      f(l).clear();
    }

    let mut values = given.into_iter();

    for (level, value) in self.levels.iter_mut().zip(&mut values) {
      *f(level) = value;
    }

    for value in values {
      let mut level = SymbolsKeyLevel::default();
      *f(&mut level) = value;
      self.levels.push(level);
    }

    self.trim()
  }

  fn trim(&mut self) -> Result<(), Never> {
    self.levels.truncate(
      self.levels.len().saturating_sub(
        self
          .levels
          .iter()
          .rev()
          .take_while(|level| {
            let Ok(emptiness) = level.emptiness();

            match emptiness {
              Emptiness::Empty => true,
              Emptiness::NonEmpty => false,
            }
          })
          .count(),
      ),
    );

    Ok(())
  }
}

fn place_of(
  groups: &[SymbolsKeyGroup],
  group: Option<GroupIdx>,
  written: impl Fn(&SymbolsKeyGroup) -> Written,
) -> Result<u32, Never> {
  match group {
    Some(idx) => {
      let Ok(raw) = idx.raw();

      Ok(raw.saturating_sub(1))
    }
    None => {
      let free = (0_u32..)
        .zip(groups)
        .find_map(|(at, candidate)| match written(candidate) {
          Written::Implicitly => Some(at),
          Written::Explicitly => None,
        });

      match free {
        Some(at) => Ok(at),
        None => fitted::<_, u32>(groups.len()),
      }
    }
  }
}

fn group_at(
  groups: &mut Vec<SymbolsKeyGroup>,
  at: u32,
) -> Result<Option<&mut SymbolsKeyGroup>, Never> {
  let Ok(position) = index(at);

  match ::core::primitive::bool::then_some(position >= groups.len(), ()) {
    ::core::option::Option::Some(()) => {
      groups.resize(
        position
          .saturating_add(1)
          .min(groups.len().saturating_add(32)),
        SymbolsKeyGroup::default(),
      );
    }
    ::core::option::Option::None => {}
  }

  Ok(groups.get_mut(position))
}

impl SymbolsKey {
  fn apply_field(&mut self, f: Spanned<SymbolsField>) -> Result<(), Never> {
    match f.val {
      SymbolsField::GroupKeyType(g, e) => {
        let Ok(at) = place_of(&self.groups, Some(g), |candidate| candidate.symbols_written);
        let Ok(found) = group_at(&mut self.groups, at);

        match found {
          Some(group) => {
            group.key_type = Some(e);
          }
          None => {}
        }
      }
      SymbolsField::DefaultKeyType(e) => {
        self.default_key_type = Some(e.spanned2(f.span));
      }
      SymbolsField::Symbols((group, e)) => {
        let Ok(at) = place_of(&self.groups, group, |candidate| candidate.symbols_written);
        let Ok(found) = group_at(&mut self.groups, at);

        match found {
          Some(target) => {
            let Ok(()) = target.set_symbols(e);
          }
          None => {}
        }
      }
      SymbolsField::Actions((group, e)) => {
        let Ok(at) = place_of(&self.groups, group, |candidate| candidate.actions_written);
        let Ok(found) = group_at(&mut self.groups, at);

        match found {
          Some(target) => {
            let Ok(()) = target.set_actions(e);
          }
          None => {}
        }
      }
      SymbolsField::Virtualmodifiers(e) => {
        self.virtualmodifiers = Some(e.spanned2(f.span));
      }
      SymbolsField::Repeating(repeating) => {
        use super::expr::KeyRepeat;

        let setting = match repeating {
          KeyRepeat::LayoutDefault => None,
          KeyRepeat::Enabled => Some(true),
          KeyRepeat::Disabled => Some(false),
        };
        self.repeating = Some(setting.spanned2(f.span));
      }
      SymbolsField::Locks(e) => {
        self.behavior = Some(SymbolsKeyBehavior::Locks(e).spanned2(f.span));
      }
      SymbolsField::Groupswrap => {
        self.groups_redirect = Some(GroupsRedirect::Wrap.spanned2(f.span))
      }
      SymbolsField::Groupsclamp => {
        self.groups_redirect = Some(GroupsRedirect::Clamp.spanned2(f.span))
      }
      SymbolsField::Groupsredirect(e) => {
        self.groups_redirect = Some(GroupsRedirect::Redirect(e).spanned2(f.span))
      }
      SymbolsField::Overlay(e) => {
        self.behavior = Some(SymbolsKeyBehavior::Overlay(e).spanned2(f.span))
      }
      SymbolsField::AllowNone(allow, mask) => match ::core::primitive::bool::then_some(allow, ())
      {
        ::core::option::Option::Some(()) => self.allow_none |= mask,
        ::core::option::Option::None => self.allow_none &= !mask,
      },
      SymbolsField::RadioGroup(rg) => {
        self.behavior = rg.map(|radio_group| {
          let Ok(mask) = radio_group.to_mask();
          let allow_none = self.allow_none & mask != 0;
          SymbolsKeyBehavior::RadioGroup(allow_none, radio_group).spanned2(f.span)
        });
      }
    }

    Ok(())
  }
}

fn place_key(
  r: &mut Resolver<'_, '_, '_>,
  keys: &mut HashMap<Keycode, SymbolsKeyWithKey>,
  mm: Option<MergeMode>,
  placed: SymbolsKeyWithKey,
) -> Result<(), Never> {
  match keys.entry(placed.code) {
    Entry::Vacant(vacant) => {
      vacant.insert(placed);
    }
    Entry::Occupied(mut occupied) => match mm {
      Some(MergeMode::Replace) => {
        occupied.insert(placed);
      }
      Some(
        MergeMode::Include | MergeMode::Augment | MergeMode::Override | MergeMode::Alternate,
      )
      | None => {
        let Ok(merging) = Merging::of(mm);
        let Ok(()) = merge_key(r, &mut occupied.get_mut().key, placed.key, merging);
      }
    },
  }

  Ok(())
}

fn merge_key(
  r: &mut Resolver<'_, '_, '_>,
  old: &mut SymbolsKey,
  key: SymbolsKey,
  merging: Merging,
) -> Result<(), Never> {
  let mut groups = key.groups.into_iter();

  for (old_group, group) in old.groups.iter_mut().zip(&mut groups) {
    let Ok(()) = merge_group(r, old_group, group, merging);
  }

  old.groups.extend(groups);
  let Ok(default_key_type) = merged(merging, &mut old.default_key_type, key.default_key_type);
  let Ok(()) = ignored_key_field(r, default_key_type);
  let Ok(virtualmodifiers) = merged(merging, &mut old.virtualmodifiers, key.virtualmodifiers);
  let Ok(()) = ignored_key_field(r, virtualmodifiers);
  let Ok(repeating) = merged(merging, &mut old.repeating, key.repeating);
  let Ok(()) = ignored_key_field(r, repeating);
  let Ok(groups_redirect) = merged(merging, &mut old.groups_redirect, key.groups_redirect);
  let Ok(()) = ignored_key_field(r, groups_redirect);
  let Ok(behavior) = merged(merging, &mut old.behavior, key.behavior);

  ignored_key_field(r, behavior)
}

fn merge_group(
  r: &mut Resolver<'_, '_, '_>,
  old: &mut SymbolsKeyGroup,
  group: SymbolsKeyGroup,
  merging: Merging,
) -> Result<(), Never> {
  let Ok(()) = merge_key_type(r, &mut old.key_type, group.key_type, merging);

  match ::core::primitive::bool::then_some(group.levels.len() > old.levels.len(), ()) {
    ::core::option::Option::Some(()) => {
      old
        .levels
        .resize(group.levels.len(), SymbolsKeyLevel::default());
    }
    ::core::option::Option::None => {}
  }

  for (old_level, level) in old.levels.iter_mut().zip(group.levels) {
    let Ok(span) = level.span();
    let Ok(symbols) = merge_level_field(r, merging, &mut old_level.symbols, level.symbols, span);
    let Ok(actions) = merge_level_field(r, merging, &mut old_level.actions, level.actions, span);

    match symbols {
      Merged::Replaced => {
        let Ok(written) = old.symbols_written.either(group.symbols_written);
        old.symbols_written = written;
      }
      Merged::Retained => {}
    }

    match actions {
      Merged::Replaced => {
        let Ok(written) = old.actions_written.either(group.actions_written);
        old.actions_written = written;
      }
      Merged::Retained => {}
    }
  }

  Ok(())
}

fn merge_key_type(
  r: &mut Resolver<'_, '_, '_>,
  old: &mut Option<KeyTypeRef>,
  given: Option<KeyTypeRef>,
  merging: Merging,
) -> Result<(), Never> {
  match given {
    None => Ok(()),
    Some(key_type) => match merging {
      Merging::Override => {
        *old = Some(key_type);
        Ok(())
      }
      Merging::Augment => match *old {
        None => {
          *old = Some(key_type);
          Ok(())
        }
        Some(KeyTypeRef::Named(named)) => ignored_key_field(r, Some(named.span)),
        Some(KeyTypeRef::BuiltIn(_)) => Ok(()),
      },
    },
  }
}

fn merge_level_field<T>(
  r: &mut Resolver<'_, '_, '_>,
  merging: Merging,
  old: &mut Vec<Spanned<T>>,
  new: Vec<Spanned<T>>,
  span: Option<Span>,
) -> Result<Merged, Never> {
  let Ok(redefinition) = merging.redefines(old.first().map(|earlier| earlier.span));

  match ::core::primitive::bool::then_some(new.is_empty(), ()) {
    ::core::option::Option::Some(()) => Ok(Merged::Retained),
    ::core::option::Option::None => match redefinition {
      Redefinition::Ignored => {
        let Ok(()) = ignored_key_field(r, span);

        Ok(Merged::Retained)
      }
      Redefinition::Applied => {
        *old = new;

        Ok(Merged::Replaced)
      }
    },
  }
}

fn ignored_key_field(r: &mut Resolver<'_, '_, '_>, ignored: Option<Span>) -> Result<(), Never> {
  match ignored {
    Some(span) => r.diag(
      DiagnosticKind::IgnoredKeyField,
      ad_hoc_display!("ignoring redefinition").spanned2(span),
    ),
    None => Ok(()),
  }
}

#[derive(Copy, Clone)]
enum SymbolsVariable {
  Key,
  AllowNone,
  GroupName,
  Action,
}

impl SymbolsVariable {
  fn of(meaning: Meaning) -> Result<Self, Never> {
    Ok(
      match ::core::primitive::bool::then_some(meaning == Meaning::Key, ()) {
        ::core::option::Option::Some(()) => SymbolsVariable::Key,
        ::core::option::Option::None => {
          match ::core::primitive::bool::then_some(meaning == Meaning::Allownone, ()) {
            ::core::option::Option::Some(()) => SymbolsVariable::AllowNone,
            ::core::option::Option::None => match ::core::primitive::bool::then_some(
              meaning == Meaning::Name || meaning == Meaning::Groupname,
              (),
            ) {
              ::core::option::Option::Some(()) => SymbolsVariable::GroupName,
              ::core::option::Option::None => SymbolsVariable::Action,
            },
          }
        }
      },
    )
  }
}

impl SymbolsResolver<'_, '_, '_, '_> {
  fn parse_symbols_field(
    &mut self,
    not: Option<Span>,
    path: Option<Spanned<&Path>>,
    expr: Option<Spanned<&Expr>>,
    span: Span,
    field_path: FieldPath,
  ) -> Result<Option<Spanned<SymbolsField>>, Never> {
    let Ok(mut tables) = self.r.tables();
    let res = eval_symbols_field(
      &mut tables,
      &self.data.action_defaults,
      self.keycodes,
      self.mods,
      self.types,
      KeyVar { not, path, expr }.spanned2(span),
      field_path,
    );

    self.r.reported(res)
  }

  fn handle_mod_map_entry(
    &mut self,
    mm: Option<MergeMode>,
    key: Spanned<ModMapField>,
    modifier: Option<Spanned<ModifierIndex>>,
  ) -> Result<(), Never> {
    let Ok(merging) = Merging::of(mm);

    match self.data.mod_map_entries.entry(key.val) {
      indexmap::map::Entry::Vacant(vacant) => {
        vacant.insert(ModMapEntryWithKey { key, modifier });
      }
      indexmap::map::Entry::Occupied(mut occupied) => match merging {
        Merging::Augment => {
          let Ok(()) = self.r.diag(
            DiagnosticKind::IgnoredModMapEntry,
            ad_hoc_display!("ignoring duplicate mod map entry").spanned2(key.span),
          );
        }
        Merging::Override => {
          occupied.insert(ModMapEntryWithKey { key, modifier });
        }
      },
    }

    Ok(())
  }

  fn handle_key(
    &mut self,
    mm: Option<MergeMode>,
    name: Spanned<Interned>,
    key: SymbolsKey,
  ) -> Result<(), Never> {
    match self.keycodes.name_to_key.get(&name.val) {
      None => self.r.diag(
        DiagnosticKind::UnknownKey,
        ad_hoc_display!("unknown key").spanned2(name.span),
      ),
      Some(known) => {
        let (code, real_name) = match known.kind {
          ResolvedKeyKind::Real(real) => (real.val, name),
          ResolvedKeyKind::Alias(_, aliased, Some(real)) => (aliased, real),
          ResolvedKeyKind::Alias(alias, aliased, None) => (aliased, alias),
        };
        let placed = SymbolsKeyWithKey {
          name: real_name,
          code,
          key,
        };

        place_key(self.r, &mut self.data.keys, mm, placed)
      }
    }
  }

  fn handle_group_name(
    &mut self,
    mm: Option<MergeMode>,
    idx: GroupIdx,
    name: Spanned<Interned>,
  ) -> Result<(), Never> {
    let Ok(raw) = idx.raw();
    let Ok(position) = index(raw.saturating_sub(1));

    match ::core::primitive::bool::then_some(position >= self.data.group_names.len(), ()) {
      ::core::option::Option::Some(()) => {
        self
          .data
          .group_names
          .resize(position.saturating_add(1).min(32), None);
      }
      ::core::option::Option::None => {}
    }

    let Ok(merging) = Merging::of(mm);

    match self.data.group_names.get_mut(position) {
      Some(target) => match (merging, target.is_some()) {
        (Merging::Augment, true) => self.r.diag(
          DiagnosticKind::IgnoredKeyField,
          ad_hoc_display!("ignoring group name").spanned2(name.span),
        ),
        (Merging::Augment, false) | (Merging::Override, true | false) => {
          *target = Some((idx, name));

          Ok(())
        }
      },
      None => Ok(()),
    }
  }

  fn handle_symbols(
    &mut self,
    mm: Option<MergeMode>,
    decl: &KeySymbolsDecl,
  ) -> Result<(), Never> {
    let mut key = self.data.key_default.clone();

    for var in &decl.vars {
      let Ok(parsed) = self.parse_key_var(var);

      match parsed {
        Some(field) => {
          let Ok(()) = key.apply_field(field);
        }
        None => {}
      }
    }

    self.handle_key(mm, decl.key, key)
  }

  fn parse_key_var(
    &mut self,
    var: &Spanned<VarOrExpr>,
  ) -> Result<Option<Spanned<SymbolsField>>, Never> {
    match &var.val {
      VarOrExpr::Var(v) => {
        let Ok(path) = v.path.as_ref();
        let expr = v.expr.as_ref().map(|given| {
          let Ok(borrowed) = given.as_ref();
          borrowed
        });

        self.parse_symbols_field(v.not, Some(path), expr, var.span, FieldPath::Bare)
      }
      VarOrExpr::Expr(e) => self.parse_symbols_field(
        None,
        None,
        Some(e.spanned2(var.span)),
        var.span,
        FieldPath::Bare,
      ),
    }
  }

  fn handle_mod_map(&mut self, mm: Option<MergeMode>, decl: &ModMapDecl) -> Result<(), Never> {
    let Ok(meaning) = self
      .r
      .meaning_cache
      .get_case_insensitive(self.r.interner, decl.modifier.val);

    match ::core::primitive::bool::then_some(meaning == Meaning::None, ()) {
      ::core::option::Option::Some(()) => self.handle_mod_map_keys(mm, decl, None),
      ::core::option::Option::None => {
        let Ok(real_index) =
          ident_to_real_mod_index(self.r.interner, self.r.meaning_cache, decl.modifier.val);

        match real_index {
          Some(real) => {
            self.handle_mod_map_keys(mm, decl, Some(real.spanned2(decl.modifier.span)))
          }
          None => self.r.diag(
            DiagnosticKind::UnknownModifier,
            ad_hoc_display!("unknown modifier").spanned2(decl.modifier.span),
          ),
        }
      }
    }
  }

  fn handle_mod_map_keys(
    &mut self,
    mm: Option<MergeMode>,
    decl: &ModMapDecl,
    modifier: Option<Spanned<ModifierIndex>>,
  ) -> Result<(), Never> {
    let mut fields = vec![];

    for key in &decl.keys {
      let Ok(mut tables) = self.r.tables();
      let res = eval_mod_map_field(
        &mut tables,
        self.keycodes,
        &key.val,
        key.span,
        &mut fields,
        |found, field| {
          found.push(field);
        },
      );
      let Ok(()) = self.r.report(res);
    }

    for field in fields {
      let Ok(()) = self.handle_mod_map_entry(mm, field, modifier);
    }

    Ok(())
  }

  fn handle_default(
    &mut self,
    mm: Option<MergeMode>,
    decl: &VarDecl,
    span: Span,
  ) -> Result<(), Never> {
    let Ok(components) = decl.var.path.val.components();
    let cs = match &components {
      Some(cs) => cs.as_slice(),
      None => &[],
    };

    match cs.first() {
      None => self.unknown_field(decl),
      Some(c) => {
        let Ok(meaning) = self
          .r
          .meaning_cache
          .get_case_insensitive(self.r.interner, c.ident);
        let Ok(variable) = SymbolsVariable::of(meaning);

        match variable {
          SymbolsVariable::Key => match c.index {
            Some(_) => self.unknown_field(decl),
            None => self.set_key_default(decl, span, FieldPath::Qualified),
          },
          SymbolsVariable::AllowNone => self.set_key_default(decl, span, FieldPath::Bare),
          SymbolsVariable::GroupName => {
            match ::core::primitive::bool::then_some(cs.len() > 1, ()) {
              ::core::option::Option::Some(()) => self.unknown_field(decl),
              ::core::option::Option::None => match c.index {
                None => self.unknown_field(decl),
                Some(subscript) => self.handle_group_name_default(mm, decl, span, subscript),
              },
            }
          }
          SymbolsVariable::Action => {
            let res = eval_action_default(
              self.r.interner,
              self.r.meaning_cache,
              self.keycodes,
              self.mods,
              &decl.var,
              span,
              &mut self.data.action_defaults,
            );

            self.r.report(res)
          }
        }
      }
    }
  }

  fn unknown_field(&mut self, decl: &VarDecl) -> Result<(), Never> {
    self.r.diag(
      DiagnosticKind::UnknownSymbolsVariable,
      ad_hoc_display!("unknown field").spanned2(decl.var.path.span),
    )
  }

  fn set_key_default(
    &mut self,
    decl: &VarDecl,
    span: Span,
    field_path: FieldPath,
  ) -> Result<(), Never> {
    let Ok(path) = decl.var.path.as_ref();
    let expr = decl.var.expr.as_ref().map(|given| {
      let Ok(borrowed) = given.as_ref();
      borrowed
    });
    let Ok(field) = self.parse_symbols_field(decl.var.not, Some(path), expr, span, field_path);

    match field {
      Some(parsed) => self.data.key_default.apply_field(parsed),
      None => Ok(()),
    }
  }

  fn handle_group_name_default(
    &mut self,
    mm: Option<MergeMode>,
    decl: &VarDecl,
    span: Span,
    subscript: &PathIndex,
  ) -> Result<(), Never> {
    match decl.var.expr.as_ref() {
      None => self.r.diag(
        DiagnosticKind::MissingGroupName,
        ad_hoc_display!("missing group name").spanned2(span),
      ),
      Some(given) => {
        let Ok(expr) = given.as_ref();
        let res = eval_string(
          self.r.map,
          self.r.diagnostics,
          self.r.interner,
          self.r.cooker,
          expr,
        );
        let Ok(evaluated) = self.r.reported(res);

        match evaluated {
          None => Ok(()),
          Some(name) => {
            let Ok(group_index) = subscript.index.as_ref();
            let group = eval_group(self.r.interner, group_index);
            let Ok(group_idx) = self.r.reported(group);

            match group_idx {
              Some(idx) => self.handle_group_name(mm, idx.val, name),
              None => Ok(()),
            }
          }
        }
      }
    }
  }
}

impl Walker<SymbolsDecl> for SymbolsResolver<'_, '_, '_, '_> {
  type Data = ResolvedSymbols;

  fn data(&mut self) -> &mut ResolvedSymbols {
    &mut self.data
  }

  fn merge_child(
    &mut self,
    mm: Option<MergeMode>,
    group: Option<GroupIdx>,
    data: ResolvedSymbols,
  ) -> Result<(), Never> {
    for mut key in data.keys.into_values() {
      match group {
        Some(idx) => {
          let Ok(()) = key.key.apply_offset(self.r.map, self.r.diagnostics, idx);
        }
        None => {}
      }

      let Ok(()) = self.handle_key(mm, key.name, key.key);
    }

    for entry in data.mod_map_entries.into_values() {
      let Ok(()) = self.handle_mod_map_entry(mm, entry.key, entry.modifier);
    }

    let mut names = data.group_names.into_iter();

    match group {
      Some(idx) => {
        let first = names.next();

        match names.last() {
          Some(Some((_, beyond))) => {
            let Ok(()) = self.r.diag(
              DiagnosticKind::DiscardingGroup,
              ad_hoc_display!(
                "discarding groups other than 1 due to explicit group specifier in include"
              )
              .spanned2(beyond.span),
            );
          }
          Some(None) | None => {}
        }

        match first {
          Some(Some((_, name))) => self.handle_group_name(mm, idx, name),
          Some(None) | None => Ok(()),
        }
      }
      None => {
        for (idx, name) in names.flatten() {
          let Ok(()) = self.handle_group_name(mm, idx, name);
        }

        Ok(())
      }
    }
  }

  fn handle_decl(
    &mut self,
    mm: Option<MergeMode>,
    decl: &SymbolsDecl,
    span: Span,
  ) -> Result<(), Never> {
    match decl {
      SymbolsDecl::Symbols(symbols) => self.handle_symbols(mm, symbols),
      SymbolsDecl::ModMap(mod_map) => self.handle_mod_map(mm, mod_map),
      SymbolsDecl::Var(var) => self.handle_default(mm, var, span),
      SymbolsDecl::Include(_) | SymbolsDecl::VMod(_) => Ok(()),
    }
  }
}

impl ConfigWalker<SymbolsDecl> for SymbolsResolver<'_, '_, '_, '_> {
  fn accept(config: &ConfigItemType) -> Result<Option<&Decls<SymbolsDecl>>, Never> {
    Ok(match config {
      ConfigItemType::Symbols(c) => Some(&c.decls),
      ConfigItemType::Keycodes(_)
      | ConfigItemType::Types(_)
      | ConfigItemType::Compat(_)
      | ConfigItemType::Geometry(_) => None,
    })
  }
}
