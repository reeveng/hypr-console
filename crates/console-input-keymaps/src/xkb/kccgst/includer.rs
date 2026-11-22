//! An include is resolved by loading what it names and then resolving the
//! includes of that, which kbvm wrote as two functions calling each other. The
//! walk keeps its own stack instead: the root item at the bottom, one frame
//! above it for every item loaded and not yet finished, and each frame holding
//! where it is in its own includes. An item loaded here is embedded as soon as
//! its own includes are finished, so the embedder only ever has one level left
//! to do.

use crate::xkb::code_loader::CodeLoader;
use crate::xkb::code_loader::CodeType;
use crate::xkb::code_map::CodeMap;
use crate::xkb::diagnostic::DiagnosticKind;
use crate::xkb::diagnostic::DiagnosticSink;
use crate::xkb::include::IncludeGroup;
use crate::xkb::include::IncludeIter;
use crate::xkb::include::parse_include;
use crate::xkb::interner::Interned;
use crate::xkb::interner::Interner;
use crate::xkb::kccgst::MergeMode;
use crate::xkb::kccgst::ast::ConfigItemType;
use crate::xkb::kccgst::ast::Decls;
use crate::xkb::kccgst::ast::DirectOrIncluded;
use crate::xkb::kccgst::ast::Include;
use crate::xkb::kccgst::ast::Including;
use crate::xkb::kccgst::ast::Item;
use crate::xkb::kccgst::ast::ItemType;
use crate::xkb::kccgst::ast::LoadedInclude;
use crate::xkb::kccgst::ast_cache::AstCache;
use crate::xkb::kccgst::ast_cache::Tables;
use crate::xkb::kccgst::embedder::embed;
use crate::xkb::meaning::MeaningCache;
use crate::xkb::span::SpanExt;
use crate::xkb::span::Spanned;
use console_core_never::Never;
use console_core_number_conversion::fitted;
use console_input_keymaps_derive::ad_hoc_display;
use hashbrown::HashSet;
use std::collections::VecDeque;

#[derive(Copy, Clone)]
pub(crate) struct IncludeLimits {
  pub(crate) includes: u64,
  pub(crate) depth: u64,
}

pub(crate) fn resolve_includes(
  tables: &mut Tables<'_, '_, '_>,
  cache: &mut AstCache,
  remaining_runtime: &mut u64,
  item: &mut Item,
  limits: IncludeLimits,
) -> Result<(), Never> {
  let includer = Includer {
    diagnostics: tables.diagnostics,
    map: tables.map,
    cache,
    loader: tables.loader,
    meaning_cache: tables.meaning_cache,
    remaining_runtime,
    active_includes: Default::default(),
    num_includes: 0,
    max_includes: limits.includes,
    max_include_depth: limits.depth,
  };
  let Ok(resolving) = Resolving::of(item);
  let walk = Walk {
    includer,
    interner: tables.interner,
    root: item,
    resolving,
    loaded: vec![],
  };

  for () in walk {}

  Ok(())
}

type IncludeKey = (CodeType, Interned, Option<Interned>);

type Entry = <IncludeIter<'static> as Iterator>::Item;

struct Includer<'a, 'b, 'c> {
  diagnostics: &'a mut DiagnosticSink<'b, 'c>,
  map: &'a mut CodeMap,
  cache: &'a mut AstCache,
  loader: &'a mut CodeLoader,
  meaning_cache: &'a mut MeaningCache,
  remaining_runtime: &'a mut u64,
  active_includes: HashSet<IncludeKey>,
  num_includes: u64,
  max_includes: u64,
  max_include_depth: u64,
}

struct Walk<'a, 'b, 'c> {
  includer: Includer<'a, 'b, 'c>,
  interner: &'a mut Interner,
  root: &'a mut Item,
  resolving: Resolving,
  loaded: Vec<Loaded>,
}

struct Resolving {
  includes: VecDeque<Unresolved>,
  results: Vec<Option<Box<[LoadedInclude]>>>,
  loading: Option<Loading>,
}

#[derive(Copy, Clone)]
struct Unresolved {
  path: Spanned<Interned>,
  ty: CodeType,
}

struct Loading {
  ty: CodeType,
  entries: VecDeque<Entry>,
  resolved: Vec<LoadedInclude>,
}

struct Loaded {
  item: Spanned<Item>,
  merge_mode: Option<Spanned<MergeMode>>,
  group: Option<IncludeGroup>,
  key: IncludeKey,
  resolving: Resolving,
  parent: Loading,
}

enum Stepped {
  Went,
  Descended(Box<Loaded>),
  Finished,
}

impl Iterator for Walk<'_, '_, '_> {
  type Item = ();

  fn next(&mut self) -> Option<()> {
    let Ok(above) = fitted::<_, u64>(self.loaded.len());
    let depth = above.saturating_add(1);
    let (item, resolving) = match self.loaded.last_mut() {
      Some(top) => (&mut top.item.val, &mut top.resolving),
      None => (&mut *self.root, &mut self.resolving),
    };
    let Ok(stepped) = self.includer.step(self.interner, depth, item, resolving);

    match stepped {
      Stepped::Went => Some(()),
      Stepped::Descended(loaded) => {
        self.loaded.push(*loaded);
        Some(())
      }
      Stepped::Finished => {
        let finished = self.loaded.pop()?;
        let Ok(()) = self.deliver(finished);
        Some(())
      }
    }
  }
}

impl Walk<'_, '_, '_> {
  fn deliver(&mut self, finished: Loaded) -> Result<(), Never> {
    let Loaded {
      mut item,
      merge_mode,
      group,
      key,
      mut parent,
      ..
    } = finished;
    let Ok(()) = embed(&mut item.val);
    parent.resolved.push(LoadedInclude {
      mm: merge_mode,
      group,
      item,
    });
    self.includer.active_includes.remove(&key);
    let below = match self.loaded.last_mut() {
      Some(top) => &mut top.resolving,
      None => &mut self.resolving,
    };
    below.loading = Some(parent);

    Ok(())
  }
}

impl Resolving {
  fn of(item: &mut Item) -> Result<Self, Never> {
    let mut found = vec![];
    let Ok(()) = includes_of(item, &mut found);
    let includes = found
      .into_iter()
      .map(|(include, ty)| Unresolved {
        path: include.path,
        ty,
      })
      .collect();

    Ok(Self {
      includes,
      results: vec![],
      loading: None,
    })
  }

  fn finish(&mut self, item: &mut Item) -> Result<(), Never> {
    let mut found = vec![];
    let Ok(()) = includes_of(item, &mut found);

    for ((include, _ty), loaded) in found.into_iter().zip(self.results.drain(..)) {
      include.loaded = loaded;
    }

    Ok(())
  }
}

impl Includer<'_, '_, '_> {
  fn step(
    &mut self,
    interner: &mut Interner,
    depth: u64,
    item: &mut Item,
    resolving: &mut Resolving,
  ) -> Result<Stepped, Never> {
    match resolving.loading.take() {
      Some(loading) => self.take_entry(interner, loading, resolving),
      None => match resolving.includes.pop_front() {
        Some(unresolved) => {
          let Ok(()) = self.start(interner, depth, unresolved, resolving);

          Ok(Stepped::Went)
        }
        None => {
          let Ok(()) = resolving.finish(item);

          Ok(Stepped::Finished)
        }
      },
    }
  }

  fn start(
    &mut self,
    interner: &mut Interner,
    depth: u64,
    unresolved: Unresolved,
    resolving: &mut Resolving,
  ) -> Result<(), Never> {
    match ::core::primitive::bool::then_some(depth >= self.max_include_depth, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.diagnostics.push(
          self.map,
          DiagnosticKind::MaxIncludeDepthReached,
          ad_hoc_display!("maximum include depth ({}) reached", self.max_include_depth => u64)
            .spanned2(unresolved.path.span),
        );
        resolving.results.push(None);
      }
      ::core::option::Option::None => {
        let Ok(parsed) = parse_include(interner, unresolved.path);
        resolving.loading = Some(Loading {
          ty: unresolved.ty,
          entries: parsed.collect(),
          resolved: vec![],
        });
      }
    }

    Ok(())
  }

  fn take_entry(
    &mut self,
    interner: &mut Interner,
    mut loading: Loading,
    resolving: &mut Resolving,
  ) -> Result<Stepped, Never> {
    let wanted = match loading.entries.pop_front() {
      Some(Ok(wanted)) => wanted,
      Some(Err(e)) => {
        let Ok(diagnostic_kind) = e.val.diagnostic_kind();
        let Ok(()) = self.diagnostics.push(self.map, diagnostic_kind, e);
        resolving.loading = Some(loading);

        return Ok(Stepped::Went);
      }
      None => {
        resolving
          .results
          .push(Some(loading.resolved.into_boxed_slice()));

        return Ok(Stepped::Went);
      }
    };
    self.num_includes = self.num_includes.saturating_add(1);

    match ::core::primitive::bool::then_some(self.num_includes > self.max_includes, ()) {
      ::core::option::Option::Some(()) => {
        let Ok(()) = self.diagnostics.push(
          self.map,
          DiagnosticKind::MaxIncludesReached,
          ad_hoc_display!("maximum number of includes reached").spanned2(wanted.file.span),
        );
        resolving
          .results
          .push(Some(loading.resolved.into_boxed_slice()));

        return Ok(Stepped::Went);
      }
      ::core::option::Option::None => {}
    }

    let mut span = wanted.file.span;

    match wanted.map {
      Some(map) => {
        span.hi = map.span.hi.saturating_add(1);
      }
      None => {}
    }

    let file_map = wanted.map.map(|m| m.val);
    let key = (loading.ty, wanted.file.val, file_map);

    match ::core::primitive::bool::then_some(self.active_includes.insert(key), ()) {
      ::core::option::Option::Some(()) => {}
      ::core::option::Option::None => {
        let Ok(()) = self.diagnostics.push(
          self.map,
          DiagnosticKind::RecursiveInclude,
          ad_hoc_display!("ignoring recursive include").spanned2(span),
        );
        resolving.loading = Some(loading);

        return Ok(Stepped::Went);
      }
    }

    let mut tables = Tables {
      diagnostics: self.diagnostics,
      map: self.map,
      loader: self.loader,
      interner,
      meaning_cache: self.meaning_cache,
    };
    let res = self.cache.get(
      &mut tables,
      self.remaining_runtime,
      loading.ty,
      wanted.file.val,
      file_map,
      span,
    );

    Ok(match res {
      Ok(mut item) => {
        let Ok(child) = Resolving::of(&mut item.val);

        Stepped::Descended(Box::new(Loaded {
          item,
          merge_mode: wanted.merge_mode,
          group: wanted.group,
          key,
          resolving: child,
          parent: loading,
        }))
      }
      Err(e) => {
        let Ok(diagnostic_kind) = e.val.diagnostic_kind();
        let Ok(()) = self.diagnostics.push(self.map, diagnostic_kind, e);
        self.active_includes.remove(&key);
        resolving.loading = Some(loading);

        Stepped::Went
      }
    })
  }
}

fn includes_of<'a>(
  item: &'a mut Item,
  found: &mut Vec<(&'a mut Include, CodeType)>,
) -> Result<(), Never> {
  match &mut item.ty {
    ItemType::Composite(c) => {
      for nested in &mut c.config_items {
        let Ok(()) = includes_in(&mut nested.val.item.specific, found);
      }
    }
    ItemType::Configuration(c) => {
      let Ok(()) = includes_in(c, found);
    }
  }

  Ok(())
}

fn includes_in<'a>(
  ty: &'a mut ConfigItemType,
  found: &mut Vec<(&'a mut Include, CodeType)>,
) -> Result<(), Never> {
  match ty {
    ConfigItemType::Keycodes(e) => includes_among(&mut e.decls, CodeType::Keycodes, found),
    ConfigItemType::Types(e) => includes_among(&mut e.decls, CodeType::Types, found),
    ConfigItemType::Compat(e) => includes_among(&mut e.decls, CodeType::Compat, found),
    ConfigItemType::Symbols(e) => includes_among(&mut e.decls, CodeType::Symbols, found),
    ConfigItemType::Geometry(e) => includes_among(&mut e.decls, CodeType::Geometry, found),
  }
}

fn includes_among<'a, T: Including>(
  decls: &'a mut Decls<T>,
  ty: CodeType,
  found: &mut Vec<(&'a mut Include, CodeType)>,
) -> Result<(), Never> {
  for decl in &mut decls.decls {
    match &mut decl.val.ty {
      DirectOrIncluded::Direct(d) => match d.include() {
        Some(include) => found.push((include, ty)),
        None => {}
      },
      DirectOrIncluded::Included(_) => {}
    }
  }

  Ok(())
}
