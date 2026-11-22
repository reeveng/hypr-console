use crate::cache::Cache;
use crate::color::{ColorComponents, ColorSpace, ToRgb};
use crate::context::Context;
use crate::device::Device;
use crate::interpret::path::get_paint;
use crate::interpret::state::{ActiveTransferFunction, Opacity};
use crate::{BlendMode, CacheKey, ClipPath, Image, RasterImage, StencilImage};
use crate::{FillRule, InterpreterWarning, WarningSinkFn, interpret};
use crate::{ImageData, LumaData, RgbData};
use console_core_never::Never;
use console_core_number_conversion::{nearest_f32_from_u64, toward_zero_u8};
use console_core_portable_document_functions::{Function, Interpolation, PdfSample, interpolate};
use console_core_portable_document_syntax::ContainsKeyAnswer;
use console_core_portable_document_syntax::bit_reader::BitReader;
use console_core_portable_document_syntax::content::TypedIter;
use console_core_portable_document_syntax::object::Array;
use console_core_portable_document_syntax::object::Dict;
use console_core_portable_document_syntax::object::Name;
use console_core_portable_document_syntax::object::Object;
use console_core_portable_document_syntax::object::PdfBoolean;
use console_core_portable_document_syntax::object::Stream;
use console_core_portable_document_syntax::object::dict::keys::*;
use console_core_portable_document_syntax::object::stream::{
  FilterResult, ImageColorSpace, ImageDecodeParams, ImageSamples,
};
use console_core_portable_document_syntax::page::Resources;
use console_core_size::Size;
use kurbo::{Affine, Rect, Shape};
use std::borrow::Cow;
use std::iter;
use std::ops::Deref;

pub(crate) enum XObject<'a> {
  FormXObject(FormXObject<'a>),
  ImageXObject(ImageXObject<'a>),
}

impl<'a> XObject<'a> {
  pub(crate) fn new(
    stream: &Stream<'a>,
    warning_sink: &WarningSinkFn,
    cache: &Cache,
    transfer_function: Option<ActiveTransferFunction>,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(dict) = stream.dict();

    let Ok(subtype_entry) = dict.get::<Name<'_>>(SUBTYPE);
    Ok(
      match (match subtype_entry {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      })
      .deref()
      {
        IMAGE => {
          let Ok(image_x_object) = ImageXObject::new(
            stream,
            |_| None,
            warning_sink,
            cache,
            ImageSource::Image,
            transfer_function,
          );

          Some(Self::ImageXObject(match image_x_object {
            Some(explicit_value) => explicit_value,
            None => return Ok(None),
          }))
        }
        FORM => {
          let Ok(form_x_object) = FormXObject::new(stream);

          Some(Self::FormXObject(match form_x_object {
            Some(explicit_value) => explicit_value,
            None => return Ok(None),
          }))
        }
        _ => None,
      },
    )
  }
}

#[derive(Clone, Copy)]
enum FormComposition {
  Direct,
  TransparencyGroup,
}

pub(crate) struct FormXObject<'a> {
  pub(crate) decoded: Cow<'a, [u8]>,
  pub(crate) matrix: Affine,
  pub(crate) bbox: [f32; 4],
  composition: FormComposition,
  pub(crate) dict: Dict<'a>,
  resources: Dict<'a>,
}

impl<'a> FormXObject<'a> {
  pub(crate) fn new(
    stream: &Stream<'a>,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(dict) = stream.dict();

    let decoded = match match stream.decoded() {
      Ok(parsed_value) => Some(parsed_value),
      Err(_invalid_stream_encoding) => None,
    } {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(resources_entry) = dict.get::<Dict<'_>>(RESOURCES);
    let resources = match resources_entry {
      Some(present_value) => present_value,
      None => Default::default(),
    };

    let Ok(transform_entry) = dict.get::<[f64; 6]>(MATRIX);
    let matrix = Affine::new(match transform_entry {
      ::core::option::Option::Some(option_value) => option_value,
      ::core::option::Option::None => [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    });
    let Ok(bounds_entry) = dict.get::<[f32; 4]>(BBOX);
    let bbox = match bounds_entry {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let Ok(group) = dict.get::<Dict<'_>>(GROUP);
    let composition = match group {
      Some(_) => FormComposition::TransparencyGroup,
      None => FormComposition::Direct,
    };

    Ok(Some(Self {
      decoded,
      matrix,
      composition,
      bbox,
      dict: dict.clone(),
      resources,
    }))
  }
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Every form enters the context's depth and paint budgets before interpreting an embedded stream"
  )
)]
pub(crate) fn draw_xobject<'a>(
  x_object: &XObject<'a>,
  resources: &Resources<'a>,
  context: &mut Context<'a>,
  device: &mut impl Device<'a>,
) -> Result<(), Never> {
  match x_object {
    XObject::FormXObject(form) => draw_form_xobject(resources, form, context, device),
    XObject::ImageXObject(image) => draw_image_xobject(image, context, device),
  }
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "An embedded form consumes one level of the context's fixed depth and nested paint budgets before dispatch"
  )
)]
pub(crate) fn draw_form_xobject<'a>(
  resources: &Resources<'a>,
  form: &FormXObject<'a>,
  context: &mut Context<'a>,
  device: &mut impl Device<'a>,
) -> Result<(), Never> {
  let Ok(visibility) = context.ocg_state.is_visible();

  match visibility {
    crate::Visibility::Visible => {}
    crate::Visibility::Hidden => return Ok(()),
  }

  let Ok(nesting) = context.begin_nested_interpretation();

  match nesting {
    crate::Nesting::Entered => {}
    crate::Nesting::TooDeep => return Ok(()),
  }

  let Ok(optional_content) = xobject_oc(&form.dict, context);
  let Ok(form_visibility) = context.ocg_state.is_visible();

  match form_visibility {
    crate::Visibility::Visible => {
      let Ok(()) = draw_form_contents(resources, form, context, device);
    }
    crate::Visibility::Hidden => {}
  }

  let Ok(()) = close_xobject_oc(optional_content, context);
  context.end_nested_interpretation()
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "This private renderer only runs inside draw_form_xobject after its nested stream guard"
  )
)]
fn draw_form_contents<'a>(
  resources: &Resources<'a>,
  form: &FormXObject<'a>,
  context: &mut Context<'a>,
  device: &mut impl Device<'a>,
) -> Result<(), Never> {
  let Ok(iter) = TypedIter::new(form.decoded.as_ref());
  context.path.truncate(0);
  let Ok(()) = context.save_state();
  let Ok(()) = context.pre_concat_affine(form.matrix);
  let Ok(()) = context.push_root_transform();

  match form.composition {
    FormComposition::TransparencyGroup => {
      let Ok(state) = context.get_mut();
      let graphics = &mut state.graphics_state;
      let soft_mask = graphics.soft_mask.take();
      let blend_mode = core::mem::take(&mut graphics.blend_mode);
      device.push_transparency_group(graphics.non_stroke_alpha.0, soft_mask, blend_mode);
      graphics.non_stroke_alpha = Opacity(1.0);
      graphics.stroke_alpha = Opacity(1.0);
    }
    FormComposition::Direct => {}
  }

  let Ok(state) = context.get();
  device.set_soft_mask(state.graphics_state.soft_mask.clone());
  device.set_blend_mode(state.graphics_state.blend_mode);
  let [x0, y0, x1, y1] = form.bbox.map(f64::from);
  device.push_clip_path(&ClipPath {
    path: state.ctm * Rect::new(x0, y0, x1, y1).to_path(0.1),
    fill: FillRule::NonZero,
  });

  let Ok(child_resources) = Resources::from_parent(form.resources.clone(), resources.clone());
  let Ok(()) = interpret(iter, &child_resources, context, device);
  device.pop_clip_path();

  match form.composition {
    FormComposition::TransparencyGroup => device.pop_transparency_group(),
    FormComposition::Direct => {}
  }

  let Ok(()) = context.pop_root_transform();
  context.restore_state(device)
}

pub(crate) fn draw_image_xobject<'a, 'b>(
  x_object: &ImageXObject<'b>,
  context: &mut Context<'a>,
  device: &mut impl Device<'a>,
) -> Result<(), Never> {
  let Ok(visibility) = context.ocg_state.is_visible();

  match visibility {
    crate::Visibility::Visible => {}
    crate::Visibility::Hidden => return Ok(()),
  }

  let Ok(dict) = x_object.stream.dict();
  let Ok(optional_content) = xobject_oc(dict, context);
  let Ok(image_visibility) = context.ocg_state.is_visible();

  match image_visibility {
    crate::Visibility::Visible => {
      let Ok(()) = draw_visible_image(x_object, context, device);
    }
    crate::Visibility::Hidden => {}
  }

  close_xobject_oc(optional_content, context)
}

fn draw_visible_image<'a, 'b>(
  x_object: &ImageXObject<'b>,
  context: &mut Context<'a>,
  device: &mut impl Device<'a>,
) -> Result<(), Never> {
  let width = f64::from(x_object.width);
  let height = f64::from(x_object.height);
  let Ok(()) = context.save_state();
  let Ok(()) = context.pre_concat_affine(Affine::new([
    1.0 / width,
    0.0,
    0.0,
    -1.0 / height,
    0.0,
    1.0,
  ]));
  let Ok(state) = context.get_mut();
  let transform = state.ctm;
  let graphics = &mut state.graphics_state;
  let inherited_mask = graphics.soft_mask.take();
  let blend_mode = core::mem::take(&mut graphics.blend_mode);
  let Ok(masking) = x_object.has_mask();
  let soft_mask = match masking {
    crate::Masking::Masked => None,
    crate::Masking::Unmasked => inherited_mask,
  };

  device.push_transparency_group(graphics.non_stroke_alpha.0, soft_mask, blend_mode);
  device.set_soft_mask(None);
  device.set_blend_mode(BlendMode::default());
  let image = match x_object.kind {
    ImageKind::Stencil | ImageKind::Mask => {
      let Ok(paint) = get_paint(context, crate::PaintOperation::Fill);

      Image::Stencil(StencilImage {
        paint,
        image_xobject: x_object.clone(),
      })
    }
    ImageKind::Raster => Image::Raster(RasterImage(x_object.clone())),
  };

  device.draw_image(image, transform);
  device.pop_transparency_group();
  context.restore_state(device)
}

fn close_xobject_oc(
  optional_content: crate::OptionalContent,
  context: &mut Context<'_>,
) -> Result<(), Never> {
  match optional_content {
    crate::OptionalContent::Begun => context.ocg_state.end_marked_content(),
    crate::OptionalContent::Absent => Ok(()),
  }
}

fn xobject_oc(
  dict: &Dict<'_>,
  context: &mut Context<'_>,
) -> core::result::Result<crate::OptionalContent, console_core_never::Never> {
  let Ok(content_entry) = dict.get::<Dict<'_>>(OC);
  let oc_dict = match content_entry {
    Some(oc_dict) => oc_dict,
    None => {
      return Ok(match ::core::primitive::bool::then_some(false, ()) {
        ::core::option::Option::Some(()) => crate::OptionalContent::Begun,
        ::core::option::Option::None => crate::OptionalContent::Absent,
      });
    }
  };

  let Ok(content_reference) = dict.get_ref(OC);

  match content_reference {
    Some(oc_ref) => {
      let Ok(()) = context.ocg_state.begin_ocg(&oc_dict, <::console_core_portable_document_syntax::object::ObjectIdentifier as ::core::convert::From<::console_core_portable_document_syntax::object::ObjRef>>::from(oc_ref));
    }
    None => {
      let Ok(()) = context.ocg_state.begin_ocmd(&oc_dict);
    }
  }

  Ok(match ::core::primitive::bool::then_some(true, ()) {
    ::core::option::Option::Some(()) => crate::OptionalContent::Begun,
    ::core::option::Option::None => crate::OptionalContent::Absent,
  })
}

pub(crate) enum ImageSource {
  Image,
  MaskStream,
}

#[derive(Clone, Copy)]
enum ImageKind {
  Raster,
  Mask,
  Stencil,
}

enum SamplePolarity {
  Normal,
  Inverted,
}

impl From<bool> for SamplePolarity {
  fn from(inverted: bool) -> Self {
    match inverted.then_some(()) {
      Some(()) => Self::Inverted,
      None => Self::Normal,
    }
  }
}

#[derive(Clone)]
pub(crate) struct ImageXObject<'a> {
  width: u32,
  height: u32,
  color_space: Option<ColorSpace>,
  cache: Cache,
  interpolation: crate::Interpolation,
  kind: ImageKind,
  stream: Stream<'a>,
  transfer_function: Option<ActiveTransferFunction>,
  warning_sink: WarningSinkFn,
}

impl<'a> ImageXObject<'a> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "The deferred image retains the caller's warning callback through asynchronous device decoding"
    )
  )]
  pub(crate) fn new(
    stream: &Stream<'a>,
    resolve_cs: impl FnOnce(&Name<'_>) -> Option<ColorSpace>,
    warning_sink: &WarningSinkFn,
    cache: &Cache,
    source: ImageSource,
    transfer_function: Option<ActiveTransferFunction>,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(dict) = stream.dict();

    let stencil = ({
      let Ok(explicit_value) = dict.get::<PdfBoolean>(IM);
      explicit_value
    })
    .or_else(|| {
      let Ok(explicit_value) = dict.get::<PdfBoolean>(IMAGE_MASK);
      explicit_value
    });
    let kind = match (stencil, source) {
      (Some(PdfBoolean::True), _) => ImageKind::Stencil,
      (_, ImageSource::MaskStream) => ImageKind::Mask,
      (_, ImageSource::Image) => ImageKind::Raster,
    };

    let image_cs = match kind {
      ImageKind::Stencil | ImageKind::Mask => {
        let Ok(device_gray) = ColorSpace::device_gray();

        Some(device_gray)
      }
      ImageKind::Raster => {
        let cs_obj = ({
          let Ok(explicit_value) = dict.get::<Object<'_>>(CS);
          explicit_value
        })
        .or_else(|| {
          let Ok(explicit_value) = dict.get::<Object<'_>>(COLORSPACE);
          explicit_value
        });

        cs_obj
          .clone()
          .and_then(|c| {
            let Ok(color_space) = ColorSpace::new(c, cache);

            color_space
          })
          .or_else(|| {
            cs_obj
              .and_then(|c| {
                let Ok(name) = c.into_name();

                name
              })
              .and_then(|n| resolve_cs(&n))
          })
      }
    };

    let interpolation = match ({
      let Ok(explicit_value) = dict.get::<PdfBoolean>(I);
      explicit_value
    })
    .or_else(|| {
      let Ok(explicit_value) = dict.get::<PdfBoolean>(INTERPOLATE);
      explicit_value
    }) {
      Some(PdfBoolean::True) => crate::Interpolation::Smooth,
      Some(PdfBoolean::False) | None => crate::Interpolation::Nearest,
    };

    let width = match ({
      let Ok(explicit_value) = dict.get::<u32>(W);
      explicit_value
    })
    .or_else(|| {
      let Ok(explicit_value) = dict.get::<u32>(WIDTH);
      explicit_value
    }) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };
    let height = match ({
      let Ok(explicit_value) = dict.get::<u32>(H);
      explicit_value
    })
    .or_else(|| {
      let Ok(explicit_value) = dict.get::<u32>(HEIGHT);
      explicit_value
    }) {
      Some(explicit_value) => explicit_value,
      None => return Ok(None),
    };

    match ::core::primitive::bool::then_some(width == 0 || height == 0, ()) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    Ok(Some(Self {
      width,
      cache: cache.clone(),
      height,
      color_space: image_cs,
      warning_sink: warning_sink.clone(),
      transfer_function,
      interpolation,
      stream: stream.clone(),
      kind,
    }))
  }

  pub(crate) fn decoded_mask(
    &self,
    target_dimension: Option<(u32, u32)>,
  ) -> core::result::Result<Option<DecodedMask>, console_core_never::Never> {
    match self.kind {
      ImageKind::Raster => Ok(None),
      ImageKind::Stencil | ImageKind::Mask => decode_mask(self, target_dimension),
    }
  }

  pub(crate) fn decoded_raster(
    &self,
    target_dimension: Option<(u32, u32)>,
  ) -> core::result::Result<Option<DecodedRaster>, console_core_never::Never> {
    match self.kind {
      ImageKind::Stencil | ImageKind::Mask => Ok(None),
      ImageKind::Raster => decode_raster(self, target_dimension),
    }
  }

  pub(crate) fn width(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(self.width)
  }

  pub(crate) fn height(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(self.height)
  }

  pub(crate) fn stream(&self) -> core::result::Result<&Stream<'a>, console_core_never::Never> {
    Ok(&self.stream)
  }

  fn has_mask(&self) -> core::result::Result<crate::Masking, console_core_never::Never> {
    let Ok(dict) = self.stream.dict();

    let Ok(contains_key) = dict.contains_key(SMASK_IN_DATA);

    Ok(
      match ::core::primitive::bool::then_some(
        matches!(contains_key, ContainsKeyAnswer::Yes)
          || matches!(
            {
              let Ok(explicit_value) = dict.contains_key(SMASK);
              explicit_value
            },
            ContainsKeyAnswer::Yes
          )
          || matches!(
            {
              let Ok(explicit_value) = dict.contains_key(MASK);
              explicit_value
            },
            ContainsKeyAnswer::Yes
          ),
        (),
      ) {
        ::core::option::Option::Some(()) => crate::Masking::Masked,
        ::core::option::Option::None => crate::Masking::Unmasked,
      },
    )
  }
}

pub(crate) struct DecodedMask {
  pub(crate) luma: LumaData,
}

pub(crate) struct DecodedRaster {
  pub(crate) image: ImageData,
  pub(crate) alpha: Option<LumaData>,
}

struct RasterLayout {
  dimensions: Size<u32>,
  scale_factors: (f32, f32),
  interpolation: crate::Interpolation,
}

struct DecodeContext<'a> {
  decoded: FilterResult<'a>,
  width: u32,
  height: u32,
  scale_factors: (f32, f32),
  color_space: ColorSpace,
  bits_per_component: u8,
  decode_arr: Vec<(f32, f32)>,
}

fn decode_context<'a>(
  obj: &ImageXObject<'a>,
  target_dimension: Option<(u32, u32)>,
) -> Result<Option<DecodeContext<'a>>, Never> {
  let Ok(size) = image_size_ok(obj.width, obj.height);

  match size {
    crate::ImageSize::Decodable => {}
    crate::ImageSize::TooLarge => {
      (obj.warning_sink)(InterpreterWarning::ImageDecodeFailure);
      return Ok(None);
    }
  }

  let Ok(dict) = obj.stream.dict();
  let Ok(short_bpc) = dict.get::<u8>(BPC);
  let dict_bpc = short_bpc.or_else(|| {
    let Ok(full_bpc) = dict.get::<u8>(BITS_PER_COMPONENT);
    full_bpc
  });
  let mut samples = ImageSamples::Direct;
  let num_components = match &obj.color_space {
    Some(space) => {
      let Ok(indexed) = space.is_indexed();
      samples = match indexed {
        crate::IndexedSpace::Indexed => ImageSamples::PaletteIndices,
        crate::IndexedSpace::NotIndexed => ImageSamples::Direct,
      };
      let Ok(components) = space.num_components();
      Some(components)
    }
    None => None,
  };
  let decode_params = ImageDecodeParams {
    samples,
    bpc: dict_bpc,
    num_components,
    target_dimension,
    width: obj.width,
    height: obj.height,
  };

  let decoded = match obj.stream.decoded_image(&decode_params) {
    Ok(data) => data,
    Err(_invalid_image_stream) => {
      (obj.warning_sink)(InterpreterWarning::ImageDecodeFailure);
      return Ok(None);
    }
  };
  let (width, height, scale_factors) = match decoded.image_data.as_ref() {
    Some(data) => {
      let Ok(original_width) = nearest_f32_from_u64(u64::from(obj.width));
      let Ok(original_height) = nearest_f32_from_u64(u64::from(obj.height));
      let Ok(decoded_width) = nearest_f32_from_u64(u64::from(data.width));
      let Ok(decoded_height) = nearest_f32_from_u64(u64::from(data.height));
      (
        data.width,
        data.height,
        (
          original_width / decoded_width,
          original_height / decoded_height,
        ),
      )
    }
    None => (obj.width, obj.height, (1.0, 1.0)),
  };
  let Ok(actual_size) = image_size_ok(width, height);

  match actual_size {
    crate::ImageSize::Decodable => {}
    crate::ImageSize::TooLarge => {
      (obj.warning_sink)(InterpreterWarning::ImageDecodeFailure);
      return Ok(None);
    }
  }

  let Ok(color_space) = decoded_color_space(
    obj.color_space.as_ref(),
    decoded
      .image_data
      .as_ref()
      .and_then(|data| data.color_space),
  );
  let fallback_bpc = match obj.kind {
    ImageKind::Stencil => 1,
    ImageKind::Mask | ImageKind::Raster => 8,
  };
  let bits_per_component = match decoded
    .image_data
    .as_ref()
    .map(|data| data.bits_per_component)
    .or(dict_bpc)
  {
    Some(width_2) => width_2,
    None => fallback_bpc,
  };
  let Ok(decode_arr) = image_decode_bounds(dict, &color_space, bits_per_component);

  Ok(Some(DecodeContext {
    decoded,
    width,
    height,
    scale_factors,
    color_space,
    bits_per_component,
    decode_arr,
  }))
}

fn decoded_color_space(
  stated: Option<&ColorSpace>,
  embedded: Option<ImageColorSpace>,
) -> Result<ColorSpace, Never> {
  match stated {
    Some(space) => Ok(space.clone()),
    None => match embedded {
      Some(ImageColorSpace::Rgb) => ColorSpace::device_rgb(),
      Some(ImageColorSpace::Cmyk) => ColorSpace::device_cmyk(),
      Some(ImageColorSpace::Gray | ImageColorSpace::Unknown(_)) | None => {
        ColorSpace::device_gray()
      }
    },
  }
}

fn image_decode_bounds(
  dict: &Dict<'_>,
  color_space: &ColorSpace,
  bits_per_component: u8,
) -> Result<Vec<(f32, f32)>, Never> {
  let Ok(short_decode) = dict.get::<Array<'_>>(D);
  let fields = short_decode.or_else(|| {
    let Ok(full_decode) = dict.get::<Array<'_>>(DECODE);
    full_decode
  });

  match fields {
    Some(array) => {
      let Ok(entries) = array.iter::<(f32, f32)>();
      Ok(entries.collect())
    }
    None => color_space.default_decode_arr(bits_per_component),
  }
}

fn decode_mask(
  obj: &ImageXObject<'_>,
  target_dimension: Option<(u32, u32)>,
) -> core::result::Result<Option<DecodedMask>, console_core_never::Never> {
  let Ok(context) = decode_context(obj, target_dimension);
  let ctx = match context {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };
  let mut height = ctx.height;

  let Ok(mask_bytes) = decode_mask_bytes(
    ctx.decoded.data,
    ctx.width,
    &mut height,
    &ctx.color_space,
    ctx.bits_per_component,
    &ctx.decode_arr,
    match obj.kind {
      ImageKind::Stencil => SamplePolarity::Inverted,
      ImageKind::Mask | ImageKind::Raster => SamplePolarity::Normal,
    },
  );
  let data = match mask_bytes {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  };

  Ok(Some(DecodedMask {
    luma: LumaData {
      data,
      width: ctx.width,
      height,
      interpolation: obj.interpolation,
      scale_factors: ctx.scale_factors,
    },
  }))
}

fn decode_raster(
  obj: &ImageXObject<'_>,
  target_dimension: Option<(u32, u32)>,
) -> Result<Option<DecodedRaster>, Never> {
  let Ok(context) = decode_context(obj, target_dimension);
  let mut decoded = match context {
    Some(context_2) => context_2,
    None => return Ok(None),
  };
  let mut height = decoded.height;
  let Ok(path) = raster_path(&decoded, obj.transfer_function.as_ref());
  let layout = RasterLayout {
    dimensions: Size {
      width: decoded.width,
      height,
    },
    scale_factors: decoded.scale_factors,
    interpolation: obj.interpolation,
  };
  let Ok(image_data) = match path {
    RasterPath::Bytes(polarity) => {
      decode_byte_raster(&mut decoded, layout, &mut height, polarity)
    }
    RasterPath::Floats => decode_float_raster(
      &decoded,
      layout,
      &mut height,
      obj.transfer_function.as_ref(),
    ),
  };
  let mut image = match image_data {
    Some(image) => image,
    None => return Ok(None),
  };

  let Ok(matte) = resolve_matte(obj, &decoded.color_space, target_dimension);
  let alpha =
    match matte.filter(|(alpha, _)| alpha.width == decoded.width && alpha.height == height) {
      Some((alpha, matte_rgb)) => {
        let Ok(()) = unpremultiply(&mut image, &alpha.data, &matte_rgb);
        Some(alpha)
      }
      None => {
        let Ok(alpha) = resolve_alpha(obj, &mut decoded, &image, &mut height, target_dimension);
        alpha
      }
    };

  Ok(Some(DecodedRaster { image, alpha }))
}

enum RasterPath {
  Bytes(SamplePolarity),
  Floats,
}

fn raster_path(
  context: &DecodeContext<'_>,
  transfer: Option<&ActiveTransferFunction>,
) -> Result<RasterPath, Never> {
  let Ok(direct) = context
    .color_space
    .default_decode_arr(context.bits_per_component);
  let Ok(inverted) = context
    .color_space
    .inverted_default_decode_arr(context.bits_per_component);
  let Ok(conversion) = context.color_space.sample_conversion();

  Ok(match (context.bits_per_component, conversion, transfer) {
    (8, crate::color::SampleConversion::ByteOrFloat, None) => {
      match (context.decode_arr == direct, context.decode_arr == inverted) {
        (_, true) => RasterPath::Bytes(SamplePolarity::Inverted),
        (true, false) => RasterPath::Bytes(SamplePolarity::Normal),
        (false, false) => RasterPath::Floats,
      }
    }
    _requires_float_samples => RasterPath::Floats,
  })
}

fn decode_byte_raster(
  context: &mut DecodeContext<'_>,
  mut layout: RasterLayout,
  height: &mut u32,
  polarity: SamplePolarity,
) -> Result<Option<ImageData>, Never> {
  let Ok(length) = fix_image_length(
    context.decoded.data.to_mut(),
    layout.dimensions.width,
    height,
    0,
    &context.color_space,
  );

  match length {
    Some(()) => {}
    None => return Ok(None),
  }

  match polarity {
    SamplePolarity::Inverted => {
      for sample in context.decoded.data.to_mut() {
        *sample = u8::wrapping_sub(255, *sample);
      }
    }
    SamplePolarity::Normal => {}
  }

  layout.dimensions.height = *height;
  let Ok(gray) = context.color_space.is_device_gray();
  let RasterLayout {
    dimensions: Size {
      width,
      height: rows,
    },
    interpolation,
    scale_factors,
  } = layout;
  Ok(match gray {
    crate::GraySpace::DeviceGray => Some(ImageData::Luma(LumaData {
      data: core::mem::take(&mut context.decoded.data).into_owned(),
      width,
      height: rows,
      interpolation,
      scale_factors,
    })),
    crate::GraySpace::NotDeviceGray => {
      let Ok(buffer) = rgb_buffer(layout.dimensions);
      let mut output = match buffer {
        Some(output) => output,
        None => return Ok(None),
      };
      let Ok(converted) = context
        .color_space
        .convert_u8(&context.decoded.data, &mut output);

      converted.map(|()| {
        ImageData::Rgb(RgbData {
          data: output,
          width,
          height: rows,
          interpolation,
          scale_factors,
        })
      })
    }
  })
}

fn decode_float_raster(
  context: &DecodeContext<'_>,
  mut layout: RasterLayout,
  height: &mut u32,
  transfer: Option<&ActiveTransferFunction>,
) -> Result<Option<ImageData>, Never> {
  let Ok(raw_components) = get_components(
    &context.decoded.data,
    layout.dimensions,
    &context.color_space,
    context.bits_per_component,
  );
  let components = match raw_components {
    Some(components) => components,
    None => return Ok(None),
  };
  let Ok(decoded) = apply_decode_array(
    &components,
    &context.color_space,
    context.bits_per_component,
    &context.decode_arr,
  );
  let mut samples = match decoded {
    Some(samples) => samples,
    None => return Ok(None),
  };
  let Ok(length) = fix_image_length(
    &mut samples,
    layout.dimensions.width,
    height,
    0.0,
    &context.color_space,
  );

  match length {
    Some(()) => {}
    None => return Ok(None),
  }

  layout.dimensions.height = *height;
  let Ok(rgb_image) = get_rgb_data(&samples, layout, &context.color_space);
  let mut rgb = match rgb_image {
    Some(rgb) => rgb,
    None => return Ok(None),
  };

  match transfer {
    Some(function) => {
      let Ok(()) = apply_transfer_function(&mut rgb.data, function);
    }
    None => {}
  }

  Ok(Some(ImageData::Rgb(rgb)))
}

fn apply_transfer_function(
  data: &mut [u8],
  transfer: &ActiveTransferFunction,
) -> Result<(), Never> {
  let apply = |sample, function: &Function| {
    let Ok(evaluation) = function.eval(vec![f32::from(sample) / 255.0]);

    match evaluation.and_then(|values| values.first().copied()) {
      Some(value) => {
        let Ok(rounded) = toward_zero_u8(f64::from(value * 255.0 + 0.5));
        rounded
      }
      None => sample,
    }
  };

  match transfer {
    ActiveTransferFunction::Single(function) => {
      for sample in data {
        *sample = apply(*sample, function);
      }
    }
    ActiveTransferFunction::Four([red, green, blue, _gray]) => {
      let (pixels, _partial_pixel) = data.as_chunks_mut::<3>();

      for [r, g, b] in pixels {
        *r = apply(*r, red);
        *g = apply(*g, green);
        *b = apply(*b, blue);
      }
    }
  }

  Ok(())
}

fn decode_mask_bytes(
  mut decoded_data: Cow<'_, [u8]>,
  width: u32,
  height: &mut u32,
  color_space: &ColorSpace,
  bits_per_component: u8,
  decode_arr: &[(f32, f32)],
  polarity: SamplePolarity,
) -> core::result::Result<Option<Vec<u8>>, console_core_never::Never> {
  let Ok(default_decode) = color_space.default_decode_arr(bits_per_component);
  let Ok(inverted_default) = color_space.inverted_default_decode_arr(bits_per_component);
  let fast_path = bits_per_component == 8
    && (decode_arr == default_decode.as_slice() || decode_arr == inverted_default.as_slice());

  let mut data = match ::core::primitive::bool::then_some(fast_path, ()) {
    ::core::option::Option::Some(()) => {
      let decode_polarity = SamplePolarity::from(decode_arr == inverted_default.as_slice());

      match (&polarity, decode_polarity) {
        (SamplePolarity::Inverted, SamplePolarity::Normal)
        | (SamplePolarity::Normal, SamplePolarity::Inverted) => {
          for b in decoded_data.to_mut() {
            *b = u8::wrapping_sub(255, *b);
          }
        }
        _ => {}
      }

      decoded_data.into_owned()
    }
    ::core::option::Option::None => {
      let Ok(packed_components) = get_components(
        &decoded_data,
        Size {
          width,
          height: *height,
        },
        color_space,
        bits_per_component,
      );
      let components = match packed_components {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      };

      let Ok(decoded_components) =
        apply_decode_array(&components, color_space, bits_per_component, decode_arr);
      let f32_data = match decoded_components {
        Some(explicit_value) => explicit_value,
        None => return Ok(None),
      };

      match polarity {
        SamplePolarity::Inverted => f32_data
          .iter()
          .map(|alpha| {
            let Ok(numeric_value) = console_core_number_conversion::toward_zero_u8(f64::from(
              (1.0 - *alpha) * 255.0 + 0.5,
            ));
            numeric_value
          })
          .collect(),
        SamplePolarity::Normal => f32_data
          .iter()
          .map(|alpha| {
            let Ok(numeric_value) =
              console_core_number_conversion::toward_zero_u8(f64::from(*alpha * 255.0 + 0.5));
            numeric_value
          })
          .collect(),
      }
    }
  };

  let Ok(valid_length) = fix_image_length(&mut data, width, height, 0, color_space);
  (match valid_length {
    Some(explicit_value) => explicit_value,
    None => return Ok(None),
  });

  Ok(Some(data))
}

fn resolve_alpha(
  obj: &ImageXObject<'_>,
  context: &mut DecodeContext<'_>,
  image: &ImageData,
  height: &mut u32,
  target_dimension: Option<(u32, u32)>,
) -> Result<Option<LumaData>, Never> {
  let Ok(dict) = obj.stream.dict();
  let Ok(mask_in_data) = dict.get::<u8>(SMASK_IN_DATA);

  match mask_in_data {
    Some(1) => {
      let alpha = context
        .decoded
        .image_data
        .as_mut()
        .and_then(|metadata| metadata.alpha.take());

      return match alpha {
        Some(data) => alpha_plane(data, obj, context, height),
        None => Ok(None),
      };
    }
    Some(_) | None => {}
  }

  let Ok(soft_mask) = dict.get::<Stream<'_>>(SMASK);
  let mask_stream = soft_mask.or_else(|| {
    let Ok(hard_mask) = dict.get::<Stream<'_>>(MASK);
    hard_mask
  });

  match mask_stream {
    Some(mask) => return decode_mask_stream(obj, &mask, target_dimension),
    None => {}
  }

  let Ok(mask_ranges) = dict.get::<Vec<u16>>(MASK);
  let ranges = match mask_ranges {
    Some(ranges) => ranges,
    None => return Ok(None),
  };
  let Ok(mask_components) = color_key_alpha(context, image, *height, &ranges);

  match mask_components {
    Some(data) => alpha_plane(data, obj, context, height),
    None => Ok(None),
  }
}

fn alpha_plane(
  mut data: Vec<u8>,
  obj: &ImageXObject<'_>,
  context: &DecodeContext<'_>,
  height: &mut u32,
) -> Result<Option<LumaData>, Never> {
  let Ok(gray) = ColorSpace::device_gray();
  let Ok(length) = fix_image_length(&mut data, context.width, height, 0, &gray);

  Ok(length.map(|()| LumaData {
    data,
    width: context.width,
    height: *height,
    interpolation: obj.interpolation,
    scale_factors: context.scale_factors,
  }))
}

#[cfg_attr(
  dylint_lib = "explicit056_no_hidden_panic_api",
  expect(
    explicit056_no_hidden_panic_api,
    reason = "The stride passed to the slice is an exact native conversion of a NonZeroU8 channel count"
  )
)]
fn color_key_alpha(
  context: &DecodeContext<'_>,
  image: &ImageData,
  height: u32,
  ranges: &[u16],
) -> Result<Option<Vec<u8>>, Never> {
  let source = match image {
    ImageData::Luma(data) => data.data.as_slice(),
    ImageData::Rgb(_rgb) => context.decoded.data.as_ref(),
  };
  let dimensions = Size {
    width: context.width,
    height,
  };
  let Ok(samples) = get_components(
    source,
    dimensions,
    &context.color_space,
    context.bits_per_component,
  );
  let components = match samples {
    Some(components) => components,
    None => return Ok(None),
  };
  let Ok(color_channels) = context.color_space.num_components();
  let channels = match core::num::NonZeroU8::new(color_channels) {
    Some(channels) => channels,
    None => return Ok(None),
  };
  let Ok(stride) = console_core_number_conversion::index(channels.get());
  let (bounds, _unpaired_bound) = ranges.as_chunks::<2>();
  let mut mask = Vec::new();

  for pixel in components.chunks_exact(stride) {
    let visible = pixel
      .iter()
      .zip(bounds)
      .any(|(sample, [minimum, maximum])| sample > maximum || sample < minimum);

    mask.push(match ::core::primitive::bool::then_some(visible, ()) {
      ::core::option::Option::Some(()) => 255,
      ::core::option::Option::None => 0,
    });
  }

  Ok(Some(mask))
}

fn decode_mask_stream(
  obj: &ImageXObject<'_>,
  stream: &Stream<'_>,
  target_dimension: Option<(u32, u32)>,
) -> Result<Option<LumaData>, Never> {
  let Ok(source) = ImageXObject::new(
    stream,
    |_| None,
    &obj.warning_sink,
    &obj.cache,
    ImageSource::MaskStream,
    None,
  );
  let mask = match source {
    Some(mask) => mask,
    None => return Ok(None),
  };
  let Ok(decoded) = decode_mask(&mask, target_dimension);

  Ok(decoded.map(|bitmap| bitmap.luma))
}

fn resolve_matte(
  obj: &ImageXObject<'_>,
  color_space: &ColorSpace,
  target_dimension: Option<(u32, u32)>,
) -> Result<Option<(LumaData, [u8; 3])>, Never> {
  let Ok(dict) = obj.stream.dict();
  let Ok(soft_mask) = dict.get::<Stream<'_>>(SMASK);
  let mask = match soft_mask {
    Some(mask) => mask,
    None => return Ok(None),
  };
  let Ok(mask_dict) = mask.dict();
  let Ok(matte_values) = mask_dict.get::<ColorComponents>(MATTE);
  let matte = match matte_values {
    Some(matte) => matte,
    None => return Ok(None),
  };
  let Ok(components) = color_space.num_components();
  let Ok(expected_components) = console_core_number_conversion::index(components);

  match matte.len().cmp(&expected_components) {
    core::cmp::Ordering::Equal => {}
    core::cmp::Ordering::Less | core::cmp::Ordering::Greater => return Ok(None),
  }

  let mut matte_rgb = [0_u8; 3];
  color_space.convert_f32(
    &matte,
    &mut matte_rgb,
    crate::color::InputScaling::ApplyColorSpace,
  );

  let Ok(alpha) = decode_mask_stream(obj, &mask, target_dimension);
  Ok(alpha.map(|data| (data, matte_rgb)))
}

fn unpremultiply(image: &mut ImageData, alpha: &[u8], matte_rgb: &[u8; 3]) -> Result<(), Never> {
  match image {
    ImageData::Rgb(rgb) => {
      let (pixels, _tail) = rgb.data.as_chunks_mut::<3>();

      for (pixel, coverage) in pixels.iter_mut().zip(alpha) {
        let Ok(()) = unpremultiply_pixel(pixel, *coverage, matte_rgb);
      }
    }
    ImageData::Luma(luma) => {
      let [matte, _green, _blue] = matte_rgb;

      for (sample, coverage) in luma.data.iter_mut().zip(alpha) {
        let Ok(()) = unpremultiply_pixel(
          core::slice::from_mut(sample),
          *coverage,
          core::slice::from_ref(matte),
        );
      }
    }
  }

  Ok(())
}

fn unpremultiply_pixel(samples: &mut [u8], coverage: u8, matte: &[u8]) -> Result<(), Never> {
  let alpha = match core::num::NonZeroU8::new(coverage) {
    Some(alpha) => alpha,
    None => return Ok(()),
  };
  let inv_alpha = 255.0 / f32::from(alpha.get());

  for (sample, base) in samples.iter_mut().zip(matte) {
    let unpainted = f32::from(*base);
    let Ok(adjusted) = toward_zero_u8(f64::from(
      unpainted + (f32::from(*sample) - unpainted) * inv_alpha,
    ));
    *sample = adjusted;
  }

  Ok(())
}

const MAX_IMAGE_PIXELS: u32 = 1_u32 << 28;

fn rgb_buffer(dimensions: Size<u32>) -> Result<Option<Vec<u8>>, Never> {
  let pixels = dimensions.width.saturating_mul(dimensions.height);

  match pixels {
    1..=MAX_IMAGE_PIXELS => {
      let bytes = pixels.saturating_mul(3);
      Ok(Some((0..bytes).map(|_| 0).collect()))
    }
    _outside_image_limit => Ok(None),
  }
}

fn get_rgb_data(
  decoded: &[f32],
  layout: RasterLayout,
  color_space: &ColorSpace,
) -> Result<Option<RgbData>, Never> {
  let Ok(num_components) = color_space.num_components();

  match num_components {
    0 => return Ok(None),
    1.. => {}
  }

  let Ok(buffer) = rgb_buffer(layout.dimensions);
  let mut output = match buffer {
    Some(output) => output,
    None => return Ok(None),
  };

  color_space.convert_f32(
    decoded,
    &mut output,
    crate::color::InputScaling::ApplyColorSpace,
  );

  Ok(Some(RgbData {
    data: output,
    width: layout.dimensions.width,
    height: layout.dimensions.height,
    interpolation: layout.interpolation,
    scale_factors: layout.scale_factors,
  }))
}

impl CacheKey for ImageXObject<'_> {
  fn cache_key(&self) -> u128 {
    self.stream.cache_key()
  }
}

#[must_use]
fn fix_image_length<T: Copy>(
  data: &mut Vec<T>,
  width: u32,
  height: &mut u32,
  filler: T,
  color_space: &ColorSpace,
) -> Result<Option<()>, Never> {
  let Ok(components) = color_space.num_components();
  let row =
    match core::num::NonZeroU64::new(u64::from(width).saturating_mul(u64::from(components))) {
      Some(row) => row,
      None => return Ok(None),
    };
  let expected = row.get().saturating_mul(u64::from(*height));
  let Ok(sample_count) = console_core_number_conversion::fixed_index(data.len());

  match sample_count.cmp(&expected) {
    core::cmp::Ordering::Equal | core::cmp::Ordering::Greater => {
      let Ok(native_length) = console_core_number_conversion::index(expected);
      data.truncate(native_length);
    }
    core::cmp::Ordering::Less => {
      *height = match u32::try_from(sample_count.div_ceil(row.get())) {
        Ok(complete_rows) => complete_rows,
        Err(_too_many_rows) => return Ok(None),
      };
      let partial_row = sample_count.wrapping_rem(row.get());

      match partial_row {
        0 => {}
        _ => {
          let fill_count = row.get().saturating_sub(partial_row);
          let ::core::result::Result::Ok(native_length) =
            ::console_core_number_conversion::fixed_index(
              match console_core_number_conversion::checked_index(fill_count) {
                Ok(length) => length,
                Err(_row_exceeds_address_space) => return Ok(None),
              },
            );
          data.extend(iter::repeat_n(
            filler,
            match ::console_core_number_conversion::index(native_length) {
              ::core::result::Result::Ok(native_position) => native_position,
            },
          ));
        }
      }
    }
  }

  Ok(match *height {
    0 => None,
    1.. => Some(()),
  })
}

fn get_components(
  data: &[u8],
  dimensions: Size<u32>,
  color_space: &ColorSpace,
  bits_per_component: u8,
) -> Result<Option<Vec<u16>>, Never> {
  let samples = match bits_per_component {
    1..=7 | 9..=15 => {
      let Ok(components) = color_space.num_components();
      let Ok(samples) = read_packed_samples(
        data,
        dimensions,
        PackedSamples {
          channel_count: components,
          bit_depth: bits_per_component,
        },
      );
      samples
    }
    8 => data.iter().map(|value| u16::from(*value)).collect(),
    16 => {
      let (word_bytes, _partial_word) = data.as_chunks::<2>();
      word_bytes.iter().copied().map(u16::from_be_bytes).collect()
    }
    _ => {
      warn!("unsupported bits per component: {bits_per_component}");
      return Ok(None);
    }
  };

  Ok(Some(samples))
}

struct PackedSamples {
  channel_count: u8,
  bit_depth: u8,
}

fn read_packed_samples(
  data: &[u8],
  dimensions: Size<u32>,
  encoding: PackedSamples,
) -> Result<Vec<u16>, Never> {
  let mut samples = Vec::new();
  let Ok(mut reader) = BitReader::new(data);

  for _row in 0..dimensions.height {
    for _column in 0..dimensions.width {
      for _channel in 0..encoding.channel_count {
        let Ok(read) = reader.read(encoding.bit_depth);
        let sample = match read {
          Some(sample) => sample,
          None => 0,
        };
        let Ok(sample_u16) = console_core_number_conversion::wrapped::<_, u16>(sample);
        samples.push(sample_u16);
      }
    }

    let Ok(()) = reader.align();
  }

  Ok(samples)
}

#[cfg_attr(
  dylint_lib = "explicit056_no_hidden_panic_api",
  expect(
    explicit056_no_hidden_panic_api,
    reason = "The stride passed to the slice is an exact native conversion of a NonZeroU8 channel count"
  )
)]
fn apply_decode_array(
  components: &[u16],
  color_space: &ColorSpace,
  bits_per_component: u8,
  decode: &[(f32, f32)],
) -> Result<Option<Vec<f32>>, Never> {
  let domain = [0.0, 2.0_f32.powi(i32::from(bits_per_component)) - 1.0];
  let Ok(count) = color_space.num_components();
  let channels = match core::num::NonZeroU8::new(count) {
    Some(channels) => channels,
    None => return Ok(None),
  };
  let Ok(stride) = console_core_number_conversion::index(channels.get());
  let mut samples = Vec::new();

  for pixel in components.chunks(stride) {
    for (component, (minimum, maximum)) in pixel.iter().zip(decode) {
      let Ok(decoded) = interpolate(
        PdfSample(f32::from(*component)),
        Interpolation {
          domain,
          range: [*minimum, *maximum],
        },
      );
      samples.push(decoded);
    }
  }

  Ok(Some(samples))
}

#[cfg_attr(
  dylint_lib = "explicit024_no_swappable_params",
  expect(
    explicit024_no_swappable_params,
    reason = "The public bounds check keeps the width and height order used by image-producing callers"
  )
)]
pub fn image_size_ok(
  width: u32,
  height: u32,
) -> core::result::Result<crate::ImageSize, console_core_never::Never> {
  Ok(
    match ::core::primitive::bool::then_some(
      width > 0
        && height > 0
        && u64::wrapping_mul(u64::from(width), u64::from(height)) <= u64::wrapping_shl(1, 28_u32),
      (),
    ) {
      ::core::option::Option::Some(()) => crate::ImageSize::Decodable,
      ::core::option::Option::None => crate::ImageSize::TooLarge,
    },
  )
}
