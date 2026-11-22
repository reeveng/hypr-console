use crate::xkb::code::Code;
use crate::xkb::code_loader::CodeLoader;
use crate::xkb::code_loader::CodeType;
use crate::xkb::code_map::CodeMap;
use crate::xkb::diagnostic::DiagnosticKind;
use crate::xkb::diagnostic::DiagnosticSink;
use crate::xkb::group::GroupIdx;
use crate::xkb::interner::Interned;
use crate::xkb::interner::Interner;
use crate::xkb::kccgst::MergeMode;
use crate::xkb::kccgst::ast::Compat;
use crate::xkb::kccgst::ast::CompatmapDecl;
use crate::xkb::kccgst::ast::CompositeMap;
use crate::xkb::kccgst::ast::ConfigItem;
use crate::xkb::kccgst::ast::ConfigItemType;
use crate::xkb::kccgst::ast::Decl;
use crate::xkb::kccgst::ast::Decls;
use crate::xkb::kccgst::ast::DirectOrIncluded;
use crate::xkb::kccgst::ast::Geometry;
use crate::xkb::kccgst::ast::GeometryDecl;
use crate::xkb::kccgst::ast::Include;
use crate::xkb::kccgst::ast::Item;
use crate::xkb::kccgst::ast::ItemType;
use crate::xkb::kccgst::ast::KeycodeDecl;
use crate::xkb::kccgst::ast::Keycodes;
use crate::xkb::kccgst::ast::NestedConfigItem;
use crate::xkb::kccgst::ast::Symbols;
use crate::xkb::kccgst::ast::SymbolsDecl;
use crate::xkb::kccgst::ast::Types;
use crate::xkb::kccgst::ast::TypesDecl;
use crate::xkb::meaning::MeaningCache;
use crate::xkb::rmlvo::lexer::Lexer;
use crate::xkb::rmlvo::parser::Line;
use crate::xkb::rmlvo::parser::MappingKey;
use crate::xkb::rmlvo::parser::MappingKeyIndex;
use crate::xkb::rmlvo::parser::MappingValue;
use crate::xkb::rmlvo::parser::ParserCache;
use crate::xkb::rmlvo::parser::RuleKey;
use crate::xkb::rmlvo::parser::RuleValue;
use crate::xkb::rmlvo::parser::parse_line;
use crate::xkb::rmlvo::token::Token;
use crate::xkb::span::Span;
use crate::xkb::span::SpanExt;
use crate::xkb::span::SpanUnit;
use crate::xkb::span::Spanned;
use console_core_never::Never;
use console_core_number_conversion::FromBytes;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
use console_input_keymaps_derive::ad_hoc_display;
use hashbrown::HashMap;
use hashbrown::HashSet;
use std::collections::VecDeque;
use std::path::Path;

pub(crate) struct Group {
  pub(crate) layout: Option<Interned>,
  pub(crate) variant: Option<Interned>,
}

pub(crate) struct Tables<'a, 'b, 'c> {
  pub(crate) map: &'a mut CodeMap,
  pub(crate) interner: &'a mut Interner,
  pub(crate) loader: &'a mut CodeLoader,
  pub(crate) diagnostics: &'a mut DiagnosticSink<'b, 'c>,
  pub(crate) meaning_cache: &'a mut MeaningCache,
  pub(crate) remaining_runtime: &'a mut u64,
}

pub(crate) struct Names<'a> {
  pub(crate) rules: Spanned<Interned>,
  pub(crate) model: Option<Interned>,
  pub(crate) options: &'a [Interned],
  pub(crate) groups: &'a [Group],
  pub(crate) max_includes: u64,
}

type Queue = VecDeque<Spanned<Include>>;

#[derive(Default)]
struct Includes {
  keycodes: Queue,
  types: Queue,
  compat: Queue,
  symbols: Queue,
  geometry: Queue,
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum Availability {
  Open,
  Claimed,
}

#[derive(Copy, Clone)]
enum RuleMatch {
  Matching,
  Missed,
}

#[derive(Copy, Clone)]
struct GroupMatch {
  availability: Availability,
  rule: RuleMatch,
}

#[derive(Copy, Clone)]
enum OptionKey {
  Present,
  Absent,
}

#[derive(Copy, Clone)]
enum GroupKeys {
  Absent,
  Fixed,
  Ranged,
}

#[derive(Copy, Clone)]
enum Mapping {
  Unread,
  Reading {
    options: OptionKey,
    groups: GroupKeys,
  },
  Skipping,
}

#[derive(Copy, Clone)]
enum Verdict {
  Holds,
  Fails,
}

#[derive(Copy, Clone)]
enum Part {
  Layout,
  Variant,
}

enum Stint {
  Ended,
  Opened((Interned, Lexer)),
  Stopped,
}

enum Next {
  Line,
  Opened((Interned, Lexer)),
}

struct Walk<'n> {
  names: &'n Names<'n>,
  options: HashSet<Interned>,
  active_includes: HashSet<Interned>,
  macros: HashMap<Interned, HashSet<Interned>>,
  mapping: Mapping,
  mapping_keys: Vec<MappingKey>,
  mapping_values: Vec<MappingValue>,
  matched_groups: Vec<GroupMatch>,
  stash: Vec<u8>,
  num_includes_processed: u64,
  includes: Includes,
}

struct Facts<'a> {
  model: Option<Interned>,
  options: &'a HashSet<Interned>,
  macros: &'a HashMap<Interned, HashSet<Interned>>,
  groups: &'a [Group],
}

struct Expansions<'e, 'b, 'c> {
  map: &'e mut CodeMap,
  interner: &'e mut Interner,
  diagnostics: &'e mut DiagnosticSink<'b, 'c>,
  names: &'e Names<'e>,
  path: &'e Path,
  line: Span,
  stash: &'e mut Vec<u8>,
}

struct Expansion<'q> {
  value: Spanned<Interned>,
  group: Option<u32>,
  includes: &'q mut Queue,
  segment: Segment,
}

#[derive(Copy, Clone)]
struct Segment {
  merge_mode: Option<Spanned<MergeMode>>,
  start: u32,
}

enum Flushed {
  Retained,
  Abandoned,
}

#[derive(Copy, Clone)]
enum Colon {
  Present,
  Absent,
}

enum Measured {
  Found(u32),
  Broken(u32),
}

#[derive(Copy, Clone)]
enum Wrapping {
  Bare,
  Parenthesized,
}

#[derive(Copy, Clone)]
enum Lead {
  None,
  MergeMode(Spanned<MergeMode>),
  Prefix(u8),
}

#[derive(Copy, Clone)]
enum Component {
  Model,
  Layout,
  Variant,
  Index,
}

#[derive(Copy, Clone)]
enum Indexed {
  No,
  ByMatch,
  At(GroupIdx),
}

#[derive(Copy, Clone)]
struct PercentEncoding {
  wrapping: Wrapping,
  lead: Lead,
  component: Component,
  indexed: Indexed,
}

#[derive(Copy, Clone)]
enum Piece {
  Name(Interned),
  Number(u32),
  Skipped,
  Invalid,
}

#[derive(Copy, Clone)]
enum Validity {
  Valid,
  Invalid,
}

pub(crate) fn create_item(
  tables: &mut Tables<'_, '_, '_>,
  names: &Names<'_>,
) -> Result<Item, Never> {
  let Ok(includes) = create_includes(tables, names);
  let Ok(keycodes) = decls(includes.keycodes, KeycodeDecl::Include);
  let Ok(types) = decls(includes.types, TypesDecl::Include);
  let Ok(compat) = decls(includes.compat, CompatmapDecl::Include);
  let Ok(symbols) = decls(includes.symbols, SymbolsDecl::Include);
  let Ok(geometry) = decls(includes.geometry, GeometryDecl::Include);
  let items = [
    ConfigItemType::Keycodes(Keycodes {
      name: None,
      decls: keycodes,
    }),
    ConfigItemType::Types(Types {
      name: None,
      decls: types,
    }),
    ConfigItemType::Compat(Compat {
      name: None,
      decls: compat,
    }),
    ConfigItemType::Symbols(Symbols {
      name: None,
      decls: symbols,
    }),
    ConfigItemType::Geometry(Geometry {
      name: None,
      decls: geometry,
    }),
  ];
  let composite = CompositeMap {
    name: None,
    config_items: items
      .into_iter()
      .map(|specific| {
        NestedConfigItem {
          flags: Default::default(),
          item: ConfigItem { specific },
        }
        .spanned(0, 0)
      })
      .collect(),
  };
  Ok(Item {
    flags: Default::default(),
    ty: ItemType::Composite(composite),
  })
}

fn decls<T>(includes: Queue, wrap: fn(Include) -> T) -> Result<Decls<T>, Never> {
  Ok(Decls {
    decls: includes
      .into_iter()
      .map(|include| {
        let Ok(decl) = include.map(|i| Decl {
          merge_mode: Some(i.mm),
          ty: DirectOrIncluded::Direct(wrap(i)),
        });
        decl
      })
      .collect(),
  })
}

fn create_includes(
  tables: &mut Tables<'_, '_, '_>,
  names: &Names<'_>,
) -> Result<Includes, Never> {
  let unmatched = GroupMatch {
    availability: Availability::Claimed,
    rule: RuleMatch::Missed,
  };
  let mut walk = Walk {
    names,
    options: names.options.iter().copied().collect(),
    active_includes: HashSet::new(),
    macros: HashMap::new(),
    mapping: Mapping::Unread,
    mapping_keys: vec![],
    mapping_values: vec![],
    matched_groups: vec![unmatched; names.groups.len()],
    stash: vec![],
    num_includes_processed: 0,
    includes: Includes::default(),
  };
  walk.active_includes.insert(names.rules.val);
  let mut cache = ParserCache::default();
  let mut tokens = vec![];
  let mut lexers: Vec<(Interned, Lexer)> = vec![];
  let Ok(first_lexer) = create_lexer(tables, names.rules);

  match first_lexer {
    Some(lexer) => lexers.push((names.rules.val, lexer)),
    None => {}
  }

  let stints = names.max_includes.saturating_mul(2).saturating_add(2);

  for _ in 0..stints {
    let (name, mut lexer) = match lexers.pop() {
      Some(top) => top,
      None => break,
    };
    let Ok(stint) = walk.stint(tables, &mut lexer, &mut cache, &mut tokens);

    match stint {
      Stint::Ended => {
        walk.active_includes.remove(&name);
      }
      Stint::Opened(opened) => {
        lexers.push((name, lexer));
        lexers.push(opened);
      }
      Stint::Stopped => break,
    }
  }

  Ok(walk.includes)
}

impl Includes {
  fn of(&mut self, value: MappingValue) -> Result<&mut Queue, Never> {
    Ok(match value {
      MappingValue::Keycodes => &mut self.keycodes,
      MappingValue::Types => &mut self.types,
      MappingValue::Compat => &mut self.compat,
      MappingValue::Symbols => &mut self.symbols,
      MappingValue::Geometry => &mut self.geometry,
    })
  }
}

impl Walk<'_> {
  fn stint(
    &mut self,
    tables: &mut Tables<'_, '_, '_>,
    lexer: &mut Lexer,
    cache: &mut ParserCache,
    tokens: &mut Vec<Spanned<Token>>,
  ) -> Result<Stint, Never> {
    let Ok(steps) = lexer.steps_left();

    for _ in 0..steps {
      tokens.clear();

      match lexer.lex_line(tables.interner, tokens) {
        Err(e) => {
          let Ok(diagnostic_kind) = e.val.diagnostic_kind();
          let Ok(()) = tables.diagnostics.push(tables.map, diagnostic_kind, e);
          return Ok(Stint::Ended);
        }
        Ok(()) => {}
      }

      let end = match tokens.last() {
        Some(last) => last.span,
        None => return Ok(Stint::Ended),
      };
      let parsed = parse_line(
        tables.map,
        tables.diagnostics,
        tables.interner,
        cache,
        tables.meaning_cache,
        tokens.as_slice(),
        end,
      );

      let line = match parsed {
        Ok(parsed_line) => parsed_line,
        Err(e) => {
          let Ok(diagnostic_kind) = e.val.diagnostic_kind();
          let Ok(()) = tables.diagnostics.push(tables.map, diagnostic_kind, e);
          continue;
        }
      };

      match ::core::primitive::bool::then_some(*tables.remaining_runtime == 0, ()) {
        ::core::option::Option::Some(()) => {
          let Ok(()) = tables.diagnostics.push(
            tables.map,
            DiagnosticKind::MaxRuntimeReached,
            ad_hoc_display!("maximum number of statements reached").spanned2(line.span),
          );
          return Ok(Stint::Stopped);
        }
        ::core::option::Option::None => {}
      }

      *tables.remaining_runtime = tables.remaining_runtime.saturating_sub(1);
      let Ok(path) = lexer.path();
      let Ok(next) = self.line(tables, path, line);

      match next {
        Next::Line => {}
        Next::Opened(opened) => return Ok(Stint::Opened(opened)),
      }
    }

    Ok(Stint::Ended)
  }

  fn line(
    &mut self,
    tables: &mut Tables<'_, '_, '_>,
    path: &Path,
    line: Spanned<Line<'_>>,
  ) -> Result<Next, Never> {
    match line.val {
      Line::Macro(name, members) => {
        self.macros.insert(name, members.iter().copied().collect());
        Ok(Next::Line)
      }
      Line::Include(include) => self.include(tables, include.spanned2(line.span)),
      Line::Mapping(keys, values) => {
        let Ok(()) = self.mapping(keys, values);
        Ok(Next::Line)
      }
      Line::Rule(keys, values) => {
        let Ok(()) = self.rule(tables, path, line.span, keys, values);
        Ok(Next::Line)
      }
    }
  }

  fn include(
    &mut self,
    tables: &mut Tables<'_, '_, '_>,
    include: Spanned<Interned>,
  ) -> Result<Next, Never> {
    self.num_includes_processed = self.num_includes_processed.saturating_add(1);

    match ::core::primitive::bool::then_some(self.num_includes_processed >= self.names.max_includes, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = tables.diagnostics.push(
          tables.map,
          DiagnosticKind::MaxIncludesReached,
          ad_hoc_display!("maximum number of includes reached").spanned2(include.span),
        );
        return Ok(Next::Line);
      }
      ::core::option::Option::None => {}
    }

    match ::core::primitive::bool::then_some(self.active_includes.contains(&include.val), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = tables.diagnostics.push(
          tables.map,
          DiagnosticKind::RecursiveInclude,
          ad_hoc_display!("ignoring recursive include").spanned2(include.span),
        );
        return Ok(Next::Line);
      }
      ::core::option::Option::None => {}
    }

    let Ok(lexer) = create_lexer(tables, include);

    Ok(match lexer {
      Some(opened) => {
        self.active_includes.insert(include.val);
        Next::Opened((include.val, opened))
      }
      None => Next::Line,
    })
  }

  fn mapping(&mut self, keys: &[MappingKey], values: &[MappingValue]) -> Result<(), Never> {
    let options = match ::core::primitive::bool::then_some(keys.contains(&MappingKey::Option), ()) {
      ::core::option::Option::Some(()) => OptionKey::Present,
      ::core::option::Option::None => OptionKey::Absent,
    };
    let Ok(groups) = group_keys(keys);
    self.mapping = Mapping::Reading { options, groups };
    self.mapping_keys.clear();
    self.mapping_keys.extend_from_slice(keys);
    self.mapping_values.clear();
    self.mapping_values.extend_from_slice(values);

    for group in &mut self.matched_groups {
      group.availability = Availability::Open;
    }

    Ok(())
  }

  fn rule(
    &mut self,
    tables: &mut Tables<'_, '_, '_>,
    path: &Path,
    span: Span,
    keys: &[RuleKey],
    values: &[RuleValue],
  ) -> Result<(), Never> {
    let (options, groups) = match self.mapping {
      Mapping::Unread => {
        let Ok(()) = tables.diagnostics.push(
          tables.map,
          DiagnosticKind::UnexpectedRule,
          ad_hoc_display!("rule line without mapping").spanned2(span),
        );
        return Ok(());
      }
      Mapping::Skipping => return Ok(()),
      Mapping::Reading { options, groups } => (options, groups),
    };
    let Ok(counted) = self.counted(tables, span, keys, values);

    match counted {
      Verdict::Holds => {}
      Verdict::Fails => return Ok(()),
    }

    let facts = Facts {
      model: self.names.model,
      options: &self.options,
      macros: &self.macros,
      groups: self.names.groups,
    };
    let Ok(verdict) = holds(&facts, &mut self.matched_groups, &self.mapping_keys, keys);

    match verdict {
      Verdict::Holds => {}
      Verdict::Fails => return Ok(()),
    }

    match options {
      OptionKey::Present => {}
      OptionKey::Absent => {
        let Ok(()) = self.settle(groups);
      }
    }

    self.expand_values(tables, path, span, values, groups)
  }

  fn counted(
    &self,
    tables: &mut Tables<'_, '_, '_>,
    span: Span,
    keys: &[RuleKey],
    values: &[RuleValue],
  ) -> Result<Verdict, Never> {
    let Ok(wanted_keys) = fitted::<_, u32>(self.mapping_keys.len());
    let Ok(got_keys) = fitted::<_, u32>(keys.len());

    match ::core::primitive::bool::then_some(wanted_keys == got_keys, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        let Ok(()) = tables.diagnostics.push(
                    tables.map,
                    DiagnosticKind::InvalidNumberOfRuleKeys,
                    ad_hoc_display!("expected {} rule keys but got {}", wanted_keys => u32, got_keys => u32)
                        .spanned2(span),
                );
        return Ok(Verdict::Fails);
      }
    }

    let Ok(wanted_values) = fitted::<_, u32>(self.mapping_values.len());
    let Ok(got_values) = fitted::<_, u32>(values.len());

    match ::core::primitive::bool::then_some(wanted_values == got_values, ()) {
      ::core::option::Option::Some(()) => Ok(Verdict::Holds),
      ::core::option::Option::None => {
        let Ok(()) = tables.diagnostics.push(
                    tables.map,
                    DiagnosticKind::InvalidNumberOfRuleValues,
                    ad_hoc_display!("expected {} rule values but got {}", wanted_values => u32, got_values => u32)
                        .spanned2(span),
                );
        Ok(Verdict::Fails)
      }
    }
  }

  fn settle(&mut self, groups: GroupKeys) -> Result<(), Never> {
    match groups {
      GroupKeys::Absent | GroupKeys::Fixed => {
        self.mapping = Mapping::Skipping;
        return Ok(());
      }
      GroupKeys::Ranged => {}
    }

    for group in &mut self.matched_groups {
      group.availability = match group.rule {
        RuleMatch::Matching => Availability::Claimed,
        RuleMatch::Missed => group.availability,
      };
    }

    let open = self
      .matched_groups
      .iter()
      .any(|group| group.availability == Availability::Open);

    match ::core::primitive::bool::then_some(open, ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => self.mapping = Mapping::Skipping,
    }

    Ok(())
  }

  fn expand_values(
    &mut self,
    tables: &mut Tables<'_, '_, '_>,
    path: &Path,
    span: Span,
    values: &[RuleValue],
    groups: GroupKeys,
  ) -> Result<(), Never> {
    let mut expansions = Expansions {
      map: &mut *tables.map,
      interner: &mut *tables.interner,
      diagnostics: &mut *tables.diagnostics,
      names: self.names,
      path,
      line: span,
      stash: &mut self.stash,
    };

    for (value, rule_value) in self.mapping_values.iter().zip(values) {
      let Ok(includes) = self.includes.of(*value);

      match groups {
        GroupKeys::Absent => {
          let Ok(()) = expansions.expand(rule_value.ident, None, includes);
        }
        GroupKeys::Fixed | GroupKeys::Ranged => {
          let Ok(()) =
            expansions.expand_matched(rule_value.ident, &self.matched_groups, includes);
        }
      }
    }

    Ok(())
  }
}

fn group_keys(keys: &[MappingKey]) -> Result<GroupKeys, Never> {
  let grouped = keys
    .iter()
    .any(|key| matches!(key, MappingKey::Layout(_) | MappingKey::Variant(_)));
  let ranged = keys.iter().any(|key| {
    matches!(
      key,
      MappingKey::Layout(Some(MappingKeyIndex::Later | MappingKeyIndex::Any))
        | MappingKey::Variant(Some(MappingKeyIndex::Later | MappingKeyIndex::Any))
    )
  });

  Ok(match (grouped, ranged) {
    (false, _) => GroupKeys::Absent,
    (true, false) => GroupKeys::Fixed,
    (true, true) => GroupKeys::Ranged,
  })
}

fn holds(
  facts: &Facts<'_>,
  matched: &mut [GroupMatch],
  mapping_keys: &[MappingKey],
  keys: &[RuleKey],
) -> Result<Verdict, Never> {
  for group in matched.iter_mut() {
    group.rule = match group.availability {
      Availability::Open => RuleMatch::Matching,
      Availability::Claimed => RuleMatch::Missed,
    };
  }

  for (mapping_key, key) in mapping_keys.iter().zip(keys) {
    let Ok(verdict) = match mapping_key {
      MappingKey::Model => model_holds(facts, *key),
      MappingKey::Option => option_holds(facts, *key),
      MappingKey::Layout(at) => groups_hold(facts, matched, Part::Layout, *at, *key),
      MappingKey::Variant(at) => groups_hold(facts, matched, Part::Variant, *at, *key),
    };

    match verdict {
      Verdict::Holds => {}
      Verdict::Fails => return Ok(Verdict::Fails),
    }
  }

  Ok(Verdict::Holds)
}

fn model_holds(facts: &Facts<'_>, key: RuleKey) -> Result<Verdict, Never> {
  let found = match key {
    RuleKey::Star | RuleKey::Any => true,
    RuleKey::None => facts.model.is_none(),
    RuleKey::Some => facts.model.is_some(),
    RuleKey::Macro(name) => match (facts.model, facts.macros.get(&name)) {
      (Some(model), Some(members)) => members.contains(&model),
      (Some(_), None) | (None, Some(_) | None) => false,
    },
    RuleKey::Ident(i) => facts.model == Some(i),
  };

  Ok(match ::core::primitive::bool::then_some(found, ()) {
    ::core::option::Option::Some(()) => Verdict::Holds,
    ::core::option::Option::None => Verdict::Fails,
  })
}

fn option_holds(facts: &Facts<'_>, key: RuleKey) -> Result<Verdict, Never> {
  let found = match key {
    RuleKey::Star | RuleKey::Any => true,
    RuleKey::None => facts.options.is_empty(),
    RuleKey::Some => !facts.options.is_empty(),
    RuleKey::Macro(name) => match facts.macros.get(&name) {
      Some(members) => !members.is_disjoint(facts.options),
      None => false,
    },
    RuleKey::Ident(i) => facts.options.contains(&i),
  };

  Ok(match ::core::primitive::bool::then_some(found, ()) {
    ::core::option::Option::Some(()) => Verdict::Holds,
    ::core::option::Option::None => Verdict::Fails,
  })
}

fn group_holds(
  facts: &Facts<'_>,
  named: Option<Interned>,
  key: RuleKey,
) -> Result<Verdict, Never> {
  let found = match key {
    RuleKey::Any => true,
    RuleKey::None => named.is_none(),
    RuleKey::Star | RuleKey::Some => named.is_some(),
    RuleKey::Macro(name) => match (named, facts.macros.get(&name)) {
      (Some(held), Some(members)) => members.contains(&held),
      (Some(_), None) | (None, Some(_) | None) => false,
    },
    RuleKey::Ident(i) => named == Some(i),
  };

  Ok(match ::core::primitive::bool::then_some(found, ()) {
    ::core::option::Option::Some(()) => Verdict::Holds,
    ::core::option::Option::None => Verdict::Fails,
  })
}

fn groups_hold(
  facts: &Facts<'_>,
  matched: &mut [GroupMatch],
  part: Part,
  at: Option<MappingKeyIndex>,
  key: RuleKey,
) -> Result<Verdict, Never> {
  let Ok(count) = fitted::<_, u32>(facts.groups.len());

  let wanted = match at {
    Some(wanted) => wanted,
    None => MappingKeyIndex::Single,
  };
  let range = match (wanted, count == 1) {
    (MappingKeyIndex::Single, true) | (MappingKeyIndex::First, _) => 0..1,
    (MappingKeyIndex::Later, _) => 1..count,
    (MappingKeyIndex::Any, _) => 0..count,
    (MappingKeyIndex::Value(group), false) => {
      let Ok(raw) = group.raw();
      let from = raw.saturating_sub(1);
      from..from.saturating_add(1)
    }
    (MappingKeyIndex::Single, false) | (MappingKeyIndex::Value(_), true) => {
      return Ok(Verdict::Fails);
    }
  };
  let mut matched_any = Verdict::Fails;

  for (position, (group, state)) in (0_u32..).zip(facts.groups.iter().zip(matched.iter_mut())) {
    let named = match part {
      Part::Layout => group.layout,
      Part::Variant => group.variant,
    };

    state.rule = match (state.rule, range.contains(&position)) {
      (RuleMatch::Matching, true) => {
        let Ok(verdict) = group_holds(facts, named, key);

        match verdict {
          Verdict::Holds => {
            matched_any = Verdict::Holds;
            RuleMatch::Matching
          }
          Verdict::Fails => RuleMatch::Missed,
        }
      }
      (RuleMatch::Matching, false) | (RuleMatch::Missed, _) => RuleMatch::Missed,
    };
  }

  Ok(matched_any)
}

impl Expansions<'_, '_, '_> {
  fn expand_matched(
    &mut self,
    value: Spanned<Interned>,
    matched: &[GroupMatch],
    includes: &mut Queue,
  ) -> Result<(), Never> {
    for (at, group) in (0_u32..).zip(matched) {
      match group.rule {
        RuleMatch::Matching => {
          let Ok(()) = self.expand(value, Some(at), includes);
        }
        RuleMatch::Missed => {}
      }
    }

    Ok(())
  }

  fn expand(
    &mut self,
    value: Spanned<Interned>,
    group: Option<u32>,
    includes: &mut Queue,
  ) -> Result<(), Never> {
    self.stash.clear();
    let Ok(interned_input) = self.interner.get(value.val);
    let Ok(input) = interned_input.to_owned();
    let Ok(bytes) = input.as_bytes();
    let Ok(length) = fitted::<_, u32>(bytes.len());
    let mut expansion = Expansion {
      value,
      group,
      includes,
      segment: Segment {
        merge_mode: None,
        start: 0,
      },
    };
    let mut offset = 0_u32;
    let mut colon = Colon::Absent;

    for _ in 0..length {
      let Ok(position) = index(offset);

      let b = match bytes.get(position) {
        Some(b) => *b,
        None => break,
      };
      let after_colon = colon;
      colon = match ::core::primitive::bool::then_some(b == b':', ()) {
        ::core::option::Option::Some(()) => Colon::Present,
        ::core::option::Option::None => Colon::Absent,
      };
      let mode = match b {
        b'+' => Some(MergeMode::Override),
        b'|' => Some(MergeMode::Augment),
        b'^' => Some(MergeMode::Replace),
        _ => None,
      };

      let (next, flushed) = match (mode, b) {
        (Some(merge_mode), _) => {
          let Ok(flushed) = self.merge_mode(&mut expansion, merge_mode, offset);
          (offset.saturating_add(1), flushed)
        }
        (None, b'%') => {
          let Ok(rest) = after(bytes, offset.saturating_add(1));
          let Ok(stepped) = self.percent(&mut expansion, rest, offset, after_colon);
          stepped
        }
        (None, _) => {
          self.stash.push(b);
          (offset.saturating_add(1), Flushed::Retained)
        }
      };

      match flushed {
        Flushed::Retained => offset = next,
        Flushed::Abandoned => return Ok(()),
      }
    }

    let Ok(Flushed::Retained | Flushed::Abandoned) = self.flush(&mut expansion, offset);
    Ok(())
  }

  fn merge_mode(
    &mut self,
    expansion: &mut Expansion<'_>,
    mode: MergeMode,
    at: u32,
  ) -> Result<Flushed, Never> {
    let Ok(flushed) = self.flush(expansion, at);

    match flushed {
      Flushed::Retained => {}
      Flushed::Abandoned => return Ok(Flushed::Abandoned),
    }

    let lo = expansion.value.span.lo.saturating_add(at);
    expansion.segment = Segment {
      merge_mode: Some(mode.spanned(lo, lo.saturating_add(1))),
      start: at.saturating_add(1),
    };
    Ok(Flushed::Retained)
  }

  fn percent(
    &mut self,
    expansion: &mut Expansion<'_>,
    rest: &[u8],
    at: u32,
    colon: Colon,
  ) -> Result<(u32, Flushed), Never> {
    let Ok(measured) = measure(rest);

    let (consumed, found) = match measured {
      Measured::Found(consumed) => (consumed, Some(consumed)),
      Measured::Broken(consumed) => (consumed, None),
    };
    let end = at.saturating_add(1).saturating_add(consumed);
    let lo = expansion.value.span.lo;

    let decoded = match found {
      Some(length) => {
        let Ok(taken) = index(length);
        let encoding = match rest.get(..taken) {
          Some(encoding) => encoding,
          None => rest,
        };
        let Ok(decoded) = decode(encoding, colon, lo.saturating_add(end));
        decoded
      }
      None => None,
    };

    let invalid = Span {
      lo: lo.saturating_add(at),
      hi: lo.saturating_add(end),
    };

    let (encoding, piece) = match decoded {
      Some(encoding) => {
        let Ok(piece) = resolve(self.names, expansion.group, encoding);
        (encoding, piece)
      }
      None => {
        let Ok(()) = self.invalid(invalid);
        return Ok((end, Flushed::Retained));
      }
    };

    match piece {
      Piece::Name(_) | Piece::Number(_) => {}
      Piece::Skipped => return Ok((end, Flushed::Retained)),
      Piece::Invalid => {
        let Ok(()) = self.invalid(invalid);
        return Ok((end, Flushed::Retained));
      }
    }

    let Ok(flushed) = self.write(expansion, encoding, piece, at);
    Ok((end, flushed))
  }

  fn invalid(&mut self, span: Span) -> Result<(), Never> {
    self.diagnostics.push(
      self.map,
      DiagnosticKind::InvalidPercentEncoding,
      ad_hoc_display!("invalid percent encoding").spanned2(span),
    )
  }

  fn write(
    &mut self,
    expansion: &mut Expansion<'_>,
    encoding: PercentEncoding,
    piece: Piece,
    at: u32,
  ) -> Result<Flushed, Never> {
    match encoding.lead {
      Lead::MergeMode(merge_mode) => {
        let Ok(flushed) = self.flush(expansion, at);

        match flushed {
          Flushed::Retained => {}
          Flushed::Abandoned => return Ok(Flushed::Abandoned),
        }

        expansion.segment = Segment {
          merge_mode: Some(merge_mode),
          start: at,
        };
      }
      Lead::None | Lead::Prefix(_) => {}
    }

    match encoding.wrapping {
      Wrapping::Parenthesized => self.stash.push(b'('),
      Wrapping::Bare => {}
    }

    match encoding.lead {
      Lead::Prefix(prefix) => self.stash.push(prefix),
      Lead::None | Lead::MergeMode(_) => {}
    }

    match piece {
      Piece::Name(name) => {
        let Ok(spelled) = self.interner.get(name);
        self.stash.extend_from_slice(spelled);
      }
      Piece::Number(number) => self.stash.extend_from_slice(number.to_string().as_bytes()),
      Piece::Skipped | Piece::Invalid => {}
    }

    match encoding.wrapping {
      Wrapping::Parenthesized => self.stash.push(b')'),
      Wrapping::Bare => {}
    }

    Ok(Flushed::Retained)
  }

  fn flush(&mut self, expansion: &mut Expansion<'_>, end: u32) -> Result<Flushed, Never> {
    match ::core::primitive::bool::then_some(self.stash.is_empty(), ()) {
      ::core::option::Option::Some(()) => return Ok(Flushed::Retained),
      ::core::option::Option::None => {}
    }

    let segment = expansion.segment;

    match (segment.merge_mode, expansion.includes.front()) {
      (None, Some(first)) => match ::core::primitive::bool::then_some(first.val.mm.val == MergeMode::Include, ()) {
        ::core::option::Option::Some(()) => return Ok(Flushed::Abandoned),
        ::core::option::Option::None => {}
      },
      (None, None) | (Some(_), _) => {}
    }

    let span_lo = expansion.value.span.lo;
    let span = Span {
      lo: span_lo.saturating_add(segment.start),
      hi: span_lo.saturating_add(end),
    };

    match ::core::primitive::bool::then_some(self.stash.ends_with(b":all"), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.flush_every_group(expansion, span);
      }
      ::core::option::Option::None => {
        let Ok(()) = self.flush_once(expansion, segment.merge_mode, span);
      }
    }

    self.stash.clear();
    Ok(Flushed::Retained)
  }

  fn flush_every_group(
    &mut self,
    expansion: &mut Expansion<'_>,
    span: Span,
  ) -> Result<(), Never> {
    let Ok(length) = fitted::<_, u32>(self.stash.len());
    let kept = length.saturating_sub(3);
    let Ok(keep) = index(kept);
    let mut merge_mode = expansion.segment.merge_mode;

    for (number, _) in (1_u32..).zip(self.names.groups) {
      self.stash.truncate(keep);
      self.stash.extend_from_slice(number.to_string().as_bytes());
      let Ok(()) = self.flush_once(expansion, merge_mode, span);

      match merge_mode {
        Some(_) => {}
        None => {
          let lo = span.lo.saturating_add(kept);
          merge_mode = Some(MergeMode::Override.spanned(lo, lo.saturating_add(3)));
        }
      }
    }

    Ok(())
  }

  fn flush_once(
    &mut self,
    expansion: &mut Expansion<'_>,
    merge_mode: Option<Spanned<MergeMode>>,
    span: Span,
  ) -> Result<(), Never> {
    let Ok(code) = Code::owning(self.stash.to_vec());
    let Ok(code_span) = self.map.add(Some(self.path), Some(self.line), &code);
    let Ok(slice) = code.to_slice();
    let Ok(interned) = self.interner.intern(&slice);
    let path = interned.spanned2(code_span);

    match merge_mode {
      Some(mm) => {
        let include = Include {
          mm,
          path,
          loaded: None,
        };
        expansion.includes.push_back(include.spanned2(span));
      }
      None => {
        let mm = MergeMode::Include.spanned2(expansion.value.span);
        let include = Include {
          mm,
          path,
          loaded: None,
        };
        expansion.includes.push_front(include.spanned2(span));
      }
    }

    Ok(())
  }
}

fn after(bytes: &[u8], from: u32) -> Result<&[u8], Never> {
  let Ok(position) = index(from);

  Ok(match bytes.get(position..) {
    Some(rest) => rest,
    None => &[],
  })
}

fn closing(bytes: &[u8], wanted: u8) -> Result<Option<u32>, Never> {
  Ok(match bytes.iter().position(|b| *b == wanted) {
    Some(position) => {
      let Ok(at) = fitted::<_, u32>(position);
      Some(at)
    }
    None => None,
  })
}

fn measure(rest: &[u8]) -> Result<Measured, Never> {
  let Ok(length) = fitted::<_, u32>(rest.len());

  let (lead, after_lead) = match rest.split_first() {
    Some((lead, after_lead)) => (*lead, after_lead),
    None => return Ok(Measured::Broken(0)),
  };
  let (letter, consumed, after_letter) = match lead {
    b'(' => {
      let Ok(close) = closing(after_lead, b')');

      return Ok(match close {
        Some(at) => Measured::Found(at.saturating_add(2)),
        None => Measured::Broken(length),
      });
    }
    b'+' | b'|' | b'^' | b'-' | b'_' => match after_lead.split_first() {
      Some((letter, after_letter)) => (*letter, 2_u32, after_letter),
      None => return Ok(Measured::Broken(1)),
    },
    _ => (lead, 1_u32, after_lead),
  };

  Ok(match letter {
    b'i' | b'm' => Measured::Found(consumed),
    b'l' | b'v' => match after_letter.split_first() {
      Some((b'[', inside)) => {
        let Ok(close) = closing(inside, b']');

        match close {
          Some(at) => Measured::Found(consumed.saturating_add(at).saturating_add(2)),
          None => Measured::Broken(length),
        }
      }
      Some(_) | None => Measured::Found(consumed),
    },
    _ => Measured::Broken(consumed),
  })
}

fn decode(encoding: &[u8], colon: Colon, hi: SpanUnit) -> Result<Option<PercentEncoding>, Never> {
  let (wrapping, inner) = match encoding {
    [b'(', inner @ .., _] => (Wrapping::Parenthesized, inner),
    _ => (Wrapping::Bare, encoding),
  };

  let (first, after_first) = match inner.split_first() {
    Some((first, after_first)) => (*first, after_first),
    None => return Ok(None),
  };
  let merge_mode_at = |mode: MergeMode| Lead::MergeMode(mode.spanned(hi.saturating_sub(1), hi));
  let (lead, rest) = match first {
    b'|' => (merge_mode_at(MergeMode::Augment), after_first),
    b'+' => (merge_mode_at(MergeMode::Override), after_first),
    b'^' => (merge_mode_at(MergeMode::Replace), after_first),
    b'-' | b'_' => (Lead::Prefix(first), after_first),
    _ => (Lead::None, inner),
  };

  let (letter, after_letter) = match rest.split_first() {
    Some((letter, after_letter)) => (*letter, after_letter),
    None => return Ok(None),
  };
  let component = match (letter, colon) {
    (b'm', _) => Component::Model,
    (b'l', _) => Component::Layout,
    (b'v', _) => Component::Variant,
    (b'i', Colon::Present) => Component::Index,
    _ => return Ok(None),
  };

  let indexed = match (component, after_letter) {
    (_, []) => Indexed::No,
    (Component::Layout | Component::Variant, [b'[', inside @ .., b']']) => match inside {
      b"%i" => Indexed::ByMatch,
      number => {
        let numbered = match u32::from_bytes_dec(number) {
          Some(value) => {
            let Ok(group) = GroupIdx::new(value);
            group
          }
          None => None,
        };

        match numbered {
          Some(group) => Indexed::At(group),
          None => return Ok(None),
        }
      }
    },
    _ => return Ok(None),
  };

  Ok(Some(PercentEncoding {
    wrapping,
    lead,
    component,
    indexed,
  }))
}

fn resolve(
  names: &Names<'_>,
  expanding: Option<u32>,
  encoding: PercentEncoding,
) -> Result<Piece, Never> {
  let Ok(count) = fitted::<_, u32>(names.groups.len());

  let matched = match expanding {
    Some(at) => {
      let Ok(position) = index(at);
      names.groups.get(position)
    }
    None => None,
  };
  let (group, validity) = match encoding.indexed {
    Indexed::At(wanted) => {
      let Ok(raw) = wanted.raw();
      let Ok(position) = index(raw.saturating_sub(1));

      match names.groups.get(position) {
        Some(found) => (Some(found), Validity::Valid),
        None => (matched, Validity::Invalid),
      }
    }
    Indexed::ByMatch => (matched, Validity::Valid),
    Indexed::No => match (count > 1, encoding.component) {
      (true, Component::Layout | Component::Variant) => (matched, Validity::Invalid),
      (true, Component::Model | Component::Index) | (false, _) => (matched, Validity::Valid),
    },
  };
  let piece = match (encoding.component, expanding) {
    (Component::Index, Some(at)) => Piece::Number(at.saturating_add(1)),
    (Component::Index, None) => Piece::Invalid,
    (Component::Model, _) => match names.model {
      Some(model) => Piece::Name(model),
      None => Piece::Invalid,
    },
    (Component::Layout, _) => match group.and_then(|g| g.layout) {
      Some(layout) => Piece::Name(layout),
      None => Piece::Invalid,
    },
    (Component::Variant, _) => match group.and_then(|g| g.variant) {
      Some(variant) => Piece::Name(variant),
      None => Piece::Skipped,
    },
  };

  Ok(match (piece, validity) {
    (Piece::Skipped, _) => Piece::Skipped,
    (Piece::Name(_) | Piece::Number(_) | Piece::Invalid, Validity::Invalid) => Piece::Invalid,
    (Piece::Name(_) | Piece::Number(_) | Piece::Invalid, Validity::Valid) => piece,
  })
}

fn create_lexer(
  tables: &mut Tables<'_, '_, '_>,
  path: Spanned<Interned>,
) -> Result<Option<Lexer>, Never> {
  let Ok(found) = tables
    .loader
    .load(tables.interner, CodeType::Rules, path.val);
  let mut loaded = None;

  for candidate in found {
    match candidate {
      Ok(code) => {
        loaded = Some(code);
        break;
      }
      Err(e) => {
        let Ok(()) = tables.diagnostics.push(
          tables.map,
          DiagnosticKind::FileReadFailed,
          e.spanned2(path.span),
        );
      }
    }
  }

  let (real_path, code) = match loaded {
    Some(code) => code,
    None => {
      let Ok(()) = tables.diagnostics.push(
        tables.map,
        DiagnosticKind::FileNotFound,
        ad_hoc_display!("could not find file to include").spanned2(path.span),
      );
      return Ok(None);
    }
  };
  let Ok(span) = tables.map.add(Some(&real_path), Some(path.span), &code);
  let Ok(lexer) = Lexer::new(real_path, code, span.lo);
  Ok(Some(lexer))
}
