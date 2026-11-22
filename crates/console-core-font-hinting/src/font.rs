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
    let maxp = font.maxp()?;
    let max_function_defs = match {
      let answer = maxp.max_function_defs()?;
      answer
    } {
      Some(count) => count,
      None => 0,
    };
    let max_instruction_defs = match {
      let answer = maxp.max_instruction_defs()?;
      answer
    } {
      Some(count) => count,
      None => 0,
    };
    let max_twilight_points = match {
      let answer = maxp.max_twilight_points()?;
      answer
    } {
      Some(count) => count,
      None => 0,
    }
    .saturating_add(4);
    let max_stack_elements = match {
      let answer = maxp.max_stack_elements()?;
      answer
    } {
      Some(count) => count,
      None => 0,
    }
    .saturating_add(32);
    let max_storage = match {
      let answer = maxp.max_storage()?;
      answer
    } {
      Some(count) => count,
      None => 0,
    };
    let max_instructions = match {
      let answer = maxp.max_size_of_instructions()?;
      answer
    } {
      Some(count) => count,
      None => 0,
    };
    let fpgm = match {
      let answer = font.data_for_tag({
        let Ok(value) = Tag::new(b"fpgm");
        value
      })?;
      answer
    } {
      Some(data) => {
        let Ok(bytes) = data.as_bytes();
        bytes
      }
      None => &[],
    };
    let prep = match {
      let answer = font.data_for_tag({
        let Ok(value) = Tag::new(b"prep");
        value
      })?;
      answer
    } {
      Some(data) => {
        let Ok(bytes) = data.as_bytes();
        bytes
      }
      None => &[],
    };
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
    let instructions = match ::core::primitive::bool::then_some(max_instructions == 0 && fpgm.is_empty() && prep.is_empty(), ()) {
      ::core::option::Option::Some(()) => crate::InstructionPresence::Absent,
      ::core::option::Option::None => crate::InstructionPresence::Present,
    };
    Ok(Self {
      font: font.clone(),
      fpgm,
      prep,
      cvt_len,
      max_function_defs,
      max_instruction_defs,
      max_twilight_points,
      max_stack_elements,
      max_storage,
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
