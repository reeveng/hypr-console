use crate::{Expression, Value};

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one table keeps property variants, kinds and spellings aligned with their expression construction and lookup"
  )
)]
macro_rules! define_property {
    (
        $(
            $(#[$attr:meta])*
            $variant:ident($value_ty:ident, $name:expr),
        )+
    ) => {
        #[derive(Clone, Debug, PartialEq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub enum Property {
            $(
                $(#[$attr])*
                $variant(Expression),
            )+
            Dynamic(String, Expression),
        }

        impl Property {
            pub fn kind(&self) -> Result<PropertyKind, Never> {
                Ok(match self {
                    $(
                        Property::$variant(_) => PropertyKind::$variant,
                    )+
                    Property::Dynamic(s, _) => PropertyKind::Dynamic(s.clone()),
                })
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub enum PropertyKind {
            $(
                $(#[$attr])*
                $variant,
            )+
            Dynamic(String),
        }

        parse_enum! {
            PropertyKind,
            $(
                ($variant, $name),
            )+
            |s| Ok(PropertyKind::Dynamic(s.into())),
        }

        impl PropertyKind {
            pub fn make_property(self, expr: Expression) -> Result<Property, Never> {
                Ok(match self {
                    $(
                        PropertyKind::$variant => Property::$variant(expr),
                    )+
                    PropertyKind::Dynamic(name) => Property::Dynamic(name.clone(), expr),
                })
            }
        }
    };
}

#[cfg_attr(
  all(feature = "serde", dylint_lib = "explicit017_question_mark_alone"),
  expect(
    explicit017_question_mark_alone,
    reason = "serde's derive reads a two-field variant with a `?` inside an expression, and because `Dynamic` is spelled inside `define_property!` the span it gives that `?` is this file's rather than serde's; the module holds nothing but the macro's output, which is the narrowest place a derived impl's lint level can be said"
  )
)]
mod spelled {
  use crate::Expression;
  use console_core_never::Never;

  define_property! {

      Family(String, "family"),

      FamilyLang(String, "familylang"),

      Style(String, "style"),

      StyleLang(String, "stylelang"),

      FullName(String, "fullname"),

      FullNameLang(String, "fullnamelang"),


      Slant(Int, "slant"),

      Weight(Int, "weight"),

      Size(Double, "size"),

      Width(Int, "width"),

      Aspect(Double, "aspect"),

      PixelSize(Double, "pixelsize"),

      Spacing(Int, "spacing"),

      Foundry(String, "foundry"),

      Antialias(Bool, "antialias"),

      Hinting(Bool, "hinting"),

      HintStyle(Int, "hintstyle"),

      VerticalLayout(Bool, "verticallayout"),

      AutoHint(Bool, "autohint"),

      GlobalAdvance(Bool, "globaladvance"),


      File(String, "file"),

      Index(Int, "index"),




      Rasterizer(String, "rasterizer"),

      Outline(Bool, "outline"),

      Scalable(Bool, "scalable"),

      Color(Bool, "color"),

      Scale(Double, "scale"),

      Dpi(Double, "dpi"),

      Rgba(Int, "rgba"),

      Lcdfilter(Int, "lcdfilter"),

      Minspace(Bool, "minspace"),

      Charset(CharSet, "charset"),

      Lang(String, "lang"),

      Fontversion(Int, "fontversion"),

      Capability(String, "capability"),

      Fontformat(String, "fontformat"),

      Embolden(Bool, "embolden"),

      Embeddedbitmap(Bool, "embeddedbitmap"),

      Decorative(Bool, "decorative"),

      Fontfeatures(String, "fontfeatures"),

      Namelang(String, "namelang"),

      Prgname(String, "prgname"),

      Postscriptname(String, "postscriptname"),

      Fonthashint(Bool, "fonthashint"),

      Order(Int, "order"),



      Matrix(Matrix, "matrix"),
      PixelSizeFixupFactor(Double, "pixelsizefixupfactor"),
      ScalingNotNeeded(Bool, "scalingnotneeded"),
  }
}

pub use spelled::{Property, PropertyKind};

impl Default for Property {
  fn default() -> Self {
    Property::Family(Expression::Simple(Value::String(String::default())))
  }
}

impl Default for PropertyKind {
  fn default() -> Self {
    let Ok(kind) = Property::default().kind();

    kind
  }
}
