use crate::color::{Color, ColorComponents, ColorSpace};
use crate::context::{Context, InterpreterCache};
use crate::device::Device;
use crate::interpret::state::Opacity;
use crate::interpret::state::State;
use crate::util::hash128;
use crate::x_object::{FormXObject, draw_form_xobject};
use crate::{CacheKey, InterpreterSettings};
use console_core_portable_document_functions::Function;
use console_core_portable_document_syntax::object::Name;
use console_core_portable_document_syntax::object::ObjectIdentifier;
use console_core_portable_document_syntax::object::Stream;
use console_core_portable_document_syntax::object::dict::keys::*;
use console_core_portable_document_syntax::object::{Dict, Object};
use console_core_portable_document_syntax::page::Resources;
use console_core_portable_document_syntax::xref::XRef;
use kurbo::Affine;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskType {
  Luminosity,

  Alpha,
}

pub struct TransferFunction(Function);

impl TransferFunction {
  #[inline]
  #[cfg_attr(
    dylint_lib = "explicit067_no_unit_less_quantity",
    expect(
      explicit067_no_unit_less_quantity,
      reason = "The public mask transfer function accepts a normalized alpha or luminosity sample"
    )
  )]
  pub fn apply(&self, val: f32) -> core::result::Result<f32, console_core_never::Never> {
    let Ok(eval) = self.0.eval(vec![val]);

    Ok(
      match eval.and_then(|v| v.first().copied()) {
        ::core::option::Option::Some(present) => present,
        ::core::option::Option::None => 0.0,
      }
      .clamp(0.0, 1.0),
    )
  }
}

struct Repr<'a> {
  obj_id: ObjectIdentifier,
  group: FormXObject<'a>,
  mask_type: MaskType,
  parent_resources: Resources<'a>,
  root_transform: Affine,
  bbox: kurbo::Rect,
  interpreter_cache: InterpreterCache<'a>,
  transfer_function: Option<TransferFunction>,
  settings: InterpreterSettings,
  background: Color,
  xref: &'a XRef,
  nesting_depth: u32,
}

impl Hash for Repr<'_> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.obj_id.hash(state);
    self.root_transform.cache_key().hash(state);
  }
}

#[derive(Clone, Hash)]
#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Saved graphics states and deferred device groups retain a single parsed mask and its resources"
  )
)]
pub struct SoftMask<'a>(Rc<Repr<'a>>);

impl Debug for SoftMask<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "SoftMask({:?})", self.0.obj_id)
  }
}

impl PartialEq for SoftMask<'_> {
  fn eq(&self, other: &Self) -> bool {
    self.0.obj_id == other.0.obj_id
  }
}

impl Eq for SoftMask<'_> {}

impl CacheKey for SoftMask<'_> {
  fn cache_key(&self) -> u128 {
    {
      let Ok(explicit_value) = hash128(self);
      explicit_value
    }
  }
}

impl<'a> SoftMask<'a> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Cloned graphics states and deferred groups share the parsed immutable form for the mask"
    )
  )]
  pub(crate) fn new(
    dict: &Dict<'a>,
    context: &Context<'a>,
    parent_resources: Resources<'a>,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(group_reference) = dict.get_ref(G);
    let obj_id = <::console_core_portable_document_syntax::object::ObjectIdentifier as ::core::convert::From<::console_core_portable_document_syntax::object::ObjRef>>::from(match group_reference  { Some(explicit_value) => explicit_value, None => return Ok(None) } );
    let Ok(group_entry) = dict.get::<Stream<'_>>(G);
    let group_stream = match group_entry {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(form_x_object) = FormXObject::new(&group_stream);
    let group = match form_x_object {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    let Ok(group_dict) = group.dict.get::<Dict<'_>>(GROUP);
    let cs = match group_dict
      .and_then(|g| {
        let Ok(explicit_value) = g.get::<Object<'_>>(CS);
        explicit_value
      })
      .and_then(|o| {
        let Ok(color_space) = ColorSpace::new(o, &context.interpreter_cache.object_cache);

        color_space
      }) {
      ::core::option::Option::Some(option_value) => option_value,
      ::core::option::Option::None => {
        let Ok(device_rgb) = ColorSpace::device_rgb();

        device_rgb
      }
    };
    let transfer_function = ({
      let Ok(explicit_value) = dict.get::<Object<'_>>(TR);
      explicit_value
    })
    .and_then(|o| {
      let Ok(function) = Function::new(&o);

      function
    })
    .map(TransferFunction);
    let Ok(subtype_entry) = dict.get::<Name<'_>>(S);
    let (mask_type, background) = match (match subtype_entry {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    })
    .deref()
    {
      LUMINOSITY => {
        let color = {
          let option_before_default = ({
            let Ok(explicit_value) = dict.get::<ColorComponents>(BC);
            explicit_value
          })
          .map(|c| {
            let Ok(explicit_value) = Color::new(cs, c, Opacity(1.0));
            explicit_value
          });
          let chosen_default = {
            let Ok(explicit_value) = Color::new(
              {
                let Ok(explicit_value) = ColorSpace::device_gray();
                explicit_value
              },
              vec![0.0],
              Opacity(1.0),
            );
            explicit_value
          };

          match option_before_default {
            Some(present_value) => present_value,
            None => chosen_default,
          }
        };

        (MaskType::Luminosity, color)
      }
      ALPHA => {
        let Ok(device_gray) = ColorSpace::device_gray();

        (
          MaskType::Alpha,
          ({
            let Ok(explicit_value) = Color::new(device_gray, vec![0.0], Opacity(1.0));
            explicit_value
          }),
        )
      }
      _ => return Ok(None),
    };
    let Ok(context_nesting_depth) = context.nesting_depth();
    let nesting_depth = u32::wrapping_add(context_nesting_depth, 1);

    Ok(Some(Self(Rc::new(Repr {
      obj_id,
      group,
      mask_type,
      root_transform: ({
        let Ok(explicit_value) = context.get();
        explicit_value
      })
      .ctm,
      transfer_function,
      bbox: ({
        let Ok(explicit_value) = context.bbox();
        explicit_value
      }),
      interpreter_cache: context.interpreter_cache.clone(),
      settings: context.settings.clone(),
      xref: context.xref,
      background,
      parent_resources,
      nesting_depth,
    }))))
  }

  pub fn interpret(
    &self,
    device: &mut impl Device<'a>,
  ) -> core::result::Result<(), console_core_never::Never> {
    {
      let Ok(state) = State::new(self.0.root_transform);
      let Ok(mut ctx) = Context::new_with(
        self.0.root_transform,
        self.0.bbox,
        &self.0.interpreter_cache,
        self.0.xref,
        self.0.settings.clone(),
        state,
        self.0.nesting_depth,
      );
      let Ok(()) = draw_form_xobject(&self.0.parent_resources, &self.0.group, &mut ctx, device);
    };

    Ok(())
  }

  pub fn id(&self) -> core::result::Result<ObjectIdentifier, console_core_never::Never> {
    Ok(self.0.obj_id)
  }

  pub fn mask_type(&self) -> core::result::Result<MaskType, console_core_never::Never> {
    Ok(self.0.mask_type)
  }

  pub fn background_color(&self) -> core::result::Result<Color, console_core_never::Never> {
    Ok(self.0.background.clone())
  }

  pub fn transfer_function(
    &self,
  ) -> core::result::Result<Option<&TransferFunction>, console_core_never::Never> {
    Ok(self.0.transfer_function.as_ref())
  }
}
