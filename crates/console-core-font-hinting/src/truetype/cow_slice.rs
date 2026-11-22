//! Copy-on-write buffers retain an immutable source until their first write.

use console_core_never::Never;

enum BufferState<'a> {
  Borrowed {
    immutable: &'a [i32],
    mutable: &'a mut [i32],
  },
  Writable(&'a mut [i32]),
}

pub struct CowSlice<'a> {
  state: BufferState<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CowSliceOutOfBounds;

impl<'a> From<&'a mut [i32]> for CowSlice<'a> {
  fn from(values: &'a mut [i32]) -> Self {
    Self {
      state: BufferState::Writable(values),
    }
  }
}

impl<'a> CowSlice<'a> {
  pub fn new(
    immutable: &'a [i32],
    mutable: &'a mut [i32],
  ) -> Result<Self, CowSliceSizeMismatchError> {
    match ::core::primitive::bool::then_some(immutable.len() == mutable.len(), ()) {
      ::core::option::Option::Some(()) => Ok(Self {
        state: BufferState::Borrowed { immutable, mutable },
      }),
      ::core::option::Option::None => Err(CowSliceSizeMismatchError(
        {
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(immutable.len());
          value
        },
        {
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(mutable.len());
          value
        },
      )),
    }
  }

  pub fn get(&self, index: u64) -> Result<i32, CowSliceOutOfBounds> {
    let values = match &self.state {
      BufferState::Borrowed { immutable, .. } => *immutable,
      BufferState::Writable(values) => &**values,
    };
    values
      .get({
        let Ok(value) = console_core_number_conversion::index(index);
        value
      })
      .copied()
      .ok_or(CowSliceOutOfBounds)
  }

  pub fn set(&mut self, index: u64, value: i32) -> Result<(), CowSliceOutOfBounds> {
    let previous = core::mem::replace(&mut self.state, BufferState::Writable(&mut []));
    let mutable = match previous {
      BufferState::Borrowed { immutable, mutable } => {
        for (target, source) in mutable.iter_mut().zip(immutable.iter()) {
          *target = *source;
        }

        mutable
      }
      BufferState::Writable(mutable) => mutable,
    };
    let result = match mutable.get_mut({
      let Ok(value_2) = console_core_number_conversion::index(index);
      value_2
    }) {
      Some(target) => {
        *target = value;
        Ok(())
      }
      None => Err(CowSliceOutOfBounds),
    };
    self.state = BufferState::Writable(mutable);
    result
  }

  pub fn len(&self) -> Result<u64, Never> {
    Ok(match &self.state {
      BufferState::Borrowed { immutable, .. } => {
        let Ok(value) = console_core_number_conversion::fitted::<_, u64>(immutable.len());
        value
      }
      BufferState::Writable(mutable) => {
        let Ok(value) = console_core_number_conversion::fitted::<_, u64>(mutable.len());
        value
      }
    })
  }
}

#[derive(Clone, Debug)]
pub struct CowSliceSizeMismatchError(u64, u64);

impl std::fmt::Display for CowSliceSizeMismatchError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "size mismatch for immutable and mutable buffers: data.len() = {}, data_mut.len() = {}",
      self.0, self.1
    )
  }
}

#[cfg(test)]
mod tests {
  use super::{BufferState, CowSlice, CowSliceOutOfBounds, CowSliceSizeMismatchError};

  #[test]
  fn size_mismatch_error() {
    let data_mut = &mut [0, 0];
    let result = CowSlice::new(&[1, 2, 3], data_mut);
    assert!(matches!(result, Err(CowSliceSizeMismatchError(3, 2))));
  }

  #[test]
  fn copy_on_write() -> Result<(), crate::TestFailure> {
    use crate::test_support::Required;
    let data = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
    let mut data_mut = [0; 16];
    let mut slice = CowSlice::new(&data, &mut data_mut).required()?;
    assert!(matches!(&slice.state, BufferState::Borrowed { .. }));

    for (index, expected) in data.iter().enumerate() {
      let actual = slice
        .get({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(index);
          value
        })
        .required()?;
      assert_eq!(actual, *expected);
    }

    for (index, _) in data.iter().enumerate() {
      let current = slice
        .get({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(index);
          value
        })
        .required()?;
      slice
        .set(
          {
            let Ok(value) = console_core_number_conversion::fitted::<_, u64>(index);
            value
          },
          current.wrapping_mul(2),
        )
        .required()?;
    }

    assert!(matches!(&slice.state, BufferState::Writable(_)));

    for (index, expected) in data.iter().enumerate() {
      let actual = slice
        .get({
          let Ok(value) = console_core_number_conversion::fitted::<_, u64>(index);
          value
        })
        .required()?;
      assert_eq!(actual, expected.wrapping_mul(2));
    }

    Ok(())
  }

  #[test]
  fn out_of_bounds() {
    let data_mut = &mut [1, 2];
    let slice = CowSlice::from(data_mut.as_mut_slice());
    assert_eq!(slice.get(0), Ok(1));
    assert_eq!(slice.get(1), Ok(2));
    assert_eq!(slice.get(2), Err(CowSliceOutOfBounds));
  }
}
