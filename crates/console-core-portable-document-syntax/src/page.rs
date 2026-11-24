//! Reading the pages of a PDF document.

use crate::content::{TypedIter, UntypedIter};
use crate::object::Array;
use crate::object::Dict;
use crate::object::Name;
use crate::object::ObjectIdentifier;
use crate::object::Rectangle;
use crate::object::Stream;
use crate::object::dict::keys::*;
use crate::object::{Object, ObjectLike};
use crate::reader::ReaderContext;
use crate::sync::OnceLock;
use crate::transform::Transform;
use crate::util::FloatExt;
use crate::xref::XRef;
use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::vec;
use alloc::vec::Vec;
use core::ops::Deref;

#[derive(Debug, Clone)]
struct PagesContext {
  media_box: Option<Rectangle>,
  crop_box: Option<Rectangle>,
  rotate: Option<i32>,
}

impl PagesContext {
  fn new() -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      media_box: None,
      crop_box: None,
      rotate: None,
    })
  }
}

#[derive(Clone, Copy)]
enum PageReading {
  Tree,
  Recovery,
}

pub struct Pages<'a> {
  pages: Vec<Page<'a>>,
  xref: &'a XRef,
}

impl<'a> Pages<'a> {
  pub(crate) fn new(
    pages_dict: &Dict<'a>,
    ctx: &ReaderContext<'a>,
    xref: &'a XRef,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let mut pages = vec![];
    let mut visited = BTreeSet::new();
    let Ok(page_context) = PagesContext::new();
    let Ok(empty) = Dict::empty();
    let Ok(resources) = Resources::new(empty, None, ctx);
    let Ok(resolved) = resolve_pages(
      pages_dict,
      &mut pages,
      page_context,
      resources,
      &mut visited,
      0,
    );

    Ok(resolved.map(|()| Self { pages, xref }))
  }

  pub(crate) fn new_brute_force(
    ctx: &ReaderContext<'a>,
    xref: &'a XRef,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let mut pages = vec![];
    let Ok(objects) = xref.objects();

    for object in objects {
      let Ok(dictionary) = object.into_dict();
      let dict = match dictionary {
        Some(dict) => dict,
        None => continue,
      };
      let Ok(page_context) = PagesContext::new();
      let Ok(empty) = Dict::empty();
      let Ok(resources) = Resources::new(empty, None, ctx);
      let Ok(found) = Page::new(&dict, &page_context, resources, PageReading::Recovery);

      match found {
        Some(page) => pages.push(page),
        None => {}
      }
    }

    Ok(
      match ::core::primitive::bool::then_some(pages.is_empty(), ()) {
        ::core::option::Option::Some(()) => None,
        ::core::option::Option::None => Some(Self { pages, xref }),
      },
    )
  }

  pub fn xref(&self) -> core::result::Result<&'a XRef, console_core_never::Never> {
    Ok(self.xref)
  }
}

impl<'a> Deref for Pages<'a> {
  type Target = [Page<'a>];

  fn deref(&self) -> &Self::Target {
    &self.pages
  }
}

const MAX_PAGE_TREE_DEPTH: u16 = 256;

#[cfg_attr(
  dylint_lib = "explicit049_no_recursion",
  expect(
    explicit049_no_recursion,
    reason = "The page walker has a fixed 256-level limit and records each indirect page tree node"
  )
)]
fn resolve_pages<'a>(
  pages_dict: &Dict<'a>,
  entries: &mut Vec<Page<'a>>,
  mut ctx: PagesContext,
  resources: Resources<'a>,
  visited: &mut BTreeSet<ObjectIdentifier>,
  depth: u16,
) -> core::result::Result<Option<()>, console_core_never::Never> {
  match ::core::primitive::bool::then_some(depth > MAX_PAGE_TREE_DEPTH, ()) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let Ok(id) = pages_dict.obj_id();

  match id {
    Some(object_id) => match visited.insert(object_id).then_some(()) {
      Some(()) => {}
      None => return Ok(None),
    },
    None => {}
  }

  let Ok(explicit_value) = pages_dict.get::<Rectangle>(MEDIA_BOX);

  match explicit_value {
    Some(media_box) => {
      ctx.media_box = Some(media_box);
    }
    None => {}
  }

  let Ok(explicit_value_2) = pages_dict.get::<Rectangle>(CROP_BOX);

  match explicit_value_2 {
    Some(crop_box) => {
      ctx.crop_box = Some(crop_box);
    }
    None => {}
  }

  let Ok(explicit_value_3) = pages_dict.get::<i32>(ROTATE);

  match explicit_value_3 {
    Some(rotate) => {
      ctx.rotate = Some(rotate);
    }
    None => {}
  }

  let Ok(found_resources) = pages_dict.get::<Dict<'_>>(RESOURCES);
  let local_resources = match found_resources {
    Some(dictionary) => dictionary,
    None => Dict::default(),
  };
  let Ok(resources_2) = Resources::from_parent(local_resources, resources);

  let Ok(explicit_value_6) = pages_dict.get::<Array<'a>>(KIDS);
  let kids = match explicit_value_6 {
    Some(value) => value,
    None => return Ok(None),
  };

  for dict in {
    let Ok(explicit_value_7) = kids.iter::<Dict<'_>>();
    explicit_value_7
  } {
    match ({
      let Ok(explicit_value_8) = dict.get::<Name<'_>>(TYPE);
      explicit_value_8
    })
    .as_deref()
    {
      Some(PAGES) => {
        ({
          let Ok(explicit_value_9) = resolve_pages(
            &dict,
            entries,
            ctx.clone(),
            resources_2.clone(),
            visited,
            depth.saturating_add(1),
          );
          explicit_value_9
        });
      }

      ::core::option::Option::None | ::core::option::Option::Some(_) => {
        let Ok(explicit_value_10) =
          Page::new(&dict, &ctx, resources_2.clone(), PageReading::Tree);

        match explicit_value_10 {
          Some(page) => {
            entries.push(page);
          }
          None => {}
        }
      }
    }
  }

  Ok(Some(()))
}

#[derive(Debug, Copy, Clone)]
pub enum Rotation {
  None,

  Horizontal,

  Flipped,

  FlippedHorizontal,
}

#[doc = "The origin and vertical axis of the output page coordinates."]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageCoordinates {
  #[doc = "The top-left corner is the origin and y increases downward."]
  TopLeft,
  #[doc = "The bottom-left corner is the origin and y increases upward."]
  BottomLeft,
}

pub struct Page<'a> {
  inner: Dict<'a>,
  media_box: Rectangle,
  crop_box: Rectangle,
  rotation: Rotation,
  page_streams: OnceLock<Option<Vec<u8>>>,
  resources: Resources<'a>,
  ctx: ReaderContext<'a>,
}

impl<'a> Page<'a> {
  fn new(
    dict: &Dict<'a>,
    page_context: &PagesContext,
    parent_resources: Resources<'a>,
    reading: PageReading,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(contents) = dict.contains_key(CONTENTS);

    match (reading, contents) {
      (PageReading::Recovery, crate::ContainsKeyAnswer::No) => return Ok(None),
      (PageReading::Recovery, crate::ContainsKeyAnswer::Yes) | (PageReading::Tree, _) => {}
    }

    let Ok(media) = dict.get::<Rectangle>(MEDIA_BOX);
    let media_box = match media.or(page_context.media_box) {
      Some(rectangle) => rectangle,
      None => A4,
    };
    let Ok(crop) = dict.get::<Rectangle>(CROP_BOX);
    let crop_box = match crop.or(page_context.crop_box) {
      Some(rectangle) => rectangle,
      None => media_box,
    };
    let Ok(rotate) = dict.get::<i32>(ROTATE);
    let degrees = match rotate.or(page_context.rotate) {
      Some(degrees) => degrees,
      None => 0,
    };
    let rotation = match degrees.rem_euclid(360) {
      0 => Rotation::None,
      90 => Rotation::Horizontal,
      180 => Rotation::Flipped,
      270 => Rotation::FlippedHorizontal,
      _other_angle => Rotation::None,
    };
    let ctx = parent_resources.ctx.clone();
    let Ok(dictionary) = resource_dictionary(dict, RESOURCES);
    let Ok(resources) = Resources::from_parent(dictionary, parent_resources);

    Ok(Some(Self {
      inner: dict.clone(),
      media_box,
      crop_box,
      rotation,
      page_streams: OnceLock::new(),
      resources,
      ctx,
    }))
  }

  fn operations_impl(
    &self,
  ) -> core::result::Result<Option<UntypedIter<'_>>, console_core_never::Never> {
    let Ok(explicit_value) = self.page_stream();
    let stream = match explicit_value {
      Some(value) => value,
      None => return Ok(None),
    };
    let iter = {
      let Ok(explicit_value_2) = UntypedIter::new(stream);
      explicit_value_2
    };

    Ok(Some(iter))
  }

  pub fn page_stream(&self) -> core::result::Result<Option<&[u8]>, console_core_never::Never> {
    let convert_single = |s: Stream<'_>| match s.decoded() {
      Ok(data) => Some(data.into_owned()),
      Err(error) => {
        warn!("couldn't decode PDF page content {:?}", error);
        None
      }
    };

    Ok(
      self
        .page_streams
        .get_or_init(|| {
          let Ok(explicit_value) = self.inner.get::<Stream<'_>>(CONTENTS);

          match explicit_value {
            Some(stream) => convert_single(stream),
            None => match match self.inner.get::<Array<'_>>(CONTENTS) {
              Ok(explicit_value_2) => explicit_value_2,
            } {
              Some(array) => {
                let streams = ({
                  let Ok(explicit_value_3) = array.iter::<Stream<'_>>();
                  explicit_value_3
                })
                .flat_map(convert_single);

                let mut collected = vec![];

                for stream in streams {
                  collected.extend(stream);

                  collected.push(b' ');
                }

                Some(collected)
              }
              None => {
                warn!("contents entry of page was neither stream nor array of streams");

                None
              }
            },
          }
        })
        .as_ref()
        .map(|d| d.as_slice()),
    )
  }

  pub fn resources(&self) -> core::result::Result<&Resources<'a>, console_core_never::Never> {
    Ok(&self.resources)
  }

  pub fn media_box(&self) -> core::result::Result<Rectangle, console_core_never::Never> {
    Ok(self.media_box)
  }

  pub fn rotation(&self) -> core::result::Result<Rotation, console_core_never::Never> {
    Ok(self.rotation)
  }

  pub fn crop_box(&self) -> core::result::Result<Rectangle, console_core_never::Never> {
    Ok(self.crop_box)
  }

  pub fn intersected_crop_box(
    &self,
  ) -> core::result::Result<Rectangle, console_core_never::Never> {
    Ok({
      let Ok(explicit_value) = ({
        let Ok(explicit_value) = self.crop_box();
        explicit_value
      })
      .intersect({
        let Ok(explicit_value) = self.media_box();
        explicit_value
      });
      explicit_value
    })
  }

  pub fn base_dimensions(&self) -> core::result::Result<(f32, f32), console_core_never::Never> {
    let crop_box = {
      let Ok(explicit_value) = self.intersected_crop_box();
      explicit_value
    };

    Ok(
      match ::core::primitive::bool::then_some(
        matches!(
          {
            let Ok(explicit_value) =
              match ::console_core_number_conversion::nearest_f32_from_f64({
                let Ok(explicit_value) = crop_box.width();
                explicit_value
              }) {
                ::core::result::Result::Ok(_converted_number) => _converted_number,
              }
              .is_nearly_zero();
            explicit_value
          },
          crate::IsNearlyZeroAnswer::Yes
        ) || matches!(
          {
            let Ok(explicit_value) =
              match ::console_core_number_conversion::nearest_f32_from_f64({
                let Ok(explicit_value) = crop_box.height();
                explicit_value
              }) {
                ::core::result::Result::Ok(_converted_number) => _converted_number,
              }
              .is_nearly_zero();
            explicit_value
          },
          crate::IsNearlyZeroAnswer::Yes
        ),
        (),
      ) {
        ::core::option::Option::Some(()) => (
          match ::console_core_number_conversion::nearest_f32_from_f64({
            let Ok(explicit_value) = A4.width();
            explicit_value
          }) {
            ::core::result::Result::Ok(_converted_number) => _converted_number,
          },
          match ::console_core_number_conversion::nearest_f32_from_f64({
            let Ok(explicit_value) = A4.height();
            explicit_value
          }) {
            ::core::result::Result::Ok(_converted_number) => _converted_number,
          },
        ),
        ::core::option::Option::None => (
          match ::console_core_number_conversion::nearest_f32_from_f64(
            ({
              let Ok(explicit_value) = crop_box.width();
              explicit_value
            })
            .max(1.0),
          ) {
            ::core::result::Result::Ok(_converted_number) => _converted_number,
          },
          match ::console_core_number_conversion::nearest_f32_from_f64(
            ({
              let Ok(explicit_value) = crop_box.height();
              explicit_value
            })
            .max(1.0),
          ) {
            ::core::result::Result::Ok(_converted_number) => _converted_number,
          },
        ),
      },
    )
  }

  pub fn render_dimensions(&self) -> core::result::Result<(f32, f32), console_core_never::Never> {
    let (mut base_width, mut base_height) = {
      let Ok(explicit_value) = self.base_dimensions();
      explicit_value
    };

    let Ok(rotation) = self.rotation();

    match rotation {
      Rotation::Horizontal | Rotation::FlippedHorizontal => {
        core::mem::swap(&mut base_width, &mut base_height);
      }
      Rotation::None | Rotation::Flipped => {}
    }

    Ok((base_width, base_height))
  }

  pub fn operations(&self) -> core::result::Result<UntypedIter<'_>, console_core_never::Never> {
    let Ok(parsed) = self.operations_impl();

    match parsed {
      Some(operations) => Ok(operations),
      None => UntypedIter::empty(),
    }
  }

  pub fn raw(&self) -> core::result::Result<&Dict<'a>, console_core_never::Never> {
    Ok(&self.inner)
  }

  pub fn xref(&self) -> core::result::Result<&'a XRef, console_core_never::Never> {
    Ok({
      let Ok(explicit_value) = self.ctx.xref();
      explicit_value
    })
  }

  pub fn typed_operations(
    &self,
  ) -> core::result::Result<TypedIter<'_>, console_core_never::Never> {
    Ok({
      let Ok(explicit_value) = TypedIter::from_untyped({
        let Ok(explicit_value) = self.operations();
        explicit_value
      });
      explicit_value
    })
  }

  #[doc = "Map PDF user coordinates into the cropped and rotated output page."]
  pub fn initial_transform(
    &self,
    coordinates: PageCoordinates,
  ) -> core::result::Result<Transform, console_core_never::Never> {
    let crop_box = {
      let Ok(explicit_value) = self.intersected_crop_box();
      explicit_value
    };
    let (_, base_height) = {
      let Ok(explicit_value) = self.base_dimensions();
      explicit_value
    };
    let (width, height) = {
      let Ok(explicit_value) = self.render_dimensions();
      explicit_value
    };

    let horizontal_t = Transform::ROTATE_CW_90
      * ({
        let Ok(explicit_value) = Transform::translate((0.0, f64::from(-width)));
        explicit_value
      });
    let flipped_horizontal_t = ({
      let Ok(explicit_value) = Transform::translate((0.0, f64::from(height)));
      explicit_value
    }) * Transform::ROTATE_CCW_90;

    let Ok(rotation) = self.rotation();
    let rotation_transform = match (rotation, coordinates) {
      (Rotation::None, PageCoordinates::TopLeft | PageCoordinates::BottomLeft) => {
        Transform::IDENTITY
      }
      (Rotation::Horizontal, PageCoordinates::TopLeft)
      | (Rotation::FlippedHorizontal, PageCoordinates::BottomLeft) => horizontal_t,
      (Rotation::Horizontal, PageCoordinates::BottomLeft)
      | (Rotation::FlippedHorizontal, PageCoordinates::TopLeft) => flipped_horizontal_t,
      (Rotation::Flipped, PageCoordinates::TopLeft | PageCoordinates::BottomLeft) => {
        ({
          let Ok(explicit_value_2) = Transform::scale(-1.0);
          explicit_value_2
        }) * ({
          let Ok(explicit_value_3) =
            Transform::translate((f64::from(-width), f64::from(-height)));
          explicit_value_3
        })
      }
    };

    let inversion_transform = match coordinates {
      PageCoordinates::TopLeft => {
        let Ok(explicit_value_4) =
          Transform::new([1.0, 0.0, 0.0, -1.0, 0.0, f64::from(base_height)]);
        explicit_value_4
      }
      PageCoordinates::BottomLeft => Transform::IDENTITY,
    };

    Ok(
      rotation_transform
        * inversion_transform
        * ({
          let Ok(explicit_value_5) = Transform::translate((-crop_box.x0, -crop_box.y0));
          explicit_value_5
        }),
    )
  }
}

#[derive(Clone, Debug)]
pub struct Resources<'a> {
  parent: Option<Box<Self>>,
  ctx: ReaderContext<'a>,

  pub ext_g_states: Dict<'a>,

  pub fonts: Dict<'a>,

  pub properties: Dict<'a>,

  pub color_spaces: Dict<'a>,

  pub x_objects: Dict<'a>,

  pub patterns: Dict<'a>,

  pub shadings: Dict<'a>,
}

struct ResourceAncestors<'owner, 'data> {
  next: Option<&'owner Resources<'data>>,
}

impl<'owner, 'data> Iterator for ResourceAncestors<'owner, 'data> {
  type Item = &'owner Resources<'data>;

  fn next(&mut self) -> Option<Self::Item> {
    let resources = self.next?;
    self.next = resources.parent.as_deref();

    Some(resources)
  }
}

fn resource_dictionary<'a>(
  resources: &Dict<'a>,
  key: &[u8],
) -> core::result::Result<Dict<'a>, console_core_never::Never> {
  let Ok(found) = resources.get::<Dict<'a>>(key);

  match found {
    Some(dictionary) => Ok(dictionary),
    None => Dict::empty(),
  }
}

impl<'a> Resources<'a> {
  pub fn from_parent(
    resources: Dict<'a>,
    parent: Self,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let ctx = parent.ctx.clone();

    Self::new(resources, Some(parent), &ctx)
  }

  pub(crate) fn new(
    resources: Dict<'a>,
    parent: Option<Self>,
    ctx: &ReaderContext<'a>,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let Ok(ext_g_states) = resource_dictionary(&resources, EXT_G_STATE);
    let Ok(fonts) = resource_dictionary(&resources, FONT);
    let Ok(color_spaces) = resource_dictionary(&resources, COLORSPACE);
    let Ok(x_objects) = resource_dictionary(&resources, XOBJECT);
    let Ok(patterns) = resource_dictionary(&resources, PATTERN);
    let Ok(shadings) = resource_dictionary(&resources, SHADING);
    let Ok(properties) = resource_dictionary(&resources, PROPERTIES);

    Ok(Self {
      parent: parent.map(Box::new),
      ext_g_states,
      fonts,
      color_spaces,
      properties,
      x_objects,
      patterns,
      shadings,
      ctx: ctx.clone(),
    })
  }

  fn get_resource<T, Choose>(
    &self,
    name: &Name<'_>,
    choose: Choose,
  ) -> core::result::Result<Option<T>, console_core_never::Never>
  where
    T: ObjectLike<'a>,
    Choose: for<'owner> Fn(&'owner Self) -> &'owner Dict<'a>,
  {
    let ancestors = ResourceAncestors { next: Some(self) };

    for resources in ancestors {
      let dictionary = choose(resources);
      let Ok(found) = dictionary.get::<T>(name.deref());

      match found {
        Some(value) => return Ok(Some(value)),
        None => {}
      }
    }

    Ok(None)
  }

  pub fn parent(&self) -> core::result::Result<Option<&Self>, console_core_never::Never> {
    Ok(self.parent.as_deref())
  }

  pub fn get_ext_g_state(
    &self,
    name: &Name<'_>,
  ) -> core::result::Result<Option<Dict<'a>>, console_core_never::Never> {
    self.get_resource(name, |resources| &resources.ext_g_states)
  }

  pub fn get_color_space(
    &self,
    name: &Name<'_>,
  ) -> core::result::Result<Option<Object<'a>>, console_core_never::Never> {
    self.get_resource(name, |resources| &resources.color_spaces)
  }

  pub fn get_font(
    &self,
    name: &Name<'_>,
  ) -> core::result::Result<Option<Dict<'a>>, console_core_never::Never> {
    self.get_resource(name, |resources| &resources.fonts)
  }

  pub fn get_pattern(
    &self,
    name: &Name<'_>,
  ) -> core::result::Result<Option<Object<'a>>, console_core_never::Never> {
    self.get_resource(name, |resources| &resources.patterns)
  }

  pub fn get_x_object(
    &self,
    name: &Name<'_>,
  ) -> core::result::Result<Option<Stream<'a>>, console_core_never::Never> {
    self.get_resource(name, |resources| &resources.x_objects)
  }

  pub fn get_shading(
    &self,
    name: &Name<'_>,
  ) -> core::result::Result<Option<Object<'a>>, console_core_never::Never> {
    self.get_resource(name, |resources| &resources.shadings)
  }
}

const POINTS_PER_INCH: f64 = 72.0;
const POINTS_PER_MM: f64 = 1.0 / (10.0 * 2.54) * POINTS_PER_INCH;

pub const A4: Rectangle = Rectangle {
  x0: 0.0,
  y0: 0.0,
  x1: 210.0 * POINTS_PER_MM,
  y1: 297.0 * POINTS_PER_MM,
};

pub(crate) mod cached {
  use crate::page::Pages;
  use crate::reader::ReaderContext;
  use crate::xref::XRef;
  use console_core_never::Never;

  mod storage {
    #![cfg_attr(
      dylint_lib = "explicit002_infallible_result",
      allow(
        explicit002_infallible_result,
        reason = "self_cell generates owner accessors whose signatures are fixed by its self-referential storage contract"
      )
    )]

    use super::{Pages, XRef};

    self_cell::self_cell!(
      pub(super) struct PagesCache {
        owner: XRef,

        #[covariant]
        dependent: Pages,
      }
    );
  }

  use storage::PagesCache;

  pub(crate) struct CachedPages(PagesCache);

  enum MissingPages {
    NotFound,
  }

  fn load(xref: &XRef) -> Result<Pages<'_>, MissingPages> {
    let Ok(context) = ReaderContext::new(xref, crate::ObjectReadMode::Document);
    let Ok(trailer) = xref.trailer_data();
    let Ok(found) = xref.get_with(trailer.pages_ref, &context);
    let from_tree = found.and_then(|dictionary| {
      let Ok(pages) = Pages::new(&dictionary, &context, xref);

      pages
    });
    let pages = from_tree.or_else(|| {
      let Ok(fallback) = Pages::new_brute_force(&context, xref);

      fallback
    });

    pages.ok_or(MissingPages::NotFound)
  }

  impl CachedPages {
    pub(crate) fn new(xref: XRef) -> Result<Option<Self>, Never> {
      Ok(match PagesCache::try_new(xref, load) {
        Ok(cache) => Some(Self(cache)),
        Err(MissingPages::NotFound) => None,
      })
    }

    pub(crate) fn xref(&self) -> Result<&XRef, Never> {
      Ok(self.0.borrow_owner())
    }

    pub(crate) fn get(&self) -> Result<&Pages<'_>, Never> {
      Ok(self.0.borrow_dependent())
    }
  }
}

#[cfg(test)]
mod coordinate_tests {
  use super::*;

  #[test]
  fn the_cropped_page_maps_to_both_origins_in_every_rotation() {
    let Ok(inner) = Dict::empty();
    let Ok(ctx) = ReaderContext::dummy();
    let Ok(resources) = Resources::new(inner.clone(), None, &ctx);
    let Ok(media_box) = Rectangle::new(10.0, 20.0, 310.0, 420.0);
    let Ok(crop_box) = Rectangle::new(30.0, 40.0, 130.0, 240.0);
    let mut page = Page {
      inner,
      media_box,
      crop_box,
      rotation: Rotation::None,
      page_streams: OnceLock::new(),
      resources,
      ctx,
    };

    for (rotation, top_left, bottom_left) in [
      (
        Rotation::None,
        [1.0, 0.0, 0.0, -1.0, -30.0, 240.0],
        [1.0, 0.0, 0.0, 1.0, -30.0, -40.0],
      ),
      (
        Rotation::Horizontal,
        [0.0, 1.0, 1.0, 0.0, -40.0, -30.0],
        [0.0, -1.0, 1.0, 0.0, -40.0, 130.0],
      ),
      (
        Rotation::Flipped,
        [-1.0, 0.0, 0.0, 1.0, 130.0, -40.0],
        [-1.0, 0.0, 0.0, -1.0, 130.0, 240.0],
      ),
      (
        Rotation::FlippedHorizontal,
        [0.0, -1.0, -1.0, 0.0, 240.0, 130.0],
        [0.0, 1.0, -1.0, 0.0, 240.0, -30.0],
      ),
    ] {
      page.rotation = rotation;

      let Ok(top) = page.initial_transform(PageCoordinates::TopLeft);
      let Ok(bottom) = page.initial_transform(PageCoordinates::BottomLeft);

      assert_eq!(top.as_coeffs(), Ok(top_left), "{rotation:?}");
      assert_eq!(bottom.as_coeffs(), Ok(bottom_left), "{rotation:?}");
    }
  }
}
