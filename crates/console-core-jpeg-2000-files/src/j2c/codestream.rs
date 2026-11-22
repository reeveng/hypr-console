//! Read and decode a JPEG2000 codestream, as described in Annex A.

use alloc::vec;
use alloc::vec::Vec;
use console_core_number_conversion::{fixed_index, index, wrapped, wrapped_index};

use super::DecodeSettings;
use super::bitplane::BITPLANE_BIT_SIZE;
use super::build::SubBandType;
use crate::error::{DecodeError, MarkerError, Result, ValidationError};
use crate::reader::BitReader;

const MAX_LAYER_COUNT: u8 = 32;
const MAX_RESOLUTION_COUNT: u8 = 32;
const MAX_PRECINCT_EXPONENT: u8 = 31;

#[derive(Debug)]
pub(crate) struct Header<'a> {
  pub(crate) size_data: SizeData,
  pub(crate) global_coding_style: CodingStyleDefault,
  pub(crate) component_infos: Vec<ComponentInfo>,
  pub(crate) ppm_packets: Vec<PpmPacket<'a>>,
  pub(crate) skipped_resolution_levels: u8,

  pub(crate) validation: crate::ValidationMode,
}

#[derive(Debug, Clone)]
pub(crate) struct PpmMarkerData<'a> {
  pub(crate) sequence_idx: u8,
  pub(crate) packets: Vec<PpmPacket<'a>>,
}

#[derive(Debug, Clone)]
pub(crate) struct PpmPacket<'a> {
  pub(crate) data: &'a [u8],
}

pub(crate) fn read_header<'a>(
  reader: &mut BitReader<'a>,
  settings: &DecodeSettings,
) -> Result<Header<'a>> {
  let marker = reader.read_marker()?;

  match marker {
    markers::SIZ => {}
    _ => return Err(DecodeError::Marker(MarkerError::Expected("SIZ"))),
  }

  let mut size_data = size_marker(reader)?;
  let MainMarkers {
    coding,
    quantization,
    component_coding,
    component_quantization,
    mut packets,
  } = read_main_markers(reader, &size_data.component_sizes)?;
  let cod = coding.ok_or(MarkerError::Missing("COD"))?;
  let qcd = quantization.ok_or(MarkerError::Missing("QCD"))?;

  let infos = size_data
    .component_sizes
    .iter()
    .zip(component_coding)
    .zip(component_quantization)
    .map(|((size_info, coding_2), quantization_2)| {
      let coding_style = match coding_2 {
        Some(mut component) => {
          component.flags.raw |= cod.component_parameters.flags.raw;
          component
        }
        None => cod.component_parameters.clone(),
      };
      let quantization_info = match quantization_2 {
        Some(component) => component,
        None => qcd.clone(),
      };

      ComponentInfo {
        size_info: *size_info,
        coding_style,
        quantization_info,
      }
    })
    .collect::<Vec<_>>();

  let skipped = skipped_resolutions(&size_data, &infos, settings)?;
  let reduction = 1u32.wrapping_shl(u32::from(skipped));
  size_data.x_resolution_shrink_factor =
    size_data.x_resolution_shrink_factor.wrapping_mul(reduction);
  size_data.y_resolution_shrink_factor =
    size_data.y_resolution_shrink_factor.wrapping_mul(reduction);
  packets.sort_by_key(|marker_2| marker_2.sequence_idx);

  let header = Header {
    size_data,
    global_coding_style: cod,
    component_infos: infos,
    ppm_packets: packets
      .into_iter()
      .flat_map(|marker_3| marker_3.packets)
      .filter(|packet| !packet.data.is_empty())
      .collect(),
    skipped_resolution_levels: skipped,
    validation: settings.validation,
  };
  validate(&header)?;
  Ok(header)
}

struct MainMarkers<'a> {
  coding: Option<CodingStyleDefault>,
  quantization: Option<QuantizationInfo>,
  component_coding: Vec<Option<CodingStyleComponent>>,
  component_quantization: Vec<Option<QuantizationInfo>>,
  packets: Vec<PpmMarkerData<'a>>,
}

fn read_main_markers<'a>(
  reader: &mut BitReader<'a>,
  components: &[ComponentSizeInfo],
) -> Result<MainMarkers<'a>> {
  let Ok(num_components) = wrapped::<_, u16>(components.len());
  let mut found = MainMarkers {
    coding: None,
    quantization: None,
    component_coding: vec![None; components.len()],
    component_quantization: vec![None; components.len()],
    packets: Vec::new(),
  };
  let Ok(bytes) = reader.remaining_bytes();

  for _ in 0..=bytes.div_ceil(2) {
    let Ok(peek) = reader.peek_marker();
    let marker = peek.ok_or(MarkerError::Invalid)?;

    match marker {
      markers::SOT => return Ok(found),
      markers::COD => {
        reader.read_marker()?;
        let Ok(payload) = cod_marker(reader);
        let cod = payload.ok_or(MarkerError::ParseFailure("COD"))?;
        found.coding = Some(cod);
      }
      markers::COC => {
        reader.read_marker()?;
        let Ok(payload) = coc_marker(reader, num_components);
        let (index, coc) = payload.ok_or(MarkerError::ParseFailure("COC"))?;
        let Ok(component) = wrapped_index(index);
        let slot = found
          .component_coding
          .get_mut(component)
          .ok_or(MarkerError::ParseFailure("COC"))?;
        *slot = Some(coc);
      }
      markers::QCD => {
        reader.read_marker()?;
        let Ok(payload) = qcd_marker(reader);
        let qcd = payload.ok_or(MarkerError::ParseFailure("QCD"))?;
        found.quantization = Some(qcd);
      }
      markers::QCC => {
        reader.read_marker()?;
        let Ok(payload) = qcc_marker(reader, num_components);
        let (index, qcc) = payload.ok_or(MarkerError::ParseFailure("QCC"))?;
        let Ok(component) = wrapped_index(index);
        let slot = found
          .component_quantization
          .get_mut(component)
          .ok_or(MarkerError::ParseFailure("QCC"))?;
        *slot = Some(qcc);
      }
      markers::RGN => {
        reader.read_marker()?;
        let Ok(skipped) = rgn_marker(reader);
        skipped.ok_or(MarkerError::ParseFailure("RGN"))?;
      }
      markers::TLM => {
        reader.read_marker()?;
        let Ok(skipped) = tlm_marker(reader);
        skipped.ok_or(MarkerError::ParseFailure("TLM"))?;
      }
      markers::COM => {
        reader.read_marker()?;
        let Ok(skipped) = com_marker(reader);
        skipped.ok_or(MarkerError::ParseFailure("COM"))?;
      }
      markers::PPM => {
        reader.read_marker()?;
        let Ok(payload) = ppm_marker(reader);
        let parsed = payload.ok_or(MarkerError::ParseFailure("PPM"))?;
        found.packets.push(parsed);
      }
      markers::CRG => {
        reader.read_marker()?;
        let Ok(_) = skip_marker_segment(reader);
      }
      0x30..=0x3f => {
        reader.read_marker()?;
      }
      _ => return Err(DecodeError::Marker(MarkerError::Unsupported)),
    }
  }

  Err(DecodeError::Marker(MarkerError::Invalid))
}

fn skipped_resolutions(
  size: &SizeData,
  infos: &[ComponentInfo],
  settings: &DecodeSettings,
) -> Result<u8> {
  let min_levels = infos
    .iter()
    .map(|info| {
      let Ok(levels) = info.num_resolution_levels();
      levels
    })
    .min()
    .ok_or(ValidationError::InvalidComponentMetadata)?;

  let Ok(skipped) = match settings.target_resolution {
    None => return Ok(0),
    Some((target_width, target_height)) => {
      let Ok(width) = size.image_width();
      let Ok(height) = size.image_height();
      let x_ratio = width
        .checked_div(target_width)
        .ok_or(ValidationError::InvalidDimensions)?;
      let y_ratio = height
        .checked_div(target_height)
        .ok_or(ValidationError::InvalidDimensions)?;
      let logarithm = match x_ratio.min(y_ratio).checked_ilog2() {
        Some(levels) => levels,
        None => 0,
      };
      wrapped::<_, u8>(logarithm)
    }
  };
  Ok(skipped.min(min_levels.saturating_sub(1)))
}

fn validate(header: &Header<'_>) -> Result<()> {
  for info in &header.component_infos {
    let max_resolution_idx =
      u8::wrapping_sub(info.coding_style.parameters.num_resolution_levels, 1);
    let Ok(exponents) = fixed_index(info.quantization_info.step_sizes.len());

    match exponents {
      0 => {
        return Err(<crate::error::DecodeError as ::core::convert::From<
          crate::error::ValidationError,
        >>::from(ValidationError::MissingPrecinctExponents));
      }
      _ => {}
    }

    match info.quantization_info.quantization_style {
      QuantizationStyle::ScalarDerived => {}
      QuantizationStyle::NoQuantization | QuantizationStyle::ScalarExpounded => {
        let needed = u64::from(max_resolution_idx).wrapping_mul(3);

        match exponents.cmp(&needed) {
          core::cmp::Ordering::Less | core::cmp::Ordering::Equal => {
            return Err(<crate::error::DecodeError as ::core::convert::From<
              crate::error::ValidationError,
            >>::from(ValidationError::InsufficientExponents));
          }
          core::cmp::Ordering::Greater => {}
        }
      }
    }
  }

  Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct ComponentInfo {
  pub(crate) size_info: ComponentSizeInfo,
  pub(crate) coding_style: CodingStyleComponent,
  pub(crate) quantization_info: QuantizationInfo,
}

impl ComponentInfo {
  pub(crate) fn exponent_mantissa(
    &self,
    sub_band_type: SubBandType,
    resolution: u8,
  ) -> Result<(u16, u16)> {
    let n_ll = self.coding_style.parameters.num_decomposition_levels;

    let sb_index = match sub_band_type {
      SubBandType::LowLow => u16::MAX,
      SubBandType::HighLow => 0,
      SubBandType::LowHigh => 1,
      SubBandType::HighHigh => 2,
    };

    let step_sizes = &self.quantization_info.step_sizes;

    match self.quantization_info.quantization_style {
      QuantizationStyle::NoQuantization | QuantizationStyle::ScalarExpounded => {
        let entry = match resolution {
          0 => step_sizes.first(),
          _ => {
            let first = u32::from(resolution)
              .wrapping_sub(1)
              .wrapping_mul(3)
              .wrapping_add(1);
            let Ok(position) = index(first.wrapping_add(u32::from(sb_index)));
            step_sizes.get(position)
          }
        };

        let checked_value = entry
          .map(|s| (s.exponent, s.mantissa))
          .ok_or(ValidationError::MissingStepSize)?;
        Ok(checked_value)
      }
      QuantizationStyle::ScalarDerived => {
        let (e_0, mantissa) = step_sizes
          .first()
          .map(|s| (s.exponent, s.mantissa))
          .ok_or(ValidationError::MissingStepSize)?;
        let n_b = match ::core::primitive::bool::then_some(resolution == 0, ()) {
          ::core::option::Option::Some(()) => {
            match console_core_number_conversion::wrapped::<u8, u16>(n_ll) {
              Ok(converted) => converted,
            }
          }
          ::core::option::Option::None => u16::wrapping_sub(
            u16::wrapping_add(
              match console_core_number_conversion::wrapped::<u8, u16>(n_ll) {
                Ok(converted) => converted,
              },
              1,
            ),
            match console_core_number_conversion::wrapped::<u8, u16>(resolution) {
              Ok(converted) => converted,
            },
          ),
        };

        let exponent = e_0
          .checked_sub(
            match console_core_number_conversion::wrapped::<u8, u16>(n_ll) {
              Ok(converted) => converted,
            },
          )
          .and_then(|e| e.checked_add(n_b))
          .ok_or(ValidationError::InvalidExponents)?;

        Ok((exponent, mantissa))
      }
    }
  }

  pub(crate) fn wavelet_transform(
    &self,
  ) -> core::result::Result<WaveletTransform, console_core_never::Never> {
    Ok(self.coding_style.parameters.transformation)
  }

  pub(crate) fn num_resolution_levels(
    &self,
  ) -> core::result::Result<u8, console_core_never::Never> {
    Ok(self.coding_style.parameters.num_resolution_levels)
  }

  pub(crate) fn num_decomposition_levels(
    &self,
  ) -> core::result::Result<u8, console_core_never::Never> {
    Ok(self.coding_style.parameters.num_decomposition_levels)
  }

  pub(crate) fn code_block_style(
    &self,
  ) -> core::result::Result<CodeBlockStyle, console_core_never::Never> {
    Ok(self.coding_style.parameters.code_block_style)
  }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ProgressionOrder {
  LayerResolutionComponentPosition,
  ResolutionLayerComponentPosition,
  ResolutionPositionComponentLayer,
  PositionComponentResolutionLayer,
  ComponentPositionResolutionLayer,
}

impl ProgressionOrder {
  fn from_u8(value: u8) -> Result<Self> {
    match value {
      0 => Ok(Self::LayerResolutionComponentPosition),
      1 => Ok(Self::ResolutionLayerComponentPosition),
      2 => Ok(Self::ResolutionPositionComponentLayer),
      3 => Ok(Self::PositionComponentResolutionLayer),
      4 => Ok(Self::ComponentPositionResolutionLayer),
      _ => Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::ValidationError,
      >>::from(ValidationError::InvalidProgressionOrder)),
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WaveletTransform {
  Irreversible97,
  Reversible53,
}

impl WaveletTransform {
  fn from_u8(value: u8) -> Result<Self> {
    match value {
      0 => Ok(Self::Irreversible97),
      1 => Ok(Self::Reversible53),
      _ => Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::ValidationError,
      >>::from(ValidationError::InvalidTransformation)),
    }
  }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct CodingStyleFlags {
  pub(crate) raw: u8,
}

impl CodingStyleFlags {
  fn from_u8(value: u8) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self { raw: value })
  }

  pub(crate) fn has_precincts(
    &self,
  ) -> core::result::Result<crate::states::Precincts, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some((self.raw & 0x01) != 0, ()) {
        ::core::option::Option::Some(()) => crate::states::Precincts::Explicit,
        ::core::option::Option::None => crate::states::Precincts::Default,
      },
    )
  }

  pub(crate) fn may_use_sop_markers(
    &self,
  ) -> core::result::Result<crate::states::SopMarkers, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some((self.raw & 0x02) != 0, ()) {
        ::core::option::Option::Some(()) => crate::states::SopMarkers::Permitted,
        ::core::option::Option::None => crate::states::SopMarkers::Excluded,
      },
    )
  }

  pub(crate) fn uses_eph_marker(
    &self,
  ) -> core::result::Result<crate::states::EphMarkers, console_core_never::Never> {
    Ok(
      match ::core::primitive::bool::then_some((self.raw & 0x04) != 0, ()) {
        ::core::option::Option::Some(()) => crate::states::EphMarkers::Required,
        ::core::option::Option::None => crate::states::EphMarkers::Absent,
      },
    )
  }
}

#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(
  dylint_lib = "explicit048_no_unreal_state",
  expect(
    explicit048_no_unreal_state,
    reason = "the JPEG 2000 coding-style marker defines independent bit flags for these five controls"
  )
)]
pub(crate) struct CodeBlockStyle {
  pub(crate) selective_arithmetic_coding_bypass: bool,
  pub(crate) reset_context_probabilities: bool,
  pub(crate) termination_on_each_pass: bool,
  pub(crate) vertically_causal_context: bool,
  pub(crate) segmentation_symbols: bool,
}

impl CodeBlockStyle {
  fn from_u8(value: u8) -> core::result::Result<Self, console_core_never::Never> {
    Ok(Self {
      selective_arithmetic_coding_bypass: (value & 0x01) != 0,
      reset_context_probabilities: (value & 0x02) != 0,
      termination_on_each_pass: (value & 0x04) != 0,
      vertically_causal_context: (value & 0x08) != 0,

      segmentation_symbols: (value & 0x20) != 0,
    })
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuantizationStyle {
  NoQuantization,
  ScalarDerived,
  ScalarExpounded,
}

impl QuantizationStyle {
  fn from_u8(value: u8) -> Result<Self> {
    match value & 0x1F {
      0 => Ok(Self::NoQuantization),
      1 => Ok(Self::ScalarDerived),
      2 => Ok(Self::ScalarExpounded),
      _ => Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::ValidationError,
      >>::from(ValidationError::InvalidQuantizationStyle)),
    }
  }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct StepSize {
  pub(crate) mantissa: u16,
  pub(crate) exponent: u16,
}

#[derive(Clone, Debug)]
pub(crate) struct QuantizationInfo {
  pub(crate) quantization_style: QuantizationStyle,
  pub(crate) guard_bits: u8,
  pub(crate) step_sizes: Vec<StepSize>,
}

#[derive(Debug, Clone)]
pub(crate) struct CodingStyleDefault {
  pub(crate) progression_order: ProgressionOrder,
  pub(crate) num_layers: u8,
  pub(crate) mct: bool,

  pub(crate) component_parameters: CodingStyleComponent,
}

#[derive(Clone, Debug)]
pub(crate) struct CodingStyleComponent {
  pub(crate) flags: CodingStyleFlags,
  pub(crate) parameters: CodingStyleParameters,
}

#[derive(Clone, Debug)]
pub(crate) struct CodingStyleParameters {
  pub(crate) num_decomposition_levels: u8,
  pub(crate) num_resolution_levels: u8,
  pub(crate) code_block_width: u8,
  pub(crate) code_block_height: u8,
  pub(crate) code_block_style: CodeBlockStyle,
  pub(crate) transformation: WaveletTransform,
  pub(crate) precinct_exponents: Vec<(u8, u8)>,
}

#[derive(Debug)]
pub(crate) struct SizeData {
  pub(crate) reference_grid_width: u32,

  pub(crate) reference_grid_height: u32,

  pub(crate) image_area_x_offset: u32,

  pub(crate) image_area_y_offset: u32,

  pub(crate) tile_width: u32,

  pub(crate) tile_height: u32,

  pub(crate) tile_x_offset: u32,

  pub(crate) tile_y_offset: u32,

  pub(crate) component_sizes: Vec<ComponentSizeInfo>,

  pub(crate) x_shrink_factor: u32,

  pub(crate) y_shrink_factor: u32,

  pub(crate) x_resolution_shrink_factor: u32,

  pub(crate) y_resolution_shrink_factor: u32,
}

impl SizeData {
  pub(crate) fn tile_x_coord(
    &self,
    idx: u32,
  ) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(num_x_tiles) = self.num_x_tiles();

    Ok(idx.wrapping_rem(num_x_tiles.max(1)))
  }

  pub(crate) fn tile_y_coord(
    &self,
    idx: u32,
  ) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(num_x_tiles) = self.num_x_tiles();

    Ok(idx.wrapping_div(num_x_tiles.max(1)))
  }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ComponentSizeInfo {
  pub(crate) precision: u8,
  pub(crate) horizontal_resolution: u8,
  pub(crate) vertical_resolution: u8,
}

impl SizeData {
  pub(crate) fn num_x_tiles(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(u32::wrapping_sub(self.reference_grid_width, self.tile_x_offset).div_ceil(self.tile_width))
  }

  pub(crate) fn num_y_tiles(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(
      u32::wrapping_sub(self.reference_grid_height, self.tile_y_offset)
        .div_ceil(self.tile_height),
    )
  }

  pub(crate) fn num_tiles(&self) -> core::result::Result<u32, console_core_never::Never> {
    let Ok(num_x_tiles) = self.num_x_tiles();
    let Ok(num_y_tiles) = self.num_y_tiles();

    Ok(u32::wrapping_mul(num_x_tiles, num_y_tiles))
  }

  pub(crate) fn image_width(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(
      u32::wrapping_sub(self.reference_grid_width, self.image_area_x_offset).div_ceil(
        u32::wrapping_mul(self.x_shrink_factor, self.x_resolution_shrink_factor),
      ),
    )
  }

  pub(crate) fn image_height(&self) -> core::result::Result<u32, console_core_never::Never> {
    Ok(
      u32::wrapping_sub(self.reference_grid_height, self.image_area_y_offset).div_ceil(
        u32::wrapping_mul(self.y_shrink_factor, self.y_resolution_shrink_factor),
      ),
    )
  }
}

fn size_marker(reader: &mut BitReader<'_>) -> Result<SizeData> {
  let size_data = (match size_marker_inner(reader) {
    Ok(value) => value,
  })
  .ok_or(MarkerError::ParseFailure("SIZ"))?;

  match ::core::primitive::bool::then_some(
    size_data.tile_width == 0
      || size_data.tile_height == 0
      || size_data.reference_grid_width == 0
      || size_data.reference_grid_height == 0,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::ValidationError,
      >>::from(ValidationError::InvalidDimensions));
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(
    size_data.tile_x_offset >= size_data.reference_grid_width
      || size_data.tile_y_offset >= size_data.reference_grid_height,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::ValidationError,
      >>::from(ValidationError::InvalidDimensions));
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(
    size_data.tile_x_offset > size_data.image_area_x_offset
      || size_data.tile_y_offset > size_data.image_area_y_offset,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::TileError,
      >>::from(crate::TileError::InvalidOffsets));
    }
    ::core::option::Option::None => {}
  }

  let checked_value = size_data
    .tile_x_offset
    .checked_add(size_data.tile_width)
    .ok_or(crate::TileError::InvalidOffsets)?;

  match ::core::primitive::bool::then_some(
    checked_value <= size_data.image_area_x_offset || {
      let checked_value_2 = size_data
        .tile_y_offset
        .checked_add(size_data.tile_height)
        .ok_or(crate::TileError::InvalidOffsets)?;
      checked_value_2
    } <= size_data.image_area_y_offset,
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::TileError,
      >>::from(crate::TileError::InvalidOffsets));
    }
    ::core::option::Option::None => {}
  }

  for comp in &size_data.component_sizes {
    match ::core::primitive::bool::then_some(
      comp.precision == 0 || comp.vertical_resolution == 0 || comp.horizontal_resolution == 0,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Err(<crate::error::DecodeError as ::core::convert::From<
          crate::error::ValidationError,
        >>::from(ValidationError::InvalidComponentMetadata));
      }
      ::core::option::Option::None => {}
    }
  }

  let Ok(width) = size_data.image_width();
  let Ok(height) = size_data.image_height();

  match (u64::from(width) > crate::MAX_IMAGE_SIDE || u64::from(height) > crate::MAX_IMAGE_SIDE)
    .then_some(())
  {
    ::core::option::Option::Some(()) => {
      return Err(<crate::error::DecodeError as ::core::convert::From<
        crate::error::ValidationError,
      >>::from(ValidationError::ImageTooLarge));
    }
    ::core::option::Option::None => {}
  }

  Ok(size_data)
}

fn size_marker_inner(
  reader: &mut BitReader<'_>,
) -> core::result::Result<Option<SizeData>, console_core_never::Never> {
  let _ = match match reader.read_u16() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  let _ = match match reader.read_u16() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  let xsiz = match match reader.read_u32() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };
  let ysiz = match match reader.read_u32() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };
  let x_osiz = match match reader.read_u32() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };
  let y_osiz = match match reader.read_u32() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };
  let xt_siz = match match reader.read_u32() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };
  let yt_siz = match match reader.read_u32() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };
  let xto_siz = match match reader.read_u32() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };
  let yto_siz = match match reader.read_u32() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };
  let csiz = match match reader.read_u16() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  match ::core::primitive::bool::then_some(x_osiz >= xsiz || y_osiz >= ysiz, ()) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  match ::core::primitive::bool::then_some(csiz == 0, ()) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let Ok(parsed) = read_component_sizes(reader, csiz);
  let components = match parsed {
    Some(components) => components,
    None => return Ok(None),
  };

  let mut x_shrink_factor = 1;
  let mut y_shrink_factor = 1;

  let (first, rest) = match components.as_slice() {
    [first, rest @ ..] => (first, rest),
    [] => return Ok(None),
  };
  let hr = first.horizontal_resolution;
  let vr = first.vertical_resolution;
  let same_resolution = rest.iter().all(|component| {
    component.horizontal_resolution == hr && component.vertical_resolution == vr
  });

  match ::core::primitive::bool::then_some(same_resolution, ()) {
    ::core::option::Option::Some(()) => {
      x_shrink_factor = match console_core_number_conversion::wrapped::<u8, u32>(hr) {
        Ok(converted) => converted,
      };
      y_shrink_factor = match console_core_number_conversion::wrapped::<u8, u32>(vr) {
        Ok(converted) => converted,
      };
    }
    ::core::option::Option::None => {}
  }

  Ok(Some(SizeData {
    reference_grid_width: xsiz,
    reference_grid_height: ysiz,
    image_area_x_offset: x_osiz,
    image_area_y_offset: y_osiz,
    tile_width: xt_siz,
    tile_height: yt_siz,
    tile_x_offset: xto_siz,
    tile_y_offset: yto_siz,
    component_sizes: components,
    x_shrink_factor,
    y_shrink_factor,
    x_resolution_shrink_factor: 1,
    y_resolution_shrink_factor: 1,
  }))
}

fn read_component_sizes(
  reader: &mut BitReader<'_>,
  count: u16,
) -> core::result::Result<Option<Vec<ComponentSizeInfo>>, console_core_never::Never> {
  let mut components = Vec::new();

  for _ in 0..count {
    let Ok(byte) = reader.read_byte();
    let ssiz = match byte {
      Some(value) => value,
      None => return Ok(None),
    };
    let Ok(reader_byte) = reader.read_byte();
    let x_rsiz = match reader_byte {
      Some(value) => value,
      None => return Ok(None),
    };
    let y_rsiz = match match reader.read_byte() {
      Ok(value) => value,
    } {
      Some(value) => value,
      None => return Ok(None),
    };

    let precision = u8::wrapping_add(ssiz & 0x7F, 1);

    let _is_signed = (ssiz & 0x80) != 0;

    match ::core::primitive::bool::then_some(
      (match console_core_number_conversion::wrapped::<u8, u32>(precision) {
        Ok(converted) => converted,
      }) > BITPLANE_BIT_SIZE,
      (),
    ) {
      ::core::option::Option::Some(()) => {
        return Ok(None);
      }
      ::core::option::Option::None => {}
    }

    components.push(ComponentSizeInfo {
      precision,
      horizontal_resolution: x_rsiz,
      vertical_resolution: y_rsiz,
    });
  }

  Ok(Some(components))
}

fn coding_style_parameters(
  reader: &mut BitReader<'_>,
  coding_style: &CodingStyleFlags,
) -> core::result::Result<Option<CodingStyleParameters>, console_core_never::Never> {
  let Ok(byte) = reader.read_byte();
  let num_decomposition_levels = match byte {
    Some(value) => value,
    None => return Ok(None),
  };

  match ::core::primitive::bool::then_some(num_decomposition_levels > MAX_RESOLUTION_COUNT, ()) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let num_resolution_levels = match num_decomposition_levels.checked_add(1) {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(reader_byte) = reader.read_byte();
  let code_block_width = match (match reader_byte {
    Some(value) => value,
    None => return Ok(None),
  })
  .checked_add(2)
  {
    Some(value) => value,
    None => return Ok(None),
  };
  let code_block_height = match (match match reader.read_byte() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  })
  .checked_add(2)
  {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(code_block_style) = CodeBlockStyle::from_u8(
    match match reader.read_byte() {
      Ok(value) => value,
    } {
      Some(value) => value,
      None => return Ok(None),
    },
  );
  let transformation = match WaveletTransform::from_u8(
    match match reader.read_byte() {
      Ok(value) => value,
    } {
      Some(value) => value,
      None => return Ok(None),
    },
  ) {
    Ok(value) => value,
    Err(error) => {
      let Ok(()) = crate::log::debug(format_args!("invalid JPEG 2000 marker field: {error}"));
      return Ok(None);
    }
  };

  let mut precinct_exponents = Vec::new();

  let Ok(has_precincts) = (coding_style.has_precincts())
    .map(|state| matches!(state, crate::states::Precincts::Explicit));

  match ::core::primitive::bool::then_some(has_precincts, ()) {
    ::core::option::Option::Some(()) => {
      for _ in 0..num_resolution_levels {
        let precinct_size = match match reader.read_byte() {
          Ok(value) => value,
        } {
          Some(value) => value,
          None => return Ok(None),
        };
        let width_exp = precinct_size & 0xF;
        let height_exp = u8::wrapping_shr(precinct_size, 4u32);

        match ::core::primitive::bool::then_some(
          width_exp > MAX_PRECINCT_EXPONENT || height_exp > MAX_PRECINCT_EXPONENT,
          (),
        ) {
          ::core::option::Option::Some(()) => {
            return Ok(None);
          }
          ::core::option::Option::None => {}
        }

        precinct_exponents.push((width_exp, height_exp));
      }
    }
    ::core::option::Option::None => {
      for _ in 0..num_resolution_levels {
        precinct_exponents.push((15, 15));
      }
    }
  }

  Ok(Some(CodingStyleParameters {
    num_decomposition_levels,
    num_resolution_levels,
    code_block_width,
    code_block_height,
    code_block_style,
    transformation,
    precinct_exponents,
  }))
}

fn com_marker(
  reader: &mut BitReader<'_>,
) -> core::result::Result<Option<()>, console_core_never::Never> {
  Ok(match skip_marker_segment(reader) {
    Ok(value) => value,
  })
}

fn tlm_marker(
  reader: &mut BitReader<'_>,
) -> core::result::Result<Option<()>, console_core_never::Never> {
  Ok(match skip_marker_segment(reader) {
    Ok(value) => value,
  })
}

fn ppm_marker<'a>(
  reader: &mut BitReader<'a>,
) -> core::result::Result<Option<PpmMarkerData<'a>>, console_core_never::Never> {
  let segment_len = match (match match reader.read_u16() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  })
  .checked_sub(2)
  {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(bytes) = reader.read_bytes(u64::from(segment_len));
  let ppm_data = match bytes {
    Some(value) => value,
    None => return Ok(None),
  };
  let mut packets = vec![];

  let Ok(mut packet_reader) = BitReader::new(ppm_data);
  let Ok(byte) = packet_reader.read_byte();
  let sequence_idx = match byte {
    Some(value) => value,
    None => return Ok(None),
  };

  for _ in 0..u32::from(segment_len.saturating_sub(1)).div_ceil(2) {
    let Ok(status) = packet_reader.at_end();

    match status {
      crate::ReaderEnd::Reached => break,
      crate::ReaderEnd::Pending => {}
    }

    let packet_len = {
      let outcome_packet_len = packet_reader.read_u16();

      match outcome_packet_len {
        Ok(Some(matched_packet_len)) => matched_packet_len,
        ::core::result::Result::Ok(::core::option::Option::None)
        | ::core::result::Result::Err(_) => return Ok(None),
      }
    };

    let Ok(reader_bytes) = packet_reader.read_bytes(u64::from(packet_len));
    let data = match reader_bytes {
      Some(value) => value,
      None => return Ok(None),
    };

    packets.push(PpmPacket { data });
  }

  Ok(Some(PpmMarkerData {
    sequence_idx,
    packets,
  }))
}

fn rgn_marker(
  reader: &mut BitReader<'_>,
) -> core::result::Result<Option<()>, console_core_never::Never> {
  Ok(match skip_marker_segment(reader) {
    Ok(value) => value,
  })
}

pub(crate) fn skip_marker_segment(
  reader: &mut BitReader<'_>,
) -> core::result::Result<Option<()>, console_core_never::Never> {
  let length = match (match match reader.read_u16() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  })
  .checked_sub(2)
  {
    Some(value) => value,
    None => return Ok(None),
  };
  let Ok(skip_bytes) = reader.skip_bytes(u64::from(length));
  (match skip_bytes {
    Some(value) => value,
    None => return Ok(None),
  });

  Ok(Some(()))
}

pub(crate) fn cod_marker(
  reader: &mut BitReader<'_>,
) -> core::result::Result<Option<CodingStyleDefault>, console_core_never::Never> {
  let _ = match match reader.read_u16() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(byte) = reader.read_byte();
  let Ok(coding_style_flags) = CodingStyleFlags::from_u8(match byte {
    Some(value) => value,
    None => return Ok(None),
  });
  let Ok(reader_byte) = reader.read_byte();
  let progression_order = match ProgressionOrder::from_u8(match reader_byte {
    Some(value) => value,
    None => return Ok(None),
  }) {
    Ok(value) => value,
    Err(error) => {
      let Ok(()) = crate::log::debug(format_args!("invalid JPEG 2000 marker field: {error}"));
      return Ok(None);
    }
  };

  let num_layers = match match reader.read_u16() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  match ::core::primitive::bool::then_some(
    num_layers == 0
      || num_layers
        > (match console_core_number_conversion::wrapped::<u8, u16>(MAX_LAYER_COUNT) {
          Ok(converted) => converted,
        }),
    (),
  ) {
    ::core::option::Option::Some(()) => {
      return Ok(None);
    }
    ::core::option::Option::None => {}
  }

  let mct = (match match reader.read_byte() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  }) == 1;

  let coding_style_parameters = match match coding_style_parameters(reader, &coding_style_flags) {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  Ok(Some(CodingStyleDefault {
    progression_order,
    num_layers: (match console_core_number_conversion::wrapped::<u16, u8>(num_layers) {
      Ok(converted) => converted,
    }),
    mct,
    component_parameters: CodingStyleComponent {
      flags: coding_style_flags,
      parameters: coding_style_parameters,
    },
  }))
}

pub(crate) fn coc_marker(
  reader: &mut BitReader<'_>,
  csiz: u16,
) -> core::result::Result<Option<(u16, CodingStyleComponent)>, console_core_never::Never> {
  let _ = match match reader.read_u16() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  let component_index = match ::core::primitive::bool::then_some(csiz < 257, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(byte) = reader.read_byte();

      match console_core_number_conversion::wrapped::<u8, u16>(match byte {
        Some(value) => value,
        None => return Ok(None),
      }) {
        Ok(converted) => converted,
      }
    }
    ::core::option::Option::None => {
      match match reader.read_u16() {
        Ok(value) => value,
      } {
        Some(value) => value,
        None => return Ok(None),
      }
    }
  };
  let Ok(byte) = reader.read_byte();
  let Ok(coding_style) = CodingStyleFlags::from_u8(match byte {
    Some(value) => value,
    None => return Ok(None),
  });

  let parameters = match match coding_style_parameters(reader, &coding_style) {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  let coc = CodingStyleComponent {
    flags: coding_style,
    parameters,
  };

  Ok(Some((component_index, coc)))
}

pub(crate) fn qcd_marker(
  reader: &mut BitReader<'_>,
) -> core::result::Result<Option<QuantizationInfo>, console_core_never::Never> {
  let length = match match reader.read_u16() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  let Ok(byte) = reader.read_byte();
  let sqcd_val = match byte {
    Some(value) => value,
    None => return Ok(None),
  };
  let quantization_style = match QuantizationStyle::from_u8(sqcd_val & 0x1F) {
    Ok(value) => value,
    Err(error) => {
      let Ok(()) = crate::log::debug(format_args!("invalid JPEG 2000 marker field: {error}"));
      return Ok(None);
    }
  };
  let guard_bits = u8::wrapping_shr(sqcd_val, 5u32) & 0x07;

  let remaining_bytes = match length.checked_sub(3) {
    Some(value) => value,
    None => return Ok(None),
  };

  let mut parameters =
    match match quantization_parameters(reader, quantization_style, remaining_bytes) {
      Ok(value) => value,
    } {
      Some(value) => value,
      None => return Ok(None),
    };
  parameters.guard_bits = guard_bits;

  Ok(Some(parameters))
}

pub(crate) fn qcc_marker(
  reader: &mut BitReader<'_>,
  csiz: u16,
) -> core::result::Result<Option<(u16, QuantizationInfo)>, console_core_never::Never> {
  let length = match match reader.read_u16() {
    Ok(value) => value,
  } {
    Some(value) => value,
    None => return Ok(None),
  };

  let component_index = match ::core::primitive::bool::then_some(csiz < 257, ()) {
    ::core::option::Option::Some(()) => {
      let Ok(byte) = reader.read_byte();

      match console_core_number_conversion::wrapped::<u8, u16>(match byte {
        Some(value) => value,
        None => return Ok(None),
      }) {
        Ok(converted) => converted,
      }
    }
    ::core::option::Option::None => {
      match match reader.read_u16() {
        Ok(value) => value,
      } {
        Some(value) => value,
        None => return Ok(None),
      }
    }
  };

  let Ok(byte) = reader.read_byte();
  let sqcc_val = match byte {
    Some(value) => value,
    None => return Ok(None),
  };
  let quantization_style = match QuantizationStyle::from_u8(sqcc_val & 0x1F) {
    Ok(value) => value,
    Err(error) => {
      let Ok(()) = crate::log::debug(format_args!("invalid JPEG 2000 marker field: {error}"));
      return Ok(None);
    }
  };
  let guard_bits = u8::wrapping_shr(sqcc_val, 5u32) & 0x07;

  let component_index_size = match ::core::primitive::bool::then_some(csiz < 257, ()) {
    ::core::option::Option::Some(()) => 1,
    ::core::option::Option::None => 2,
  };
  let remaining_bytes = match length.checked_sub(3u16.wrapping_add(component_index_size)) {
    Some(value) => value,
    None => return Ok(None),
  };

  let mut parameters =
    match match quantization_parameters(reader, quantization_style, remaining_bytes) {
      Ok(value) => value,
    } {
      Some(value) => value,
      None => return Ok(None),
    };
  parameters.guard_bits = guard_bits;

  Ok(Some((component_index, parameters)))
}

fn quantization_parameters(
  reader: &mut BitReader<'_>,
  quantization_style: QuantizationStyle,
  remaining_bytes: u16,
) -> core::result::Result<Option<QuantizationInfo>, console_core_never::Never> {
  let mut step_sizes = Vec::new();

  let irreversible = |val: u16| {
    let exponent = u16::wrapping_shr(val, 11u32);
    let mantissa = val & u16::wrapping_sub(u16::wrapping_shl(1, 11u32), 1);

    StepSize { exponent, mantissa }
  };

  match quantization_style {
    QuantizationStyle::NoQuantization => {
      for _ in 0..remaining_bytes {
        let Ok(byte) = reader.read_byte();
        let Ok(value) = console_core_number_conversion::wrapped::<u8, u16>(match byte {
          Some(value) => value,
          None => return Ok(None),
        });
        step_sizes.push(StepSize {
          mantissa: 0,
          exponent: u16::wrapping_shr(value, 3u32),
        });
      }
    }
    QuantizationStyle::ScalarDerived => {
      let value = match match reader.read_u16() {
        Ok(value) => value,
      } {
        Some(value) => value,
        None => return Ok(None),
      };
      step_sizes.push(irreversible(value));
    }
    QuantizationStyle::ScalarExpounded => {
      let num_bands = remaining_bytes.wrapping_div(2);

      for _ in 0..num_bands {
        let value = match match reader.read_u16() {
          Ok(value) => value,
        } {
          Some(value) => value,
          None => return Ok(None),
        };

        step_sizes.push(irreversible(value));
      }
    }
  }

  Ok(Some(QuantizationInfo {
    quantization_style,
    guard_bits: 0,
    step_sizes,
  }))
}

pub(crate) mod markers {

  pub(crate) const SOC: u8 = 0x4F;

  pub(crate) const SOT: u8 = 0x90;

  pub(crate) const SOD: u8 = 0x93;

  pub(crate) const EOC: u8 = 0xD9;

  pub(crate) const SIZ: u8 = 0x51;

  pub(crate) const COD: u8 = 0x52;

  pub(crate) const COC: u8 = 0x53;

  pub(crate) const RGN: u8 = 0x5E;

  pub(crate) const QCD: u8 = 0x5C;

  pub(crate) const QCC: u8 = 0x5D;

  pub(crate) const TLM: u8 = 0x55;

  pub(crate) const PLT: u8 = 0x58;

  pub(crate) const PPM: u8 = 0x60;

  pub(crate) const PPT: u8 = 0x61;

  pub(crate) const SOP: u8 = 0x91;

  pub(crate) const EPH: u8 = 0x92;

  pub(crate) const CRG: u8 = 0x63;

  pub(crate) const COM: u8 = 0x64;
}
