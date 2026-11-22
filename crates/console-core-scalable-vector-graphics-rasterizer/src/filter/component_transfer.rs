//! Copyright 2020 the Resvg Authors
//! SPDX-License-Identifier: Apache-2.0 OR MIT

use super::{ChannelValue, ImageRefMut};
use console_core_never::Never;
use console_core_number_conversion::{fixed_index, index, nearest_f32_from_u64, toward_zero_u64};
use console_core_scalable_vector_graphics::filter::{ComponentTransfer, TransferFunction};

#[doc = "Applies a transfer function to each unpremultiplied color channel."]
pub(super) fn apply(transfer: &ComponentTransfer, source: ImageRefMut<'_>) -> Result<(), Never> {
  let Ok(red) = transfer.func_r();
  let Ok(green) = transfer.func_g();
  let Ok(blue) = transfer.func_b();
  let Ok(alpha) = transfer.func_a();

  for [r, g, b, a] in source.data {
    let Ok(r_value) = channel(red, *r);
    let Ok(g_value) = channel(green, *g);
    let Ok(b_value) = channel(blue, *b);
    let Ok(a_value) = channel(alpha, *a);
    *r = r_value;
    *g = g_value;
    *b = b_value;
    *a = a_value;
  }

  Ok(())
}

fn channel(function: &TransferFunction, original: u8) -> Result<u8, Never> {
  let Ok(normalized) = ChannelValue::from_byte(original);
  let value = match function {
    TransferFunction::Identity => return Ok(original),
    TransferFunction::Table(values) => {
      let Ok(converted) = interpolated(values, normalized);

      match converted {
        Some(value) => value,
        None => return Ok(original),
      }
    }
    TransferFunction::Discrete(values) => {
      let Ok(converted) = discrete(values, normalized);

      match converted {
        Some(value) => value,
        None => return Ok(original),
      }
    }
    TransferFunction::Linear { slope, intercept } => {
      ChannelValue(slope * normalized.0 + intercept)
    }
    TransferFunction::Gamma {
      amplitude,
      exponent,
      offset,
    } => ChannelValue(amplitude * normalized.0.powf(*exponent) + offset),
  };

  value.quantized()
}

fn interpolated(
  values: &[f32],
  ChannelValue(channel): ChannelValue,
) -> Result<Option<ChannelValue>, Never> {
  let (last, prefix) = match values.split_last() {
    Some(parts) => parts,
    None => return Ok(None),
  };
  let Ok(final_step) = fixed_index(prefix.len());
  let Ok(segments) = nearest_f32_from_u64(final_step);
  let Ok(ordinal) = toward_zero_u64(f64::from((channel * segments).floor()));

  match ::core::primitive::bool::then_some(ordinal.min(final_step) == final_step, ()) {
    ::core::option::Option::Some(()) => return Ok(Some(ChannelValue(*last))),
    ::core::option::Option::None => {}
  }

  let Ok(location) = index(ordinal);
  let Ok(step) = nearest_f32_from_u64(ordinal);
  Ok(match values.get(location..) {
    Some([left, right, ..]) => Some(ChannelValue(
      left + (channel - step / segments) * segments * (right - left),
    )),
    Some([] | [_]) | None => None,
  })
}

fn discrete(
  values: &[f32],
  ChannelValue(channel): ChannelValue,
) -> Result<Option<ChannelValue>, Never> {
  let Ok(length) = fixed_index(values.len());
  let Ok(count) = nearest_f32_from_u64(length);
  let Ok(ordinal) = toward_zero_u64(f64::from((channel * count).floor()));
  let Ok(location) = index(ordinal.min(length.saturating_sub(1)));

  Ok(values.get(location).copied().map(ChannelValue))
}
