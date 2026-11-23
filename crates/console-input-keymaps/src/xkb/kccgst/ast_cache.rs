mod error;

use crate::xkb::clone_with_delta::CloneWithDelta;
use crate::xkb::code_loader::CodeIter;
use crate::xkb::code_loader::CodeLoader;
use crate::xkb::code_loader::CodeType;
use crate::xkb::code_map::CodeMap;
use crate::xkb::diagnostic::DiagnosticKind;
use crate::xkb::diagnostic::DiagnosticSink;
use crate::xkb::interner::Interned;
use crate::xkb::interner::Interner;
use crate::xkb::kccgst::ast::Item;
use crate::xkb::kccgst::ast_cache::error::AstCacheError;
use crate::xkb::kccgst::ast_cache::error::not_found;
use crate::xkb::kccgst::lexer::Lexer;
use crate::xkb::kccgst::parser::parse_item;
use crate::xkb::kccgst::parser::snoop_ty_and_name;
use crate::xkb::kccgst::token::Token;
use crate::xkb::meaning::MeaningCache;
use crate::xkb::span::Span;
use crate::xkb::span::SpanExt;
use crate::xkb::span::Spanned;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_core_number_conversion::index;
use console_input_keymaps_derive::ad_hoc_display;
use hashbrown::HashMap;
use hashbrown::hash_map::Entry;
use std::collections::VecDeque;
use std::ops::ControlFlow;

#[derive(Default)]
pub(crate) struct AstCache {
  files: HashMap<FileKey, Value>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct FileKey {
  ty: CodeType,
  file: Interned,
}

#[derive(Debug)]
struct SeenMap {
  span: Span,
  tokens_or_item: TokensOrItem,
}

#[derive(Debug)]
enum TokensOrItem {
  Invalid,
  Tokens(Vec<Spanned<Token>>),
  Item(Spanned<Item>),
}

#[derive(Debug)]
struct Value {
  lexers: VecDeque<Lexer>,
  maps: Vec<SeenMap>,
  map_names: HashMap<MapName, u32>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
enum MapName {
  Unnamed,
  Default,
  Named(Interned),
}

pub(crate) struct Tables<'a, 'b, 'c> {
  pub(crate) diagnostics: &'a mut DiagnosticSink<'b, 'c>,
  pub(crate) map: &'a mut CodeMap,
  pub(crate) loader: &'a mut CodeLoader,
  pub(crate) interner: &'a mut Interner,
  pub(crate) meaning_cache: &'a mut MeaningCache,
}

#[derive(Copy, Clone)]
struct Wanted {
  key: FileKey,
  file_map: Option<Interned>,
  include_span: Span,
  desired_map: MapName,
}

#[derive(Copy, Clone)]
enum Fallback {
  Unreported,
  Reported,
}

impl AstCache {
  pub(crate) fn get(
    &mut self,
    tables: &mut Tables<'_, '_, '_>,
    remaining_runtime: &mut u64,
    ty: CodeType,
    file: Interned,
    file_map: Option<Interned>,
    include_span: Span,
  ) -> Result<Spanned<Item>, Spanned<AstCacheError>> {
    let key = FileKey { ty, file };
    let value = match self.files.entry(key) {
      Entry::Occupied(known) => known.into_mut(),
      Entry::Vacant(first) => {
        let Ok(loaded) = tables.loader.load(tables.interner, ty, file);
        let Ok(lexers) = lexers_of(loaded, tables.diagnostics, tables.map, include_span);
        first.insert(Value {
          lexers,
          maps: Vec::new(),
          map_names: HashMap::default(),
        })
      }
    };
    let desired_map = match file_map {
      Some(named) => MapName::Named(named),
      None => MapName::Default,
    };
    let wanted = Wanted {
      key,
      file_map,
      include_span,
      desired_map,
    };
    let mut fallback = Fallback::Unreported;
    let Ok(attempts) = attempts_at_most(value);

    for _attempt in 0..attempts {
      let at = chosen(tables, wanted, value, &mut fallback)?;
      let Ok(found) = attempt(tables, include_span, value, at, remaining_runtime);

      match found {
        Some(item) => {
          return Ok(item);
        }
        None => {}
      }
    }

    let Ok(not_found) = not_found(tables.interner, file, file_map, include_span);
    Err(not_found)
  }
}

fn lexers_of(
  loaded: CodeIter<'_>,
  diagnostics: &mut DiagnosticSink,
  map: &mut CodeMap,
  include_span: Span,
) -> Result<VecDeque<Lexer>, Never> {
  let mut lexers = VecDeque::new();

  for one in loaded {
    match one {
      Ok((path, code)) => {
        let Ok(span) = map.add(Some(&path), Some(include_span), &code);
        let Ok(lexer) = Lexer::new(code, span.lo);
        lexers.push_back(lexer);
      }
      Err(e) => {
        let Ok(()) = diagnostics.push(
          map,
          DiagnosticKind::FileReadFailed,
          e.spanned2(include_span),
        );
      }
    }
  }

  Ok(lexers)
}

fn attempts_at_most(value: &Value) -> Result<u64, Never> {
  let lexable = value.lexers.iter().fold(0_u64, |total, lexer| {
    let Ok(code) = lexer.code();
    let Ok(length) = fitted::<_, u64>(code.len());
    total.saturating_add(length).saturating_add(1)
  });
  let Ok(seen) = fitted::<_, u64>(value.maps.len());

  Ok(lexable.saturating_add(seen).saturating_add(1))
}

fn attempt(
  tables: &mut Tables<'_, '_, '_>,
  include_span: Span,
  value: &mut Value,
  at: u32,
  remaining_runtime: &mut u64,
) -> Result<Option<Spanned<Item>>, Never> {
  let Ok(position) = index(at);
  let entry = match value.maps.get_mut(position) {
    Some(entry) => entry,
    None => {
      return Ok(None);
    }
  };
  let Ok(new_span) = tables.map.repeat(entry.span, include_span);
  let delta = new_span.lo.saturating_sub(entry.span.lo);

  Ok(match &entry.tokens_or_item {
    TokensOrItem::Invalid => None,
    TokensOrItem::Item(item) => Some(item.clone_with_delta(delta)),
    TokensOrItem::Tokens(tokens) => {
      let parsed = parse_item(
        tables.map,
        tables.diagnostics,
        tables.interner,
        tables.meaning_cache,
        tokens,
        delta,
        remaining_runtime,
      );

      match parsed {
        Ok(item) => {
          let moved = item.clone_with_delta(delta);
          entry.tokens_or_item = TokensOrItem::Item(item);
          Some(moved)
        }
        Err(e) => {
          let Ok(diagnostic_kind) = e.val.diagnostic_kind();
          let Ok(()) = tables.diagnostics.push(tables.map, diagnostic_kind, e);
          entry.tokens_or_item = TokensOrItem::Invalid;
          None
        }
      }
    }
  })
}

fn chosen(
  tables: &mut Tables<'_, '_, '_>,
  wanted: Wanted,
  value: &mut Value,
  fallback: &mut Fallback,
) -> Result<u32, Spanned<AstCacheError>> {
  let Ok(remembered) = remembered(value, wanted.desired_map);

  match remembered {
    Some(at) => {
      return Ok(at);
    }
    None => {}
  }

  let Ok(lexed) = lexed(tables, wanted, value);

  match lexed {
    Some(at) => {
      return Ok(at);
    }
    None => {}
  }

  let Ok(first) = first_instead(tables, wanted, value, fallback);

  match first {
    Some(at) => Ok(at),
    None => {
      let Ok(not_found) = not_found(
        tables.interner,
        wanted.key.file,
        wanted.file_map,
        wanted.include_span,
      );
      Err(not_found)
    }
  }
}

fn remembered(value: &mut Value, desired: MapName) -> Result<Option<u32>, Never> {
  let known = match value.map_names.entry(desired) {
    Entry::Occupied(known) => known,
    Entry::Vacant(_) => {
      return Ok(None);
    }
  };
  let Ok(position) = index(*known.get());

  Ok(
    match value.maps.get(position).map(|seen| &seen.tokens_or_item) {
      Some(TokensOrItem::Tokens(_) | TokensOrItem::Item(_)) => Some(*known.get()),
      Some(TokensOrItem::Invalid) | None => {
        known.remove();
        None
      }
    },
  )
}

fn lexed(
  tables: &mut Tables<'_, '_, '_>,
  wanted: Wanted,
  value: &mut Value,
) -> Result<Option<u32>, Never> {
  let Ok(steps) = attempts_at_most(value);

  for _step in 0..steps {
    let Ok(next) = next_map(tables, wanted, value);

    match next {
      ControlFlow::Break(found) => {
        return Ok(found);
      }
      ControlFlow::Continue(()) => {}
    }
  }

  Ok(None)
}

fn next_map(
  tables: &mut Tables<'_, '_, '_>,
  wanted: Wanted,
  value: &mut Value,
) -> Result<ControlFlow<Option<u32>>, Never> {
  let lexer = match value.lexers.front_mut() {
    Some(lexer) => lexer,
    None => {
      return Ok(ControlFlow::Break(None));
    }
  };
  let mut tokens = vec![];
  let lexed = lexer.lex_item(tables.interner, &mut tokens);

  match lexed {
    Ok(()) => {}
    Err(e) => {
      value.lexers.pop_front();
      let Ok(diagnostic_kind) = e.val.diagnostic_kind();
      let Ok(()) = tables.diagnostics.push(tables.map, diagnostic_kind, e);
      return Ok(ControlFlow::Continue(()));
    }
  }

  match ::core::primitive::bool::then_some(tokens.is_empty(), ()) {
    ::core::option::Option::Some(()) => {
      value.lexers.pop_front();
      return Ok(ControlFlow::Continue(()));
    }
    ::core::option::Option::None => {}
  }

  let snooped = match snoop_ty_and_name(&tokens, tables.interner, tables.meaning_cache) {
    Ok(snooped) => snooped,
    Err(e) => {
      let Ok(diagnostic_kind) = e.val.diagnostic_kind();
      let Ok(()) = tables.diagnostics.push(tables.map, diagnostic_kind, e);
      return Ok(ControlFlow::Continue(()));
    }
  };
  let (default, ty, name) = snooped;

  match ::core::primitive::bool::then_some(ty.val == wanted.key.ty, ()) {
    ::core::option::Option::Some(()) => {}
    ::core::option::Option::None => {
      let Ok(()) = tables.diagnostics.push(
        tables.map,
        DiagnosticKind::UnexpectedItemType,
        ad_hoc_display!("unexpected item type").spanned2(ty.span),
      );
      return Ok(ControlFlow::Continue(()));
    }
  }

  let Ok(lexer_span) = lexer.span();
  let seen_map = SeenMap {
    span: lexer_span,
    tokens_or_item: TokensOrItem::Tokens(tokens),
  };
  let Ok(at) = fitted::<_, u32>(value.maps.len());
  value.maps.push(seen_map);
  let Ok(()) = remember_default(tables, value, at, default);
  let map_name = match name {
    Some(named) => MapName::Named(named.val),
    None => MapName::Unnamed,
  };

  match value.map_names.entry(map_name) {
    Entry::Occupied(_) => {
      let span = match name {
        Some(named) => named.span,
        None => ty.span,
      };
      let Ok(()) = tables.diagnostics.push(
        tables.map,
        DiagnosticKind::DuplicateItemName,
        ad_hoc_display!("duplicate item name in file").spanned2(span),
      );
    }
    Entry::Vacant(vacant) => {
      vacant.insert(at);
    }
  }

  Ok(match (default, wanted.desired_map) {
    (Some(_), MapName::Default) => ControlFlow::Break(Some(at)),
    (Some(_) | None, desired) => {
      match ::core::primitive::bool::then_some(desired == map_name, ()) {
        ::core::option::Option::Some(()) => ControlFlow::Break(Some(at)),
        ::core::option::Option::None => ControlFlow::Continue(()),
      }
    }
  })
}

fn remember_default(
  tables: &mut Tables<'_, '_, '_>,
  value: &mut Value,
  at: u32,
  default: Option<Span>,
) -> Result<(), Never> {
  let span = match default {
    Some(span) => span,
    None => {
      return Ok(());
    }
  };

  match value.map_names.entry(MapName::Default) {
    Entry::Occupied(_) => {
      let Ok(()) = tables.diagnostics.push(
        tables.map,
        DiagnosticKind::MultipleDefaultItems,
        ad_hoc_display!("file contains multiple default items").spanned2(span),
      );
    }
    Entry::Vacant(vacant) => {
      vacant.insert(at);
    }
  }

  Ok(())
}

fn first_instead(
  tables: &mut Tables<'_, '_, '_>,
  wanted: Wanted,
  value: &Value,
  fallback: &mut Fallback,
) -> Result<Option<u32>, Never> {
  match wanted.desired_map {
    MapName::Default => {}
    MapName::Unnamed | MapName::Named(_) => {
      return Ok(None);
    }
  }

  let first = (0_u32..).zip(value.maps.iter()).find(|(_, seen)| {
    matches!(
      seen.tokens_or_item,
      TokensOrItem::Tokens(_) | TokensOrItem::Item(_)
    )
  });
  let at = match first {
    Some((at, _)) => at,
    None => {
      return Ok(None);
    }
  };

  match *fallback {
    Fallback::Reported => {}
    Fallback::Unreported => {
      *fallback = Fallback::Reported;
      let Ok(()) = tables.diagnostics.push(
        tables.map,
        DiagnosticKind::UsingFirstInsteadOfDefault,
        ad_hoc_display!("default map not found, using first instead")
          .spanned2(wanted.include_span),
      );
    }
  }

  Ok(Some(at))
}
