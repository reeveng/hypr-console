//! PDF shadings.

use crate::CacheKey;
use crate::cache::Cache;
use crate::color::{ColorComponents, ColorSpace};
use crate::util::{Nearness, RectExt};
use console_core_never::Never;
use console_core_number_conversion::{nearest_f32_from_f64, nearest_f32_from_u64};
use console_core_portable_document_functions::{
  Function, Interpolation, PdfSample, Values, interpolate,
};
use console_core_portable_document_syntax::bit_reader::BitReader;
use console_core_portable_document_syntax::object::Array;
use console_core_portable_document_syntax::object::Dict;
use console_core_portable_document_syntax::object::Rectangle;
use console_core_portable_document_syntax::object::Stream;
use console_core_portable_document_syntax::object::dict::keys::{
  BACKGROUND, BBOX, BITS_PER_COMPONENT, BITS_PER_COORDINATE, BITS_PER_FLAG, COLORSPACE, COORDS,
  DECODE, DOMAIN, EXTEND, FUNCTION, MATRIX, SHADING_TYPE, VERTICES_PER_ROW,
};
use console_core_portable_document_syntax::object::{Object, PdfBoolean};
use kurbo::{Affine, BezPath, CubicBez, ParamCurve, Point, Shape};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum ShadingFunction {
  Single(Function),

  Multiple(Vec<Function>),
}

impl ShadingFunction {
  pub fn eval(
    &self,
    input: &Values,
  ) -> core::result::Result<Option<Values>, console_core_never::Never> {
    Ok(match self {
      Self::Single(s) => {
        let Ok(eval) = s.eval(input.clone());

        eval
      }
      Self::Multiple(m) => {
        let mut out = vec![];

        for func in m {
          let Ok(eval) = func.eval(input.clone());
          out.push(
            *(match (match eval {
              Some(explicit_value) => explicit_value,
              None => return Ok(None),
            })
            .first()
            {
              Some(explicit_value) => explicit_value,
              None => return Ok(None),
            }),
          );
        }

        Some(out)
      }
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientGeometry {
  Axial,
  Radial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientExtension {
  Extend,
  Stop,
}

impl From<PdfBoolean> for GradientExtension {
  fn from(value: PdfBoolean) -> Self {
    match value {
      PdfBoolean::True => Self::Extend,
      PdfBoolean::False => Self::Stop,
    }
  }
}

#[derive(Clone, Copy)]
enum MeshColorEncoding {
  FunctionInput,
  DirectColor,
}

impl From<Option<&ShadingFunction>> for MeshColorEncoding {
  fn from(function: Option<&ShadingFunction>) -> Self {
    match function {
      Some(_) => Self::FunctionInput,
      None => Self::DirectColor,
    }
  }
}

#[derive(Debug)]
pub enum ShadingType {
  FunctionBased {
    domain: [f32; 4],

    matrix: Affine,

    function: ShadingFunction,
  },

  RadialAxial {
    coords: [f32; 6],

    domain: [f32; 2],

    function: ShadingFunction,

    extend: [GradientExtension; 2],

    geometry: GradientGeometry,
  },

  TriangleMesh {
    triangles: Vec<Triangle>,

    function: Option<ShadingFunction>,
  },

  CoonsPatchMesh {
    patches: Vec<CoonsPatch>,

    function: Option<ShadingFunction>,
  },

  TensorProductPatchMesh {
    patches: Vec<TensorProductPatch>,

    function: Option<ShadingFunction>,
  },

  Dummy,
}

#[derive(Clone, Debug)]
pub struct Shading {
  cache_key: u128,

  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "Cached shadings and deferred device operations independently retain this public immutable mesh"
    )
  )]
  pub shading_type: Arc<ShadingType>,

  pub color_space: ColorSpace,

  pub clip_path: Option<BezPath>,

  pub background: Option<Vec<f32>>,
}

impl Shading {
  #[cfg_attr(
    dylint_lib = "explicit081_no_shared_ownership",
    expect(
      explicit081_no_shared_ownership,
      reason = "The public shading field holds immutable geometry shared with deferred device operations"
    )
  )]
  pub(crate) fn new(
    dict: &Dict<'_>,
    stream: Option<&Stream<'_>>,
    cache: &Cache,
  ) -> core::result::Result<Option<Self>, console_core_never::Never> {
    let Ok(number) = dict.get::<u8>(SHADING_TYPE);

    let shading_number = match number {
      Some(matched_shading_number) => matched_shading_number,
      ::core::option::Option::None => return Ok(None),
    };

    let Ok(object) = dict.get(COLORSPACE);

    let color_space_object = match object {
      Some(matched_color_space_object) => matched_color_space_object,
      ::core::option::Option::None => return Ok(None),
    };

    let Ok(space) = ColorSpace::new(color_space_object, cache);

    let color_space = match space {
      Some(matched_color_space) => matched_color_space,
      ::core::option::Option::None => return Ok(None),
    };

    let Ok(kind) = match shading_number {
      1 => read_function_shading(dict, &color_space),
      2 => read_gradient(dict, &color_space, GradientGeometry::Axial),
      3 => read_gradient(dict, &color_space, GradientGeometry::Radial),
      4..=7 => match stream {
        Some(mesh_stream) => read_mesh(dict, mesh_stream, &color_space, shading_number),
        None => Ok(None),
      },
      _unsupported_shading => Ok(None),
    };

    let shading_type = match kind {
      Some(matched_shading_type) => matched_shading_type,
      ::core::option::Option::None => return Ok(None),
    };

    let Ok(bbox) = dict.get::<Rectangle>(BBOX);
    let Ok(background_colors) = dict.get::<Array<'_>>(BACKGROUND);

    Ok(Some(Self {
      cache_key: dict.cache_key(),
      shading_type: Arc::new(shading_type),
      color_space,
      clip_path: bbox.map(|rect| rect.to_kurbo().to_path(0.1)),
      background: background_colors.map(|array| {
        let Ok(components) = array.iter::<f32>();
        components.collect()
      }),
    }))
  }
}

fn read_function_shading(
  dict: &Dict<'_>,
  color_space: &ColorSpace,
) -> Result<Option<ShadingType>, Never> {
  let Ok(domain_values) = dict.get::<[f32; 4]>(DOMAIN);
  let domain = match domain_values {
    Some(domain) => domain,
    None => [0.0, 1.0, 0.0, 1.0],
  };
  let Ok(transform) = dict.get::<[f64; 6]>(MATRIX);
  let matrix = match transform {
    Some(transform_2) => Affine::new(transform_2),
    None => Affine::default(),
  };
  let Ok(sampled_function) = read_function(dict, color_space);

  Ok(sampled_function.map(|function| ShadingType::FunctionBased {
    domain,
    matrix,
    function,
  }))
}

fn read_gradient(
  dict: &Dict<'_>,
  color_space: &ColorSpace,
  geometry: GradientGeometry,
) -> Result<Option<ShadingType>, Never> {
  let Ok(domain_values) = dict.get::<[f32; 2]>(DOMAIN);
  let domain = match domain_values {
    Some(domain) => domain,
    None => [0.0, 1.0],
  };
  let Ok(sampled_function) = read_function(dict, color_space);

  let function = match sampled_function {
    Some(matched_function) => matched_function,
    ::core::option::Option::None => return Ok(None),
  };

  let Ok(extension_options) = dict.get::<[PdfBoolean; 2]>(EXTEND);
  let extend = match extension_options {
    Some(extension) => extension.map(GradientExtension::from),
    None => [GradientExtension::Stop; 2],
  };
  let Ok(coordinates) = gradient_coordinates(dict, geometry);

  Ok(match coordinates {
    Some(GradientCoordinates::Draw(coords)) => Some(ShadingType::RadialAxial {
      domain,
      function,
      extend,
      coords,
      geometry,
    }),
    Some(GradientCoordinates::Empty) => Some(ShadingType::Dummy),
    None => None,
  })
}

enum GradientCoordinates {
  Draw([f32; 6]),
  Empty,
}

fn gradient_coordinates(
  dict: &Dict<'_>,
  geometry: GradientGeometry,
) -> Result<Option<GradientCoordinates>, Never> {
  let (coords, empty) = match geometry {
    GradientGeometry::Axial => {
      let Ok(read) = dict.get::<[f32; 4]>(COORDS);

      let (x0, y0, x1, y1) = match read {
        Some([matched_x0, matched_y0, matched_x1, matched_y1]) => {
          (matched_x0, matched_y0, matched_x1, matched_y1)
        }
        ::core::option::Option::None => return Ok(None),
      };

      let empty = matches!((x0 - x1).nearly(0.0), Ok(crate::util::Nearly::Equal))
        && matches!((y0 - y1).nearly(0.0), Ok(crate::util::Nearly::Equal));

      ([x0, y0, x1, y1, 0.0, 0.0], empty)
    }
    GradientGeometry::Radial => {
      let Ok(read) = dict.get::<[f32; 6]>(COORDS);

      let coords = match read {
        Some(coordinates) => coordinates,
        None => return Ok(None),
      };
      let [x0, y0, r0, x1, y1, r1] = coords;

      let empty = matches!((x0 - x1).nearly(0.0), Ok(crate::util::Nearly::Equal))
        && matches!((y0 - y1).nearly(0.0), Ok(crate::util::Nearly::Equal))
        && matches!((r0 - r1).nearly(0.0), Ok(crate::util::Nearly::Equal));

      (coords, empty)
    }
  };

  Ok(Some(match ::core::primitive::bool::then_some(empty, ()) {
    ::core::option::Option::Some(()) => GradientCoordinates::Empty,
    ::core::option::Option::None => GradientCoordinates::Draw(coords),
  }))
}

#[derive(Clone, Copy)]
struct SampleWidths {
  coordinate: u8,
  component: u8,
}

#[derive(Clone, Copy)]
enum MeshTopology {
  FreeForm { flag_bits: u8 },
  Lattice { vertices_per_row: u32 },
  Coons { flag_bits: u8 },
  Tensor { flag_bits: u8 },
}

fn mesh_topology(dict: &Dict<'_>, shading_number: u8) -> Result<Option<MeshTopology>, Never> {
  let Ok(flag_width) = dict.get::<u8>(BITS_PER_FLAG);

  Ok(match shading_number {
    4 => flag_width.map(|flag_bits| MeshTopology::FreeForm { flag_bits }),
    5 => {
      let Ok(row_width) = dict.get::<u32>(VERTICES_PER_ROW);
      row_width.map(|vertices_per_row| MeshTopology::Lattice { vertices_per_row })
    }
    6 => flag_width.map(|flag_bits| MeshTopology::Coons { flag_bits }),
    7 => flag_width.map(|flag_bits| MeshTopology::Tensor { flag_bits }),
    _unsupported_shading => None,
  })
}

fn read_mesh(
  dict: &Dict<'_>,
  stream: &Stream<'_>,
  color_space: &ColorSpace,
  shading_number: u8,
) -> Result<Option<ShadingType>, Never> {
  let data = match stream.decoded() {
    Ok(data) => data,
    Err(_invalid_stream_encoding) => return Ok(None),
  };
  let Ok(coord_bits) = dict.get::<u8>(BITS_PER_COORDINATE);
  let Ok(component_bits) = dict.get::<u8>(BITS_PER_COMPONENT);

  let (coordinate, component) = {
    let outcome_coordinate = (coord_bits, component_bits);

    match outcome_coordinate {
      (Some(matched_coordinate), Some(matched_component)) => {
        (matched_coordinate, matched_component)
      }
      (::core::option::Option::None, _) | (_, ::core::option::Option::None) => return Ok(None),
    }
  };

  let Ok(function) = read_function(dict, color_space);
  let Ok(decode_array) = dict.get::<Array<'_>>(DECODE);

  let decode_fields = match decode_array {
    Some(matched_decode_fields) => matched_decode_fields,
    ::core::option::Option::None => return Ok(None),
  };

  let Ok(decode_values) = decode_fields.iter::<f32>();
  let decode: Vec<_> = decode_values.collect();
  let Ok(layout) = mesh_topology(dict, shading_number);

  let topology = match layout {
    Some(matched_topology) => matched_topology,
    ::core::option::Option::None => return Ok(None),
  };

  let encoding = MeshColorEncoding::from(function.as_ref());
  let Ok(decoded_reader) = MeshReader::new(
    &data,
    SampleWidths {
      coordinate,
      component,
    },
    encoding,
    &decode,
  );

  let reader = match decoded_reader {
    Some(matched_reader) => matched_reader,
    ::core::option::Option::None => return Ok(None),
  };

  Ok(match topology {
    MeshTopology::FreeForm { flag_bits } => {
      let Ok(triangles) = read_free_form_triangles(reader, flag_bits);
      triangles.map(|triangles_2| ShadingType::TriangleMesh {
        triangles: triangles_2,
        function,
      })
    }
    MeshTopology::Lattice { vertices_per_row } => {
      let Ok(triangles) = read_lattice_triangles(reader, vertices_per_row);
      triangles.map(|triangles_3| ShadingType::TriangleMesh {
        triangles: triangles_3,
        function,
      })
    }
    MeshTopology::Coons { flag_bits } => {
      let Ok(patches) = read_patch_mesh(reader, flag_bits, PatchSize::Coons);
      patches.map(|patches_2| ShadingType::CoonsPatchMesh {
        patches: patches_2
          .into_iter()
          .map(|patch| {
            let [p0, p1, p2, p3, p4, p5, p6, p7, p8, p9, p10, p11, ..] = patch.points;
            CoonsPatch {
              control_points: [p0, p1, p2, p3, p4, p5, p6, p7, p8, p9, p10, p11],
              colors: patch.colors,
            }
          })
          .collect(),
        function,
      })
    }
    MeshTopology::Tensor { flag_bits } => {
      let Ok(patches) = read_patch_mesh(reader, flag_bits, PatchSize::Tensor);
      patches.map(|patches_3| ShadingType::TensorProductPatchMesh {
        patches: patches_3
          .into_iter()
          .map(|patch| TensorProductPatch {
            control_points: patch.points,
            colors: patch.colors,
          })
          .collect(),
        function,
      })
    }
  })
}

impl CacheKey for Shading {
  fn cache_key(&self) -> u128 {
    self.cache_key
  }
}

#[derive(Clone, Debug)]
pub struct Triangle {
  pub p0: TriangleVertex,

  pub p1: TriangleVertex,

  pub p2: TriangleVertex,
  kurbo_tri: kurbo::Triangle,
  dot_products: EdgeDotProducts,
}

#[derive(Clone, Debug)]
struct EdgeDotProducts([f64; 3]);

impl Triangle {
  pub fn new(
    p0: TriangleVertex,
    p1: TriangleVertex,
    p2: TriangleVertex,
  ) -> core::result::Result<Self, console_core_never::Never> {
    let v0 = p1.point - p0.point;
    let v1 = p2.point - p0.point;

    let d00 = v0.dot(v0);
    let d01 = v0.dot(v1);
    let d11 = v1.dot(v1);

    let kurbo_tri = kurbo::Triangle::new(p0.point, p1.point, p2.point);

    Ok(Self {
      p0,
      p1,
      kurbo_tri,
      p2,
      dot_products: EdgeDotProducts([d00, d01, d11]),
    })
  }

  pub fn interpolate(
    &self,
    pos: Point,
  ) -> core::result::Result<ColorComponents, console_core_never::Never> {
    let Ok((u, v, w)) = self.barycentric_coords(pos);

    let mut result = vec![];

    for ((c0, c1), c2) in self
      .p0
      .colors
      .iter()
      .zip(&self.p1.colors)
      .zip(&self.p2.colors)
    {
      result.push(u * c0 + v * c1 + w * c2);
    }

    Ok(result)
  }

  pub fn contains_point(
    &self,
    pos: Point,
  ) -> core::result::Result<crate::Containment, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some(self.kurbo_tri.winding(pos) != 0, ()) {
        ::core::option::Option::Some(()) => crate::Containment::Inside,
        ::core::option::Option::None => crate::Containment::Outside,
      },
    )
  }

  pub fn bounding_box(&self) -> core::result::Result<kurbo::Rect, console_core_never::Never> {
    Ok(self.kurbo_tri.bounding_box())
  }

  fn barycentric_coords(
    &self,
    p: Point,
  ) -> core::result::Result<(f32, f32, f32), console_core_never::Never> {
    let (a, b, c) = (self.p0.point, self.p1.point, self.p2.point);
    let v0 = b - a;
    let v1 = c - a;
    let v2 = p - a;

    let EdgeDotProducts([d00, d01, d11]) = self.dot_products;
    let d20 = v2.dot(v0);
    let d21 = v2.dot(v1);

    let cross_term_squared = d01 * d01;
    let denom = d00 * d11 - cross_term_squared;
    let v = (d11 * d20 - d01 * d21) / denom;
    let w = (d00 * d21 - d01 * d20) / denom;
    let ::core::result::Result::Ok(u) =
      ::console_core_number_conversion::nearest_f32_from_f64(1.0 - v - w);

    Ok((
      u,
      match ::console_core_number_conversion::nearest_f32_from_f64(v) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      },
      match ::console_core_number_conversion::nearest_f32_from_f64(w) {
        ::core::result::Result::Ok(_converted_number) => _converted_number,
      },
    ))
  }
}

#[derive(Clone, Debug)]
pub struct TriangleVertex {
  flag: u32,

  pub point: Point,

  pub colors: ColorComponents,
}

#[derive(Clone, Debug)]
pub struct CoonsPatch {
  pub control_points: [Point; 12],

  pub colors: [ColorComponents; 4],
}

#[derive(Clone, Debug)]
pub struct TensorProductPatch {
  pub control_points: [Point; 16],

  pub colors: [ColorComponents; 4],
}

impl CoonsPatch {
  pub fn map_coordinate(
    &self,
    p: Point,
  ) -> core::result::Result<Point, console_core_never::Never> {
    let (u, v) = (p.x, p.y);

    let [p0, p1, p2, p3, p4, p5, p6, p7, p8, p9, p10, p11] = self.control_points;
    let c1 = CubicBez::new(p0, p11, p10, p9);
    let c2 = CubicBez::new(p3, p4, p5, p6);
    let d1 = CubicBez::new(p0, p1, p2, p3);
    let d2 = CubicBez::new(p9, p8, p7, p6);

    let sc = (1.0 - v) * c1.eval(u).to_vec2() + v * c2.eval(u).to_vec2();
    let sd = (1.0 - u) * d1.eval(v).to_vec2() + u * d2.eval(v).to_vec2();
    let sb = (1.0 - v) * ((1.0 - u) * c1.eval(0.0).to_vec2() + u * c1.eval(1.0).to_vec2())
      + v * ((1.0 - u) * c2.eval(0.0).to_vec2() + u * c2.eval(1.0).to_vec2());

    Ok((sc + sd - sb).to_point())
  }

  pub fn to_triangles(
    &self,
    buffer: &mut Vec<Triangle>,
  ) -> core::result::Result<(), console_core_never::Never> {
    generate_patch_triangles(
      |point| self.map_coordinate(point),
      |point| self.interpolate(point),
      buffer,
    )
  }

  pub fn interpolate(
    &self,
    pos: Point,
  ) -> core::result::Result<ColorComponents, console_core_never::Never> {
    interpolate_patch_color(pos, &self.colors)
  }
}

impl TensorProductPatch {
  pub fn map_coordinate(&self, p: Point) -> Result<Point, Never> {
    let [
      p0,
      p1,
      p2,
      p3,
      p4,
      p5,
      p6,
      p7,
      p8,
      p9,
      p10,
      p11,
      p12,
      p13,
      p14,
      p15,
    ] = self.control_points;
    let ordered = [
      [p0, p1, p2, p3],
      [p11, p12, p13, p4],
      [p10, p15, p14, p5],
      [p9, p8, p7, p6],
    ];
    Ok(kurbo::bezier::evaluate_bicubic_patch(ordered, p))
  }

  pub fn to_triangles(&self, buffer: &mut Vec<Triangle>) -> Result<(), Never> {
    generate_patch_triangles(
      |point| self.map_coordinate(point),
      |point| self.interpolate(point),
      buffer,
    )
  }

  pub fn interpolate(&self, pos: Point) -> Result<ColorComponents, Never> {
    interpolate_patch_color(pos, &self.colors)
  }
}

fn interpolate_patch_color(
  pos: Point,
  [c0, c1, c2, c3]: &[ColorComponents; 4],
) -> Result<ColorComponents, Never> {
  let (u, v) = (pos.x, pos.y);
  let mut result = Vec::new();

  for (((first, second), third), fourth) in c0.iter().zip(c1).zip(c2).zip(c3) {
    let value = (1.0 - u) * (1.0 - v) * f64::from(*first)
      + u * (1.0 - v) * f64::from(*fourth)
      + u * v * f64::from(*third)
      + (1.0 - u) * v * f64::from(*second);
    let Ok(converted) = nearest_f32_from_f64(value);
    result.push(converted);
  }

  Ok(result)
}

fn read_free_form_triangles(
  mut reader: MeshReader<'_>,
  flag_bits: u8,
) -> Result<Option<Vec<Triangle>>, Never> {
  let mut triangles = Vec::new();
  let mut previous: Option<[TriangleVertex; 3]> = None;

  for _stream_byte in reader.bits.data {
    let first = match reader.vertex(VertexFlag::Read(flag_bits)) {
      Ok(vertex) => vertex,
      Err(_incomplete_trailing_vertex) => break,
    };

    let next = match (first.flag, previous) {
      (0, _previous) => {
        let second = match reader.vertex(VertexFlag::Read(flag_bits)) {
          Ok(vertex) => vertex,
          Err(_incomplete_second_vertex) => return Ok(None),
        };
        let third = match reader.vertex(VertexFlag::Read(flag_bits)) {
          Ok(vertex) => vertex,
          Err(_incomplete_third_vertex) => return Ok(None),
        };

        [first, second, third]
      }
      (1, Some([_a, b, c])) => [b, c, first],
      (2, Some([a, _b, c])) => [a, c, first],
      (3.., Some(held)) => held,
      (_, None) => return Ok(None),
    };
    let [p0, p1, p2] = &next;

    match (
      crate::util::nearly_same(p0.point, p1.point),
      crate::util::nearly_same(p1.point, p2.point),
    ) {
      (Ok(crate::util::Sameness::Same), _) | (_, Ok(crate::util::Sameness::Same)) => {}
      _ => {
        let Ok(triangle) = Triangle::new(p0.clone(), p1.clone(), p2.clone());
        triangles.push(triangle);
      }
    }

    previous = Some(next);
  }

  Ok(Some(triangles))
}

enum VertexFlag {
  Unflagged,
  Read(u8),
}

enum MeshReadError {
  SampleUnavailable,
  MissingFunctionBounds,
  MissingPreviousPatch,
}

struct BitSample {
  width: u8,
  domain: [f32; 2],
}

impl BitSample {
  fn new(width: u8) -> Result<Self, Never> {
    Ok(Self {
      width,
      domain: [0.0, 2.0_f32.powi(i32::from(width)) - 1.0],
    })
  }

  fn decode(&self, bits: &mut BitReader<'_>, range: [f32; 2]) -> Result<f32, MeshReadError> {
    let Ok(encoded) = bits.read(self.width);
    let checked_value = encoded.ok_or(MeshReadError::SampleUnavailable)?;
    let Ok(sample) = nearest_f32_from_u64(u64::from(checked_value));
    let Ok(decoded) = interpolate(
      PdfSample(sample),
      Interpolation {
        domain: self.domain,
        range,
      },
    );
    Ok(decoded)
  }
}

struct MeshReader<'a> {
  bits: BitReader<'a>,
  coordinate: BitSample,
  component: BitSample,
  x_range: [f32; 2],
  y_range: [f32; 2],
  color_bounds: &'a [[f32; 2]],
  encoding: MeshColorEncoding,
}

impl<'a> MeshReader<'a> {
  fn new(
    data: &'a [u8],
    widths: SampleWidths,
    encoding: MeshColorEncoding,
    decode: &'a [f32],
  ) -> Result<Option<Self>, Never> {
    let (x_min, x_max, y_min, y_max, component_bounds) = {
      let optional_x_min = decode.split_first_chunk::<4>();

      match optional_x_min {
        Some((
          [matched_x_min, matched_x_max, matched_y_min, matched_y_max],
          matched_component_bounds,
        )) => (
          matched_x_min,
          matched_x_max,
          matched_y_min,
          matched_y_max,
          matched_component_bounds,
        ),
        ::core::option::Option::None => return Ok(None),
      }
    };

    let (color_bounds, _unpaired_bound) = component_bounds.as_chunks::<2>();
    let Ok(bits) = BitReader::new(data);
    let Ok(coordinate) = BitSample::new(widths.coordinate);
    let Ok(component) = BitSample::new(widths.component);

    Ok(Some(Self {
      bits,
      coordinate,
      component,
      x_range: [*x_min, *x_max],
      y_range: [*y_min, *y_max],
      color_bounds,
      encoding,
    }))
  }

  fn point(&mut self) -> Result<Point, MeshReadError> {
    let x = self.coordinate.decode(&mut self.bits, self.x_range)?;
    let y = self.coordinate.decode(&mut self.bits, self.y_range)?;
    Ok(Point::new(f64::from(x), f64::from(y)))
  }

  fn colors(&mut self) -> Result<ColorComponents, MeshReadError> {
    let bounds = match self.encoding {
      MeshColorEncoding::FunctionInput => {
        let first = self
          .color_bounds
          .first()
          .ok_or(MeshReadError::MissingFunctionBounds)?;
        core::slice::from_ref(first)
      }
      MeshColorEncoding::DirectColor => self.color_bounds,
    };
    let mut colors = Vec::new();

    for range in bounds {
      let decoded = self.component.decode(&mut self.bits, *range)?;
      colors.push(decoded);
    }

    Ok(colors)
  }

  fn vertex(&mut self, flags: VertexFlag) -> Result<TriangleVertex, MeshReadError> {
    let flag = match flags {
      VertexFlag::Unflagged => 0,
      VertexFlag::Read(width) => {
        let Ok(flag) = self.bits.read(width);

        match flag {
          Some(value) => value,
          None => return Err(MeshReadError::SampleUnavailable),
        }
      }
    };
    let point = self.point()?;
    let colors = self.colors()?;
    let Ok(()) = self.bits.align();
    Ok(TriangleVertex {
      flag,
      point,
      colors,
    })
  }
}

#[derive(Clone, Copy)]
struct GridPoint {
  parameter: Point,
  mapped: Point,
}

fn generate_patch_triangles(
  map_coordinate: impl Fn(Point) -> Result<Point, Never>,
  interpolate: impl Fn(Point) -> Result<ColorComponents, Never>,
  buffer: &mut Vec<Triangle>,
) -> Result<(), Never> {
  const LAST_GRID_LINE: u8 = 19;
  let mut grid = Vec::new();

  for row_index in 0..=LAST_GRID_LINE {
    let mut row = Vec::new();

    for column_index in 0..=LAST_GRID_LINE {
      let u = f64::from(row_index) / f64::from(LAST_GRID_LINE);
      let v = f64::from(column_index) / f64::from(LAST_GRID_LINE);
      let parameter = Point::new(u, v);
      let Ok(mapped) = map_coordinate(parameter);
      row.push(GridPoint { parameter, mapped });
    }

    grid.push(row);
  }

  let Ok(rows) = neighbors(&grid);

  for (top, bottom) in rows {
    let Ok(upper) = neighbors(top);
    let Ok(lower) = neighbors(bottom);

    for ((p00, p01), (p10, p11)) in upper.zip(lower) {
      let [v00, v10, v01, v11] = [p00, p10, p01, p11].map(|point| {
        let Ok(colors) = interpolate(point.parameter);
        TriangleVertex {
          flag: 0,
          point: point.mapped,
          colors,
        }
      });
      let Ok(first) = Triangle::new(v00, v10.clone(), v01.clone());
      let Ok(second) = Triangle::new(v10, v11, v01);
      let Ok(inflated_first) = inflate_triangle(first);
      let Ok(inflated_second) = inflate_triangle(second);
      buffer.extend([inflated_first, inflated_second]);
    }
  }

  Ok(())
}

fn neighbors<T>(items: &[T]) -> Result<impl Iterator<Item = (&T, &T)>, Never> {
  Ok(items.iter().zip(items.iter().skip(1)))
}

fn inflate_triangle(mut triangle: Triangle) -> Result<Triangle, Never> {
  const INFLATION_FACTOR: f64 = 1.025;
  let middle = triangle.kurbo_tri.centroid();

  for vertex in [&mut triangle.p0, &mut triangle.p1, &mut triangle.p2] {
    vertex.point = middle + (vertex.point - middle) * INFLATION_FACTOR;
  }

  Ok(triangle)
}

fn read_lattice_triangles(
  mut reader: MeshReader<'_>,
  vertices_per_row: u32,
) -> Result<Option<Vec<Triangle>>, Never> {
  let width = {
    let optional_width = core::num::NonZeroU32::new(vertices_per_row);

    match optional_width {
      Some(matched_width) => matched_width,
      ::core::option::Option::None => return Ok(None),
    }
  };

  let mut previous: Option<Vec<TriangleVertex>> = None;
  let mut triangles = Vec::new();

  'rows: for _stream_byte in reader.bits.data {
    let mut row = Vec::new();

    for _ in 0..width.get() {
      let vertex = match reader.vertex(VertexFlag::Unflagged) {
        Ok(vertex) => vertex,
        Err(_incomplete_trailing_row) => break 'rows,
      };

      row.push(vertex);
    }

    match previous {
      Some(top) => {
        let Ok(upper) = neighbors(&top);
        let Ok(lower) = neighbors(&row);

        for ((a, b), (c, d)) in upper.zip(lower) {
          let Ok(first) = Triangle::new(a.clone(), c.clone(), b.clone());
          let Ok(second) = Triangle::new(d.clone(), c.clone(), b.clone());
          triangles.extend([first, second]);
        }
      }
      None => {}
    }

    previous = Some(row);
  }

  Ok(Some(triangles))
}

#[derive(Clone, Copy)]
enum PatchSize {
  Coons,
  Tensor,
}

#[derive(Clone, Copy)]
enum PatchEdge {
  First,
  Second,
  Third,
}

enum PatchFlag {
  Fresh,
  Shared(PatchEdge),
}

struct PatchData {
  points: [Point; 16],
  colors: [ColorComponents; 4],
}

impl PatchData {
  fn read(
    reader: &mut MeshReader<'_>,
    flag: PatchFlag,
    size: PatchSize,
    previous: Option<&Self>,
  ) -> Result<Self, MeshReadError> {
    let mut result = Self {
      points: [Point::ZERO; 16],
      colors: [vec![], vec![], vec![], vec![]],
    };
    let [
      p0,
      p1,
      p2,
      p3,
      p4,
      p5,
      p6,
      p7,
      p8,
      p9,
      p10,
      p11,
      p12,
      p13,
      p14,
      p15,
    ] = &mut result.points;
    let [c0, c1, c2, c3] = &mut result.colors;

    match flag {
      PatchFlag::Fresh => {
        for point in [p0, p1, p2, p3] {
          let checked_value = reader.point()?;
          *point = checked_value;
        }
      }
      PatchFlag::Shared(edge) => {
        let checked_value_2 = previous.ok_or(MeshReadError::MissingPreviousPatch)?;
        let Ok(points) = checked_value_2.edge_points(edge);
        let [a, b, c, d] = points;
        *p0 = a;
        *p1 = b;
        *p2 = c;
        *p3 = d;
      }
    }

    for point in [p4, p5, p6, p7, p8, p9, p10, p11] {
      let checked_value_3 = reader.point()?;
      *point = checked_value_3;
    }

    match size {
      PatchSize::Coons => {}
      PatchSize::Tensor => {
        for point in [p12, p13, p14, p15] {
          let checked_value_4 = reader.point()?;
          *point = checked_value_4;
        }
      }
    }

    match flag {
      PatchFlag::Fresh => {
        let first_color = reader.colors()?;
        *c0 = first_color;
        let second_color = reader.colors()?;
        *c1 = second_color;
      }
      PatchFlag::Shared(edge) => {
        let checked_value_7 = previous.ok_or(MeshReadError::MissingPreviousPatch)?;
        let Ok([first, second]) = checked_value_7.edge_colors(edge);
        *c0 = first.clone();
        *c1 = second.clone();
      }
    }

    let third_color = reader.colors()?;
    *c2 = third_color;
    let fourth_color = reader.colors()?;
    *c3 = fourth_color;
    Ok(result)
  }

  fn edge_points(&self, edge: PatchEdge) -> Result<[Point; 4], Never> {
    let [p0, _, _, p3, p4, p5, p6, p7, p8, p9, p10, p11, ..] = self.points;
    Ok(match edge {
      PatchEdge::First => [p3, p4, p5, p6],
      PatchEdge::Second => [p6, p7, p8, p9],
      PatchEdge::Third => [p9, p10, p11, p0],
    })
  }

  fn edge_colors(&self, edge: PatchEdge) -> Result<[&ColorComponents; 2], Never> {
    let [c0, c1, c2, c3] = &self.colors;
    Ok(match edge {
      PatchEdge::First => [c1, c2],
      PatchEdge::Second => [c2, c3],
      PatchEdge::Third => [c3, c0],
    })
  }
}

fn read_patch_mesh(
  mut reader: MeshReader<'_>,
  flag_bits: u8,
  size: PatchSize,
) -> Result<Option<Vec<PatchData>>, Never> {
  let mut patches = Vec::new();

  for _stream_bit in reader.bits.data.iter().flat_map(|_| 0_u8..8) {
    let Ok(encoded) = reader.bits.read(flag_bits);
    let flag = match encoded {
      Some(0) => PatchFlag::Fresh,
      Some(1) => PatchFlag::Shared(PatchEdge::First),
      Some(2) => PatchFlag::Shared(PatchEdge::Second),
      Some(3) => PatchFlag::Shared(PatchEdge::Third),
      Some(_unsupported_flag) => break,
      None => break,
    };

    let patch = match PatchData::read(&mut reader, flag, size, patches.last()) {
      Ok(patch) => patch,
      Err(_incomplete_patch) => return Ok(None),
    };

    patches.push(patch);
  }

  Ok(Some(patches))
}

fn read_function(
  dict: &Dict<'_>,
  color_space: &ColorSpace,
) -> Result<Option<ShadingFunction>, Never> {
  let Ok(array) = dict.get::<Array<'_>>(FUNCTION);

  let entries = match array {
    Some(entries) => entries,
    None => {
      let Ok(single) = dict.get::<Object<'_>>(FUNCTION);

      return Ok(
        single
          .and_then(|object| {
            let Ok(function) = Function::new(&object);
            function
          })
          .map(ShadingFunction::Single),
      );
    }
  };

  let Ok(objects) = entries.iter::<Object<'_>>();
  let collected: Option<Vec<_>> = objects
    .map(|object| {
      let Ok(function) = Function::new(&object);
      function
    })
    .collect();

  let functions = match collected {
    Some(matched_functions) => matched_functions,
    ::core::option::Option::None => return Ok(None),
  };

  let Ok(components) = color_space.num_components();
  let Ok(required) = console_core_number_conversion::index(components);

  Ok(match functions.len().cmp(&required) {
    core::cmp::Ordering::Equal => Some(ShadingFunction::Multiple(functions)),
    core::cmp::Ordering::Less | core::cmp::Ordering::Greater => {
      warn!("function array of shading has wrong size");
      None
    }
  })
}

#[cfg(test)]
mod tests {
  use super::{MeshColorEncoding, MeshReader, SampleWidths, read_lattice_triangles};

  #[derive(Debug)]
  enum FixtureError {
    MissingMeshReader,
  }

  #[test]
  fn zero_width_lattice_is_rejected() -> Result<(), FixtureError> {
    let Ok(fixture_reader) = MeshReader::new(
      &[0, 0, 0],
      SampleWidths {
        coordinate: 8,
        component: 8,
      },
      MeshColorEncoding::DirectColor,
      &[0.0, 1.0, 0.0, 1.0, 0.0, 1.0],
    );

    let reader = match fixture_reader {
      Some(matched_reader) => matched_reader,
      ::core::option::Option::None => return Err(FixtureError::MissingMeshReader),
    };

    assert!(matches!(read_lattice_triangles(reader, 0), Ok(None)));
    Ok(())
  }
}
