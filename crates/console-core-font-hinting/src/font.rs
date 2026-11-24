//! Borrowed font programs and interpreter resource limits.
use console_core_never::Never;
use console_core_number_conversion::fitted;
use read_fonts::{FontRef, ReadError, TableProvider, types::Tag};

#[derive(Clone)]
pub struct FontProgram<'a> {
  pub(crate) font: FontRef<'a>,
  pub(crate) fpgm: &'a [u8],
  pub(crate) prep: &'a [u8],
  pub(crate) cvt_len: u32,
  pub(crate) max_function_defs: u16,
  pub(crate) max_instruction_defs: u16,
  pub(crate) max_twilight_points: u16,
  pub(crate) max_stack_elements: u16,
  pub(crate) max_storage: u16,
  pub(crate) axis_count: u16,
  instructions: crate::InstructionPresence,
}

impl<'a> FontProgram<'a> {
  pub fn new(font: &FontRef<'a>) -> Result<Self, ReadError> {
    let limits = ProgramLimits::read(font)?;
    let Ok(fpgm_tag) = Tag::new(b"fpgm");
    let Ok(prep_tag) = Tag::new(b"prep");
    let fpgm = program_for(font, fpgm_tag)?;
    let prep = program_for(font, prep_tag)?;
    let cvt_len = match font.cvt() {
      Ok(cvt) => {
        let Ok(count) = fitted::<_, u32>(cvt.len());
        count
      }
      Err(ReadError::TableIsMissing(_tag)) => 0,
      Err(fault) => return Err(fault),
    };
    let axis_count = match font.gvar() {
      Ok(gvar) => {
        let Ok(gvar_axis_count) = gvar.axis_count();

        gvar_axis_count
      }
      Err(ReadError::TableIsMissing(_tag)) => 0,
      Err(fault) => return Err(fault),
    };
    let instructions = match ::core::primitive::bool::then_some(
      limits.max_instructions == 0 && fpgm.is_empty() && prep.is_empty(),
      (),
    ) {
      ::core::option::Option::Some(()) => crate::InstructionPresence::Absent,
      ::core::option::Option::None => crate::InstructionPresence::Present,
    };
    Ok(Self {
      font: font.clone(),
      fpgm,
      prep,
      cvt_len,
      max_function_defs: limits.max_function_defs,
      max_instruction_defs: limits.max_instruction_defs,
      max_twilight_points: limits.max_twilight_points,
      max_stack_elements: limits.max_stack_elements,
      max_storage: limits.max_storage,
      instructions,
      axis_count,
    })
  }

  pub fn instruction_presence(&self) -> Result<crate::InstructionPresence, Never> {
    Ok(self.instructions)
  }

  pub fn max_stack_elements(&self) -> Result<u32, Never> {
    Ok(u32::from(self.max_stack_elements))
  }

  pub fn cvt_count(&self) -> Result<u32, Never> {
    Ok(self.cvt_len)
  }

  pub fn storage_count(&self) -> Result<u32, Never> {
    Ok(u32::from(self.max_storage))
  }

  pub fn twilight_count(&self) -> Result<u32, Never> {
    Ok(u32::from(self.max_twilight_points))
  }
}

struct ProgramLimits {
  max_function_defs: u16,
  max_instruction_defs: u16,
  max_twilight_points: u16,
  max_stack_elements: u16,
  max_storage: u16,
  max_instructions: u16,
}

impl ProgramLimits {
  fn read(font: &FontRef<'_>) -> Result<Self, ReadError> {
    let maxp = font.maxp()?;
    let functions = maxp.max_function_defs()?;
    let definitions = maxp.max_instruction_defs()?;
    let twilight = maxp.max_twilight_points()?;
    let stack = maxp.max_stack_elements()?;
    let storage = maxp.max_storage()?;
    let instructions = maxp.max_size_of_instructions()?;
    let Ok(max_function_defs) = limit_or_zero(functions);
    let Ok(max_instruction_defs) = limit_or_zero(definitions);
    let Ok(twilight_points) = limit_or_zero(twilight);
    let Ok(stack_elements) = limit_or_zero(stack);
    let Ok(max_storage) = limit_or_zero(storage);
    let Ok(max_instructions) = limit_or_zero(instructions);

    Ok(Self {
      max_function_defs,
      max_instruction_defs,
      max_twilight_points: twilight_points.saturating_add(4),
      max_stack_elements: stack_elements.saturating_add(32),
      max_storage,
      max_instructions,
    })
  }
}

fn limit_or_zero(limit: Option<u16>) -> Result<u16, Never> {
  Ok(match limit {
    Some(count) => count,
    None => 0,
  })
}

fn program_for<'a>(font: &FontRef<'a>, tag: Tag) -> Result<&'a [u8], ReadError> {
  let table = font.data_for_tag(tag)?;

  Ok(match table {
    Some(data) => {
      let Ok(bytes) = data.as_bytes();
      bytes
    }
    None => &[],
  })
}

#[cfg(test)]
mod tests {
  use super::{FontProgram, FontRef, ReadError, Tag};

  #[test]
  fn missing_limits_table_is_reported() -> Result<(), ReadError> {
    let font = FontRef::new(&[0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])?;
    let actual = FontProgram::new(&font).map(|_program| ());
    assert_eq!(
      actual,
      Err(ReadError::TableIsMissing({
        let Ok(value) = Tag::new(b"maxp");
        value
      }))
    );
    Ok(())
  }

  #[test]
  fn malformed_limits_table_is_reported() -> Result<(), ReadError> {
    let font = FontRef::new(b"\x00\x01\x00\x00\x00\x01\x00\x10\x00\x00\x00\x00maxp\x00\x00\x00\x00\x00\x00\x00\x1c\x00\x00\x00\x01\x00")?;
    let actual = FontProgram::new(&font).map(|_program| ());
    assert_eq!(actual, Err(ReadError::OutOfBounds));
    Ok(())
  }
}
