use crate::pipeline;

#[derive(Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Debug)]
pub enum BlendMode {
  Clear,

  Source,

  Destination,

  #[default]
  SourceOver,

  DestinationOver,

  SourceIn,

  DestinationIn,

  SourceOut,

  DestinationOut,

  SourceAtop,

  DestinationAtop,

  Xor,

  Plus,

  Modulate,

  Screen,

  Overlay,

  Darken,

  Lighten,

  ColorDodge,

  ColorBurn,

  HardLight,

  SoftLight,

  Difference,

  Exclusion,

  Multiply,

  Hue,

  Saturation,

  Color,

  Luminosity,
}

impl BlendMode {
  pub(crate) fn should_pre_scale_coverage(
    self,
  ) -> core::result::Result<crate::ShouldPreScaleCoverageAnswer, console_core_never::Never> {
    Ok(match self {
      BlendMode::Destination
      | BlendMode::DestinationOver
      | BlendMode::Plus
      | BlendMode::DestinationOut
      | BlendMode::SourceAtop
      | BlendMode::SourceOver
      | BlendMode::Xor => crate::ShouldPreScaleCoverageAnswer::Yes,
      BlendMode::Clear
      | BlendMode::Source
      | BlendMode::SourceIn
      | BlendMode::DestinationIn
      | BlendMode::SourceOut
      | BlendMode::DestinationAtop
      | BlendMode::Modulate
      | BlendMode::Screen
      | BlendMode::Overlay
      | BlendMode::Darken
      | BlendMode::Lighten
      | BlendMode::ColorDodge
      | BlendMode::ColorBurn
      | BlendMode::HardLight
      | BlendMode::SoftLight
      | BlendMode::Difference
      | BlendMode::Exclusion
      | BlendMode::Multiply
      | BlendMode::Hue
      | BlendMode::Saturation
      | BlendMode::Color
      | BlendMode::Luminosity => crate::ShouldPreScaleCoverageAnswer::No,
    })
  }

  pub(crate) fn to_stage(
    self,
  ) -> core::result::Result<Option<pipeline::Stage>, console_core_never::Never> {
    Ok(match self {
      BlendMode::Clear => Some(pipeline::Stage::Clear),
      BlendMode::Source => None,
      BlendMode::Destination => Some(pipeline::Stage::MoveDestinationToSource),
      BlendMode::SourceOver => Some(pipeline::Stage::SourceOver),
      BlendMode::DestinationOver => Some(pipeline::Stage::DestinationOver),
      BlendMode::SourceIn => Some(pipeline::Stage::SourceIn),
      BlendMode::DestinationIn => Some(pipeline::Stage::DestinationIn),
      BlendMode::SourceOut => Some(pipeline::Stage::SourceOut),
      BlendMode::DestinationOut => Some(pipeline::Stage::DestinationOut),
      BlendMode::SourceAtop => Some(pipeline::Stage::SourceAtop),
      BlendMode::DestinationAtop => Some(pipeline::Stage::DestinationAtop),
      BlendMode::Xor => Some(pipeline::Stage::Xor),
      BlendMode::Plus => Some(pipeline::Stage::Plus),
      BlendMode::Modulate => Some(pipeline::Stage::Modulate),
      BlendMode::Screen => Some(pipeline::Stage::Screen),
      BlendMode::Overlay => Some(pipeline::Stage::Overlay),
      BlendMode::Darken => Some(pipeline::Stage::Darken),
      BlendMode::Lighten => Some(pipeline::Stage::Lighten),
      BlendMode::ColorDodge => Some(pipeline::Stage::ColorDodge),
      BlendMode::ColorBurn => Some(pipeline::Stage::ColorBurn),
      BlendMode::HardLight => Some(pipeline::Stage::HardLight),
      BlendMode::SoftLight => Some(pipeline::Stage::SoftLight),
      BlendMode::Difference => Some(pipeline::Stage::Difference),
      BlendMode::Exclusion => Some(pipeline::Stage::Exclusion),
      BlendMode::Multiply => Some(pipeline::Stage::Multiply),
      BlendMode::Hue => Some(pipeline::Stage::Hue),
      BlendMode::Saturation => Some(pipeline::Stage::Saturation),
      BlendMode::Color => Some(pipeline::Stage::Color),
      BlendMode::Luminosity => Some(pipeline::Stage::Luminosity),
    })
  }
}
