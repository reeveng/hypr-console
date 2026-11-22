use std::io::ErrorKind;
use std::path::Path as FilePath;
use std::sync::Arc;

use console_core_never::Never;

use console_core_scalable_vector_graphics_values::{AspectRatio, Length};

use super::svgtree::{AId, SvgNode};
use super::{OptionLog, Options, converter};
use crate::{
  ClipPath, Group, Image, ImageKind, ImageRendering, Node, NonZeroRect, Path, Size, Transform,
  Tree, Visibility,
};

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The public resolver supplies encoded immutable data that can be reused in image nodes"
  )
)]
pub type ImageHrefDataResolverFn<'a> =
  Box<dyn Fn(&str, Arc<Vec<u8>>, &Options) -> Option<ImageKind> + Send + Sync + 'a>;

pub type ImageHrefStringResolverFn<'a> =
  Box<dyn Fn(&str, &Options) -> Option<ImageKind> + Send + Sync + 'a>;

pub struct ImageHrefResolver<'a> {
  pub resolve_data: ImageHrefDataResolverFn<'a>,

  pub resolve_string: ImageHrefStringResolverFn<'a>,
}

impl Default for ImageHrefResolver<'_> {
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "Constructing a resolver registers callbacks. Every embedded SVG enters the document depth limit when a callback is invoked"
    )
  )]
  fn default() -> Self {
    let Ok(resolve_data) = ImageHrefResolver::default_data_resolver();
    let Ok(resolve_string) = ImageHrefResolver::default_string_resolver();

    ImageHrefResolver {
      resolve_data,
      resolve_string,
    }
  }
}

impl ImageHrefResolver<'_> {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "The public data resolver preserves immutable image buffers for reuse by referenced image nodes"
    )
  )]
  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "The data callback enters the document guard before parsing embedded SVG data"
    )
  )]
  pub fn default_data_resolver() -> Result<ImageHrefDataResolverFn<'static>, Never> {
    Ok(Box::new(
      |mime: &str, data: Arc<Vec<u8>>, options: &Options| {
        let format = match mime {
          "image/jpg" | "image/jpeg" => Some(ImageFormat::Jpeg),
          "image/png" => Some(ImageFormat::Png),
          "image/gif" => Some(ImageFormat::Gif),
          "image/webp" => Some(ImageFormat::Webp),
          "image/svg+xml" => Some(ImageFormat::Svg),
          "text/plain" => {
            let Ok(detected) = get_image_data_format(&data);

            Some(match detected {
              Some(detected_format) => detected_format,
              None => ImageFormat::Svg,
            })
          }
          _ => None,
        };
        let source_format = format?;
        let Ok(image) = decode_image(source_format, data, options);

        image
      },
    ))
  }

  #[cfg_attr(
    dylint_lib = "explicit049_no_recursion",
    expect(
      explicit049_no_recursion,
      reason = "The registered local image callback enters the document guard when the file contains SVG"
    )
  )]
  pub fn default_string_resolver() -> Result<ImageHrefStringResolverFn<'static>, Never> {
    Ok(Box::new(|href, options| {
      let Ok(image) = read_local_image(href, options);

      image
    }))
  }
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Encoded raster bytes are immutable and can be reused by several image nodes"
  )
)]
#[must_use]
#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The file resolver uses the guarded SVG document parser for nested images"
  )
)]
fn read_local_image(href: &str, options: &Options<'_>) -> Result<Option<ImageKind>, Never> {
  let Ok(path) = options.get_abs_path(FilePath::new(href));
  let data = match std::fs::read(&path) {
    Ok(data) => data,
    Err(problem) => {
      match ::core::primitive::bool::then_some(problem.kind() == ErrorKind::NotFound, ()) {
        ::core::option::Option::Some(()) => {
          console_core_logging::warn!("'{}' is not a path to an image.", href)
        }
        ::core::option::Option::None => console_core_logging::warn!("Failed to load '{}'. {}.", href, problem),
      }

      return Ok(None);
    }
  };
  let Ok(detected) = get_image_file_format(&path, &data);
  let format = match detected {
    Some(format) => format,
    None => {
      console_core_logging::warn!("'{}' is not a PNG, JPEG, GIF, WebP or SVG(Z) image.", href);
      return Ok(None);
    }
  };
  decode_image(format, Arc::new(data), options)
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "Raster image variants retain shared immutable encoded bytes so repeated references need no byte copy"
  )
)]
#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "Nested SVG conversion enters the bounded document depth before further image resolution"
  )
)]
fn decode_image(
  format: ImageFormat,
  data: Arc<Vec<u8>>,
  options: &Options<'_>,
) -> Result<Option<ImageKind>, Never> {
  let image = match format {
    ImageFormat::Jpeg => ImageKind::JPEG(data),
    ImageFormat::Png => ImageKind::PNG(data),
    ImageFormat::Gif => ImageKind::GIF(data),
    ImageFormat::Webp => ImageKind::WEBP(data),
    ImageFormat::Svg => return load_sub_svg(&data, options),
  };

  Ok(Some(image))
}

impl std::fmt::Debug for ImageHrefResolver<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("ImageHrefResolver { .. }")
  }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ImageFormat {
  Png,
  Jpeg,
  Gif,
  Webp,
  Svg,
}

pub(crate) fn convert(
  node: SvgNode,
  state: &converter::State,
  cache: &mut converter::Cache,
  parent: &mut Group,
) -> Result<Option<()>, Never> {
  let Ok(attribute) = node.try_attribute(AId::Href);
  let href = match attribute
    .log_none(|| console_core_logging::warn!("Image lacks the 'xlink:href' attribute. Skipped."))
  {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(href_data) = get_href_data(href, state);
  let kind = match href_data {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(specified_visibility) = node.find_attribute(AId::Visibility);
  let visibility = match specified_visibility {
    Some(visibility) => visibility,
    None => Visibility::Visible,
  };
  let Ok(specified_rendering) = node.find_attribute(AId::ImageRendering);
  let rendering_mode = match specified_rendering {
    Some(mode) => mode,
    None => state.opt.image_rendering,
  };
  let id = match state.parent_markers.is_empty().then_some(()) {
    Some(()) => {
      let Ok(element_id) = node.element_id();
      element_id.to_owned()
    }
    None => String::new(),
  };

  let Ok(intrinsic_size) = kind.actual_size();
  let actual_size = match intrinsic_size {
    Some(size) => size,
    None => return Ok(None),
  };
  let Ok(rectangle) = image_rectangle(node, state, actual_size);
  let rect = match rectangle
    .log_none(|| console_core_logging::warn!("Image has an invalid size. Skipped."))
  {
    Some(rect) => rect,
    None => return Ok(None),
  };
  let Ok(specified_aspect) = node.attribute(AId::PreserveAspectRatio);
  let aspect = match specified_aspect {
    Some(aspect) => aspect,
    None => AspectRatio::default(),
  };

  convert_inner(
    ImagePlan {
      kind,
      id,
      visibility,
      rendering_mode,
      aspect,
      actual_size,
      rect,
    },
    cache,
    parent,
  )
}

fn image_rectangle(
  node: SvgNode,
  state: &converter::State,
  actual: Size,
) -> Result<Option<NonZeroRect>, Never> {
  let Ok(zero) = Length::zero();
  let Ok(x) = node.convert_user_length(AId::X, state, zero);
  let Ok(y) = node.convert_user_length(AId::Y, state, zero);
  let Ok(original_width) = actual.width();
  let Ok(original_height) = actual.height();
  let Ok(initial_width) = Length::new_number(f64::from(original_width));
  let Ok(initial_height) = Length::new_number(f64::from(original_height));
  let Ok(width) = node.convert_user_length(AId::Width, state, initial_width);
  let Ok(height) = node.convert_user_length(AId::Height, state, initial_height);
  let Ok(width_attribute) = node.attribute::<Length>(AId::Width);
  let Ok(height_attribute) = node.attribute::<Length>(AId::Height);
  let (width_2, height_2) = match (width_attribute, height_attribute) {
    (Some(_), None) => (width, original_height * (width / original_width)),
    (None, Some(_)) => (original_width * (height / original_height), height),
    (Some(_), Some(_)) | (None, None) => (width, height),
  };

  NonZeroRect::from_xywh(x, y, width_2, height_2)
}

pub(crate) struct ImagePlan {
  pub kind: ImageKind,
  pub id: String,
  pub visibility: Visibility,
  pub rendering_mode: ImageRendering,
  pub aspect: AspectRatio,
  pub actual_size: Size,
  pub rect: NonZeroRect,
}

pub(crate) fn convert_inner(
  plan: ImagePlan,
  cache: &mut converter::Cache,
  parent: &mut Group,
) -> Result<Option<()>, Never> {
  let Ok(viewbox) = aligned_view_box(plan.actual_size, plan.rect, plan.aspect);
  let view_box = match viewbox {
    Some(bounds) => bounds,
    None => return Ok(None),
  };
  let Ok(width) = view_box.width();
  let Ok(height) = view_box.height();
  let Ok(x) = view_box.x();
  let Ok(y) = view_box.y();
  let Ok(source_width) = plan.actual_size.width();
  let Ok(source_height) = plan.actual_size.height();
  let Ok(placement) =
    Transform::from_row(width / source_width, 0.0, 0.0, height / source_height, x, y);
  let Ok(abs_transform) = parent.abs_transform.pre_concat(placement);
  let Ok(mapped) = view_box.transform(parent.abs_transform);
  let abs_bounding_box = match mapped {
    Some(bounds) => bounds,
    None => return Ok(None),
  };
  let Ok(mut group) = Group::empty();
  group.id = plan.id;
  group.transform = placement;
  group.abs_transform = abs_transform;
  group.children.push(Node::Image(Box::new(Image {
    id: String::new(),
    visible: plan.visibility,
    size: plan.actual_size,
    rendering_mode: plan.rendering_mode,
    kind: plan.kind,
    abs_transform,
    abs_bounding_box,
  })));
  let Ok(_bounds) = group.calculate_bounding_boxes();

  match plan.aspect.slice.then_some(()) {
    Some(()) => {
      let Ok(clipped) = clip_sliced_image(group, plan.rect, cache, parent.abs_transform);

      match clipped {
        Some(group_2) => parent.children.push(Node::Group(Box::new(group_2))),
        None => return Ok(None),
      }
    }
    None => parent.children.push(Node::Group(Box::new(group))),
  }

  Ok(Some(()))
}

fn aligned_view_box(
  actual: Size,
  rect: NonZeroRect,
  aspect: AspectRatio,
) -> Result<Option<NonZeroRect>, Never> {
  let Ok(fitted) = fit_view_box(actual, rect, aspect);
  let fitted_2 = match fitted {
    Some(size) => size,
    None => return Ok(None),
  };
  let Ok(x) = rect.x();
  let Ok(y) = rect.y();
  let Ok(width) = rect.width();
  let Ok(height) = rect.height();
  let Ok(fitted_width) = fitted_2.width();
  let Ok(fitted_height) = fitted_2.height();
  let Ok(console_core_paths::Point { x: aligned_x, y: aligned_y }) = crate::aligned_pos(
    aspect.align,
    console_core_paths::Point { x, y },
    console_core_paths::Point { x: width - fitted_width, y: height - fitted_height },
  );

  fitted_2.to_non_zero_rect(aligned_x, aligned_y)
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The tree uses immutable geometry and clip path records that can be referenced by multiple nodes"
  )
)]
fn clip_sliced_image(
  mut image: Group,
  bounds: NonZeroRect,
  cache: &mut converter::Cache,
  abs_transform: Transform,
) -> Result<Option<Group>, Never> {
  let Ok(rect) = bounds.to_rect();
  let Ok(outline) = console_core_paths::PathBuilder::from_rect(rect);
  let Ok(geometry) = Path::new_simple(Arc::new(outline));
  let mut geometry_2 = match geometry {
    Some(path) => path,
    None => return Ok(None),
  };
  geometry_2.fill = Some(crate::Fill::default());
  let Ok(clip_id) = cache.gen_clip_path_id();
  let Ok(mut clip) = ClipPath::empty(clip_id);
  clip.root.children.push(Node::Path(Box::new(geometry_2)));

  let Ok(mut clipped) = Group::empty();
  std::mem::swap(&mut image.id, &mut clipped.id);
  clipped.abs_transform = abs_transform;
  clipped.clip_path = Some(Arc::new(clip));
  clipped.children.push(Node::Group(Box::new(image)));
  let Ok(_bounds) = clipped.calculate_bounding_boxes();

  Ok(Some(clipped))
}

#[cfg_attr(
  dylint_lib = "explicit081_no_shared_ownership",
  expect(
    explicit081_no_shared_ownership,
    reason = "The public data resolver takes immutable reference-counted image bytes"
  )
)]
pub(crate) fn get_href_data(
  href: &str,
  state: &converter::State,
) -> Result<Option<ImageKind>, Never> {
  let url = match console_core_urls::DataUrl::process(href) {
    Ok(url) => url,
    Err(_not_an_inline_image) => {
      return Ok((state.opt.image_href_resolver.resolve_string)(
        href, state.opt,
      ));
    }
  };
  let (data, _) = match url.decode_to_vec() {
    Ok(decoded) => decoded,
    Err(problem) => {
      console_core_logging::warn!("Failed to decode inline image data. {:?}.", problem);
      return Ok(None);
    }
  };
  let Ok(mime_type) = url.mime_type();
  let Ok((media_type, subtype)) = mime_type.essence();
  let mime = format!("{media_type}/{subtype}");

  Ok((state.opt.image_href_resolver.resolve_data)(
    &mime,
    Arc::new(data),
    state.opt,
  ))
}

fn get_image_file_format(
  path: &std::path::Path,
  data: &[u8],
) -> Result<Option<ImageFormat>, Never> {
  let ext = match path.extension().and_then(|e| e.to_str()) {
    Some(value) => value,
    None => return Ok(None),
  }
  .to_lowercase();

  match ::core::primitive::bool::then_some(ext == "svg" || ext == "svgz", ()) {
    ::core::option::Option::Some(()) => {
      return Ok(Some(ImageFormat::Svg));
    }
    ::core::option::Option::None => {}
  }

  let Ok(image_data_format) = get_image_data_format(data);

  Ok(image_data_format)
}

fn get_image_data_format(data: &[u8]) -> Result<Option<ImageFormat>, Never> {
  Ok(
    match match console_core_picture_sizes::format(data) {
      Ok(format) => format,
      Err(_not_a_supported_raster_format) => return Ok(None),
    } {
      console_core_picture_sizes::PictureFormat::Gif => Some(ImageFormat::Gif),
      console_core_picture_sizes::PictureFormat::Jpeg => Some(ImageFormat::Jpeg),
      console_core_picture_sizes::PictureFormat::Png => Some(ImageFormat::Png),
      console_core_picture_sizes::PictureFormat::Webp => Some(ImageFormat::Webp),
    },
  )
}

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The nested document is checked by the same-thread depth guard before further callbacks"
  )
)]
pub(crate) fn load_sub_svg(data: &[u8], opt: &Options) -> Result<Option<ImageKind>, Never> {
  Ok(match Tree::from_data_nested(data, opt) {
    Ok(tree) => Some(ImageKind::SVG(tree)),
    Err(problem) => {
      console_core_logging::warn!("Failed to load nested SVG image. {}.", problem);
      None
    }
  })
}

fn fit_view_box(
  size: Size,
  rect: NonZeroRect,
  aspect: AspectRatio,
) -> Result<Option<Size>, Never> {
  let Ok(target) = rect.size();

  match (
    aspect.align == console_core_scalable_vector_graphics_values::Align::None,
    aspect.slice,
  ) {
    (true, _) => Ok(Some(target)),
    (false, true) => size.expand_to(target),
    (false, false) => size.scale_to(target),
  }
}
