use console_core_scalable_vector_graphics_values::{Length, LengthUnit as Unit};

use super::converter;
use super::svgtree::{AId, SvgNode};
use crate::Units;

#[inline(never)]
pub(crate) fn convert_length(
  length: Length,
  node: SvgNode,
  aid: AId,
  object_units: Units,
  state: &converter::State,
) -> core::result::Result<f32, console_core_never::Never> {
  let dpi = state.opt.dpi;
  let ::core::result::Result::Ok(n) =
    ::console_core_number_conversion::nearest_f32_from_f64(length.number);

  Ok(match length.unit {
    Unit::None | Unit::Px => n,
    Unit::Em => {
      n * ({
        let Ok(explicit_value) = resolve_font_size(node, state);
        explicit_value
      })
    }
    Unit::Ex => {
      n * ({
        let Ok(explicit_value) = resolve_font_size(node, state);
        explicit_value
      }) / 2.0
    }
    Unit::Inches => n * dpi,
    Unit::Cm => n * dpi / 2.54,
    Unit::Mm => n * dpi / 25.4,
    Unit::Pt => n * dpi / 72.0,
    Unit::Pc => n * dpi / 6.0,
    Unit::Percent => match object_units {
      Units::ObjectBoundingBox => n / 100.0,
      Units::UserSpaceOnUse => {
        let Ok(base) = viewport_extent(aid, state);
        base.0 * n / 100.0
      }
    },
  })
}

pub(crate) fn convert_user_length(
  length: Length,
  node: SvgNode,
  aid: AId,
  state: &converter::State,
) -> core::result::Result<f32, console_core_never::Never> {
  Ok({
    let Ok(explicit_value) = convert_length(length, node, aid, Units::UserSpaceOnUse, state);
    explicit_value
  })
}

#[inline(never)]
pub(crate) fn convert_list(
  node: SvgNode,
  aid: AId,
  state: &converter::State,
) -> core::result::Result<Option<Vec<f32>>, console_core_never::Never> {
  let Ok(attribute) = node.attribute::<&str>(aid);

  Ok(match attribute {
    Some(text) => {
      let mut num_list = Vec::new();

      for length in
        console_core_scalable_vector_graphics_values::LengthListParser::from(text).flatten()
      {
        num_list.push({
          let Ok(explicit_value) = convert_user_length(length, node, aid, state);
          explicit_value
        });
      }

      Some(num_list)
    }
    None => None,
  })
}

fn viewport_extent(
  attribute: AId,
  state: &converter::State,
) -> Result<crate::UserLength, console_core_never::Never> {
  let horizontal = matches!(
    attribute,
    AId::Cx
      | AId::Dx
      | AId::Fx
      | AId::MarkerWidth
      | AId::RefX
      | AId::Rx
      | AId::Width
      | AId::X
      | AId::X1
      | AId::X2
  );
  let vertical = matches!(
    attribute,
    AId::Cy
      | AId::Dy
      | AId::Fy
      | AId::Height
      | AId::MarkerHeight
      | AId::RefY
      | AId::Ry
      | AId::Y
      | AId::Y1
      | AId::Y2
  );
  let Ok(width) = state.view_box.width();
  let Ok(height) = state.view_box.height();

  Ok(crate::UserLength(match (horizontal, vertical) {
    (true, _) => width,
    (false, true) => height,
    (false, false) => ((width.powi(2) + height.powi(2)) / 2.0).sqrt(),
  }))
}

#[inline(never)]
pub(crate) fn resolve_font_size(
  node: SvgNode,
  state: &converter::State,
) -> core::result::Result<f32, console_core_never::Never> {
  let Ok(ancestors) = node.ancestors();
  let lineage: Vec<_> = ancestors.collect();
  let mut size = crate::UserLength(state.opt.font_size);

  for &ancestor in lineage.iter().rev().skip(1) {
    let Ok(resolved) = font_size_for_node(ancestor, size, state);
    size = resolved;
  }

  Ok(size.0)
}

fn font_size_for_node(
  node: SvgNode,
  inherited: crate::UserLength,
  state: &converter::State,
) -> Result<crate::UserLength, console_core_never::Never> {
  let Ok(length) = node.try_attribute::<Length>(AId::FontSize);
  let size = match length {
    Some(length_2) => {
      let dpi = state.opt.dpi;
      let Ok(number) = console_core_number_conversion::nearest_f32_from_f64(length_2.number);

      match length_2.unit {
        Unit::None | Unit::Px => number,
        Unit::Em => number * inherited.0,
        Unit::Ex => number * inherited.0 / 2.0,
        Unit::Inches => number * dpi,
        Unit::Cm => number * dpi / 2.54,
        Unit::Mm => number * dpi / 25.4,
        Unit::Pt => number * dpi / 72.0,
        Unit::Pc => number * dpi / 6.0,
        Unit::Percent => number * inherited.0 * 0.01,
      }
    }
    None => {
      let Ok(name) = node.attribute(AId::FontSize);

      return match name {
        Some(name_2) => convert_named_font_size(name_2, inherited),
        None => Ok(inherited),
      };
    }
  };

  Ok(crate::UserLength(size))
}

fn convert_named_font_size(
  name: &str,
  parent_font_size: crate::UserLength,
) -> Result<crate::UserLength, console_core_never::Never> {
  let factor = match name {
    "xx-small" => -3,
    "x-small" => -2,
    "small" => -1,
    "medium" => 0,
    "large" => 1,
    "x-large" => 2,
    "xx-large" => 3,
    "smaller" => -1,
    "larger" => 1,
    _ => {
      console_core_logging::warn!("Invalid 'font-size' value: '{}'.", name);
      0
    }
  };

  Ok(crate::UserLength(parent_font_size.0 * 1.2f32.powi(factor)))
}
