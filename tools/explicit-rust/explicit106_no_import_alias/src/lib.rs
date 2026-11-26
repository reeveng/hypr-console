//! EXPLICIT106 keeps an imported item's name consistent across modules.
//!
//! Use qualified paths when names collide. Where a collision requires an
//! alias, put a reasoned allow on the individual import.
#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_ast;
extern crate rustc_span;

use clippy_utils::diagnostics::span_lint_and_help;
use rustc_ast::ast::{Item, ItemKind, UseTree, UseTreeKind};
use rustc_lint::{EarlyContext, EarlyLintPass};
use rustc_span::{Ident, Symbol, kw};

dylint_linting::declare_early_lint! {
    /// EXPLICIT106: preserve the names of imported items and crates.
    pub EXPLICIT106_NO_IMPORT_ALIAS,
    Deny,
    "an import renamed with `as`"
}

fn check_alias(cx: &EarlyContext<'_>, original: Option<Symbol>, alias: Ident) {
  let (message, help) = if original == Some(alias.name) {
    (
      format!("this import already has the name `{alias}`"),
      "remove the redundant `as` clause",
    )
  } else {
    (
      format!("this import is renamed to `{alias}`"),
      "use the original name or a qualified path. If names collide and an alias is needed, \
       add an `allow` on this import with a reason naming the conflict",
    )
  };

  span_lint_and_help(
    cx,
    EXPLICIT106_NO_IMPORT_ALIAS,
    alias.span,
    message,
    None,
    help,
  );
}

fn check_use_tree(cx: &EarlyContext<'_>, tree: &UseTree, parent_name: Option<Symbol>) {
  let name = match tree
    .prefix
    .segments
    .last()
    .map(|segment| segment.ident.name)
  {
    None | Some(kw::SelfLower) => parent_name,
    name => name,
  };

  match &tree.kind {
    UseTreeKind::Simple(Some(alias)) => check_alias(cx, name, *alias),
    UseTreeKind::Nested { items, .. } => {
      for (child, _) in items {
        check_use_tree(cx, child, name);
      }
    }
    UseTreeKind::Simple(None) | UseTreeKind::Glob(_) => {}
  }
}

impl EarlyLintPass for Explicit106NoImportAlias {
  fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
    if item.span.from_expansion() {
      return;
    }

    match &item.kind {
      ItemKind::Use(tree) => check_use_tree(cx, tree, None),
      ItemKind::ExternCrate(Some(original), alias) => check_alias(cx, Some(*original), *alias),
      _ => {}
    }
  }
}

#[test]
fn ui() {
  dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
