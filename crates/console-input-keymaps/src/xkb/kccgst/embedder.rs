//! An include that was loaded becomes the declarations it loaded, as a list
//! of components in place of the include. kbvm did this as a walk that
//! embedded each loaded item's own includes on the way down; the includer now
//! embeds every item it loads once that item's includes are finished, so what
//! arrives here is one level deep and is taken as it stands.

use crate::xkb::kccgst::ast::CompatmapDecl;
use crate::xkb::kccgst::ast::Component;
use crate::xkb::kccgst::ast::ConfigItemType;
use crate::xkb::kccgst::ast::Decl;
use crate::xkb::kccgst::ast::Decls;
use crate::xkb::kccgst::ast::DirectOrIncluded;
use crate::xkb::kccgst::ast::GeometryDecl;
use crate::xkb::kccgst::ast::Included;
use crate::xkb::kccgst::ast::Including;
use crate::xkb::kccgst::ast::Item;
use crate::xkb::kccgst::ast::ItemType;
use crate::xkb::kccgst::ast::KeycodeDecl;
use crate::xkb::kccgst::ast::LoadedInclude;
use crate::xkb::kccgst::ast::SymbolsDecl;
use crate::xkb::kccgst::ast::TypesDecl;
use crate::xkb::span::Spanned;
use console_core_never::Never;

pub(crate) fn embed(item: &mut Item) -> Result<(), Never> {
  match &mut item.ty {
    ItemType::Composite(e) => {
      for nested in &mut e.config_items {
        let Ok(()) = embed_config_item_type(&mut nested.val.item.specific);
      }
    }
    ItemType::Configuration(e) => {
      let Ok(()) = embed_config_item_type(e);
    }
  }

  Ok(())
}

trait Embeddable: Including {
  fn decls_of(item: ConfigItemType) -> Box<[Spanned<Decl<Self>>]>;
}

impl Embeddable for KeycodeDecl {
  fn decls_of(item: ConfigItemType) -> Box<[Spanned<Decl<Self>>]> {
    match item {
      ConfigItemType::Keycodes(e) => e.decls.decls,
      ConfigItemType::Types(_)
      | ConfigItemType::Compat(_)
      | ConfigItemType::Symbols(_)
      | ConfigItemType::Geometry(_) => Box::default(),
    }
  }
}

impl Embeddable for TypesDecl {
  fn decls_of(item: ConfigItemType) -> Box<[Spanned<Decl<Self>>]> {
    match item {
      ConfigItemType::Types(e) => e.decls.decls,
      ConfigItemType::Keycodes(_)
      | ConfigItemType::Compat(_)
      | ConfigItemType::Symbols(_)
      | ConfigItemType::Geometry(_) => Box::default(),
    }
  }
}

impl Embeddable for CompatmapDecl {
  fn decls_of(item: ConfigItemType) -> Box<[Spanned<Decl<Self>>]> {
    match item {
      ConfigItemType::Compat(e) => e.decls.decls,
      ConfigItemType::Keycodes(_)
      | ConfigItemType::Types(_)
      | ConfigItemType::Symbols(_)
      | ConfigItemType::Geometry(_) => Box::default(),
    }
  }
}

impl Embeddable for SymbolsDecl {
  fn decls_of(item: ConfigItemType) -> Box<[Spanned<Decl<Self>>]> {
    match item {
      ConfigItemType::Symbols(e) => e.decls.decls,
      ConfigItemType::Keycodes(_)
      | ConfigItemType::Types(_)
      | ConfigItemType::Compat(_)
      | ConfigItemType::Geometry(_) => Box::default(),
    }
  }
}

impl Embeddable for GeometryDecl {
  fn decls_of(item: ConfigItemType) -> Box<[Spanned<Decl<Self>>]> {
    match item {
      ConfigItemType::Geometry(e) => e.decls.decls,
      ConfigItemType::Keycodes(_)
      | ConfigItemType::Types(_)
      | ConfigItemType::Compat(_)
      | ConfigItemType::Symbols(_) => Box::default(),
    }
  }
}

fn embed_config_item_type(item: &mut ConfigItemType) -> Result<(), Never> {
  match item {
    ConfigItemType::Keycodes(e) => embed_decls(&mut e.decls),
    ConfigItemType::Types(e) => embed_decls(&mut e.decls),
    ConfigItemType::Compat(e) => embed_decls(&mut e.decls),
    ConfigItemType::Symbols(e) => embed_decls(&mut e.decls),
    ConfigItemType::Geometry(e) => embed_decls(&mut e.decls),
  }
}

fn embed_decls<T: Embeddable>(decls: &mut Decls<T>) -> Result<(), Never> {
  for decl in &mut decls.decls {
    let taken = match &mut decl.val.ty {
      DirectOrIncluded::Direct(d) => match d.include() {
        Some(include) => include.loaded.take(),
        None => None,
      },
      DirectOrIncluded::Included(_) => None,
    };

    match taken {
      Some(loaded) => {
        let mut components = vec![];

        for one in loaded {
          let Ok(built) = component(one);
          components.push(built);
        }

        decl.val.ty = DirectOrIncluded::Included(Included {
          components: components.into_boxed_slice(),
        });
      }
      None => {}
    }
  }

  Ok(())
}

fn component<T: Embeddable>(loaded: LoadedInclude) -> Result<Component<T>, Never> {
  let decls = match loaded.item.val.ty {
    ItemType::Composite(_) => Box::default(),
    ItemType::Configuration(c) => T::decls_of(c),
  };

  Ok(Component {
    mm: loaded.mm,
    decls: Decls { decls },
    group: loaded.group,
  })
}
