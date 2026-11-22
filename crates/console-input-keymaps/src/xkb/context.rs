use crate::config::DEFAULT_INCLUDE_DIR;
use crate::config::EXTRA_INCLUDE_DIR;
use crate::xkb::code::Code;
use crate::xkb::code_loader::CodeLoader;
use crate::xkb::code_map::CodeMap;
use crate::xkb::diagnostic::Diagnostic;
use crate::xkb::diagnostic::DiagnosticHandler;
use crate::xkb::diagnostic::DiagnosticKind;
use crate::xkb::diagnostic::DiagnosticSink;
use crate::xkb::interner::Interned;
use crate::xkb::interner::Interner;
use crate::xkb::kccgst;
use crate::xkb::kccgst::ast::Item;
use crate::xkb::kccgst::ast_cache::AstCache;
use crate::xkb::kccgst::embedder::embed;
use crate::xkb::kccgst::includer::IncludeLimits;
use crate::xkb::kccgst::includer::resolve_includes;
use crate::xkb::kccgst::parser::parse_item;
use crate::xkb::kccgst::resolver::resolve;
use crate::xkb::keymap::Keymap;
use crate::xkb::meaning::MeaningCache;
use crate::xkb::rmlvo;
use crate::xkb::rmlvo::resolver::Group;
use crate::xkb::rmlvo::resolver::Names;
use crate::xkb::rmlvo::resolver::Tables;
use crate::xkb::rmlvo::resolver::create_item;
use crate::xkb::span::SpanExt;
use crate::xkb::span::Spanned;
use crate::xkb::string_cooker::StringCooker;
use console_core_never::Never;
use console_input_keymaps_derive::ad_hoc_display;
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Context {
  pub(crate) paths: Vec<PathBuf>,
}

pub(crate) const MAX_INCLUDES: u64 = 1024;
pub(crate) const MAX_INCLUDE_DEPTH: u64 = 128;
pub(crate) const MAX_RUNTIME: u64 = 500_000;
const DEFAULT_MODEL: &str = "pc105";

#[derive(Default)]
pub struct ContextBuilder {
  prefix: Vec<PathBuf>,
}

struct Compilation {
  map: CodeMap,
  interner: Interner,
  cooker: StringCooker,
  ast_cache: AstCache,
  loader: CodeLoader,
  meaning_cache: MeaningCache,
  remaining_runtime: u64,
}

impl Default for Context {
  fn default() -> Self {
    let Ok(builder) = Self::builder();
    let Ok(context) = builder.build();
    context
  }
}

impl ContextBuilder {
  pub fn prepend_path(&mut self, path: &(impl AsRef<Path> + ?Sized)) -> Result<(), Never> {
    self.prefix.push(path.as_ref().to_path_buf());

    Ok(())
  }

  pub fn build(self) -> Result<Context, Never> {
    let defaults = [EXTRA_INCLUDE_DIR, DEFAULT_INCLUDE_DIR].map(PathBuf::from);
    Ok(Context {
      paths: self.prefix.into_iter().rev().chain(defaults).collect(),
    })
  }
}

impl Context {
  pub fn builder() -> Result<ContextBuilder, Never> {
    Ok(ContextBuilder::default())
  }

  pub fn keymap_from_names(
    &self,
    mut diagnostics: impl DiagnosticHandler,
    rules: &str,
    groups: &[rmlvo::Group<'_>],
  ) -> Result<Keymap, Never> {
    let Ok(mut sink) = DiagnosticSink::new(&mut diagnostics);
    self.keymap_from_names_(&mut sink, rules, groups)
  }

  fn compilation(&self) -> Result<Compilation, Never> {
    let Ok(loader) = CodeLoader::new(&self.paths);
    Ok(Compilation {
      map: CodeMap::default(),
      interner: Interner::default(),
      cooker: StringCooker::default(),
      ast_cache: AstCache::default(),
      loader,
      meaning_cache: MeaningCache::default(),
      remaining_runtime: MAX_RUNTIME,
    })
  }

  fn keymap_from_names_(
    &self,
    diagnostics: &mut DiagnosticSink,
    rules: &str,
    groups: &[rmlvo::Group<'_>],
  ) -> Result<Keymap, Never> {
    let Ok(mut compilation) = self.compilation();
    let Ok(rules_name) = compilation.intern(rules);
    let Ok(model) = compilation.intern(DEFAULT_MODEL);
    let Ok(named_groups) = compilation.intern_groups(groups);
    let names = Names {
      rules: rules_name,
      model: Some(model.val),
      options: &[],
      groups: &named_groups,
      max_includes: MAX_INCLUDES,
    };
    let mut tables = Tables {
      map: &mut compilation.map,
      interner: &mut compilation.interner,
      loader: &mut compilation.loader,
      diagnostics,
      meaning_cache: &mut compilation.meaning_cache,
      remaining_runtime: &mut compilation.remaining_runtime,
    };
    let Ok(mut item) = create_item(&mut tables, &names);
    compilation.handle_item(diagnostics, &mut item)
  }

  pub fn keymap_from_bytes(
    &self,
    mut diagnostics: impl DiagnosticHandler,
    path: Option<&Path>,
    kccgst: impl AsRef<[u8]>,
  ) -> Result<Keymap, Diagnostic> {
    let Ok(mut sink) = DiagnosticSink::new(&mut diagnostics);
    self.keymap_from_bytes_(&mut sink, path, kccgst.as_ref())
  }

  fn keymap_from_bytes_(
    &self,
    diagnostics: &mut DiagnosticSink,
    path: Option<&Path>,
    kccgst: &[u8],
  ) -> Result<Keymap, Diagnostic> {
    let Ok(mut compilation) = self.compilation();
    let Ok(code) = Code::owning(kccgst.to_vec());
    let Ok(span) = compilation.map.add(path, None, &code);
    let Ok(mut lexer) = kccgst::lexer::Lexer::new(code, span.lo);
    let mut tokens = vec![];

    match lexer.lex_item(&mut compilation.interner, &mut tokens) {
      Err(e) => {
        let Ok(diagnostic_kind) = e.val.diagnostic_kind();
        let Ok(push_fatal) = diagnostics.push_fatal(&mut compilation.map, diagnostic_kind, e);
        return Err(push_fatal);
      }
      Ok(()) => {}
    }

    match ::core::primitive::bool::then_some(tokens.is_empty(), ()) {
      ::core::option::Option::Some(()) => {
        let Ok(push_fatal) = diagnostics.push_fatal(
          &mut compilation.map,
          DiagnosticKind::UnexpectedEof,
          ad_hoc_display!("keymap is empty").spanned2(span),
        );
        return Err(push_fatal);
      }
      ::core::option::Option::None => {}
    }

    let parsed = parse_item(
      &mut compilation.map,
      diagnostics,
      &compilation.interner,
      &mut compilation.meaning_cache,
      &tokens,
      0,
      &mut compilation.remaining_runtime,
    );
    tokens.clear();

    let mut item = match parsed {
      Ok(p) => p,
      Err(e) => {
        let Ok(diagnostic_kind) = e.val.diagnostic_kind();
        let Ok(push_fatal) = diagnostics.push_fatal(&mut compilation.map, diagnostic_kind, e);
        return Err(push_fatal);
      }
    };
    let Ok(keymap) = compilation.handle_item(diagnostics, &mut item.val);
    Ok(keymap)
  }
}

impl Compilation {
  fn intern(&mut self, name: &str) -> Result<Spanned<Interned>, Never> {
    let Ok(code) = Code::owning(name.as_bytes().to_vec());
    let Ok(span) = self.map.add(None, None, &code);
    let Ok(to_slice) = code.to_slice();
    let Ok(interned) = self.interner.intern(&to_slice);
    Ok(interned.spanned2(span))
  }

  fn intern_unless_empty(&mut self, name: &str) -> Result<Option<Interned>, Never> {
    match ::core::primitive::bool::then_some(name.is_empty(), ()) {
      ::core::option::Option::Some(()) => Ok(None),
      ::core::option::Option::None => {
        let Ok(interned) = self.intern(name);
        Ok(Some(interned.val))
      }
    }
  }

  fn intern_groups(&mut self, groups: &[rmlvo::Group<'_>]) -> Result<Vec<Group>, Never> {
    let mut named = Vec::with_capacity(groups.len());

    for group in groups {
      let Ok(layout) = self.intern_unless_empty(group.layout);
      let Ok(variant) = self.intern_unless_empty(group.variant);
      named.push(Group { layout, variant });
    }

    Ok(named)
  }

  fn handle_item(
    &mut self,
    diagnostics: &mut DiagnosticSink<'_, '_>,
    item: &mut Item,
  ) -> Result<Keymap, Never> {
    let mut tables = kccgst::ast_cache::Tables {
      diagnostics,
      map: &mut self.map,
      loader: &mut self.loader,
      interner: &mut self.interner,
      meaning_cache: &mut self.meaning_cache,
    };
    let limits = IncludeLimits {
      includes: MAX_INCLUDES,
      depth: MAX_INCLUDE_DEPTH,
    };
    let Ok(()) = resolve_includes(
      &mut tables,
      &mut self.ast_cache,
      &mut self.remaining_runtime,
      item,
      limits,
    );
    let Ok(()) = embed(item);
    let Ok(resolved) = resolve(
      &mut self.map,
      diagnostics,
      &mut self.interner,
      &mut self.meaning_cache,
      &mut self.cooker,
      item,
    );
    Keymap::from_resolved(&self.interner, &resolved)
  }
}
