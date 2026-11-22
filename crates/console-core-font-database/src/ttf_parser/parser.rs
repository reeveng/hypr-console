//! Big-endian scalar and array views over a font's bytes, with sixty-four-bit byte positions.

use console_core_never::Never;
use console_core_number_conversion::{fitted, indices, indices_from, wrapped};
use core::cmp::Ordering;

pub trait FromData: Sized {
  const SIZE: u32;

  fn parse(data: &[u8]) -> Option<Self>;
}

impl FromData for u16 {
  const SIZE: u32 = 2;

  fn parse(data: &[u8]) -> Option<Self> {
    match data {
      [high, low] => Some(u16::from_be_bytes([*high, *low])),
      _ => None,
    }
  }
}

impl FromData for u32 {
  const SIZE: u32 = 4;

  fn parse(data: &[u8]) -> Option<Self> {
    match data {
      [first, second, third, fourth] => {
        Some(u32::from_be_bytes([*first, *second, *third, *fourth]))
      }
      _ => None,
    }
  }
}

fn element<T: FromData>(data: &[u8], position: u64) -> Result<Option<T>, Never> {
  let end = match position.checked_add(u64::from(T::SIZE)) {
    Some(end) => end,
    None => return Ok(None),
  };
  let Ok(span) = indices(position..end);

  Ok(data.get(span).and_then(T::parse))
}

fn count<T: FromData>(data: &[u8]) -> Result<u64, Never> {
  let Ok(bytes) = fitted::<_, u64>(data.len());

  Ok(match bytes.checked_div(u64::from(T::SIZE)) {
    Some(many) => many,
    None => 0,
  })
}

#[derive(Clone, Copy)]
pub struct LazyArray16<'a, T> {
  data: &'a [u8],
  data_type: core::marker::PhantomData<T>,
}

impl<T> Default for LazyArray16<'_, T> {
  fn default() -> Self {
    Self {
      data: &[],
      data_type: core::marker::PhantomData,
    }
  }
}

impl<'a, T: FromData> LazyArray16<'a, T> {
  pub fn new(data: &'a [u8]) -> Result<Self, Never> {
    Ok(Self {
      data,
      data_type: core::marker::PhantomData,
    })
  }

  pub fn get(&self, position: u16) -> Result<Option<T>, Never> {
    let Ok(length) = self.len();

    match position.cmp(&length) {
      Ordering::Less => element::<T>(
        self.data,
        u64::from(position).saturating_mul(u64::from(T::SIZE)),
      ),
      Ordering::Equal | Ordering::Greater => Ok(None),
    }
  }

  pub fn len(&self) -> Result<u16, Never> {
    let Ok(many) = count::<T>(self.data);

    wrapped(many)
  }

  pub fn binary_search_by<F>(&self, mut compare: F) -> Result<Option<(u16, T)>, Never>
  where
    F: FnMut(&T) -> Ordering,
  {
    let Ok(mut size) = self.len();

    match size {
      0 => return Ok(None),
      1.. => {}
    }

    let mut base = 0u16;

    for _step in 0..u16::BITS {
      match size {
        0 | 1 => break,
        2.. => {}
      }

      let half = size.saturating_div(2);
      let middle = base.saturating_add(half);
      let Ok(found) = self.get(middle);
      let value = match found {
        Some(value) => value,
        None => return Ok(None),
      };

      base = match compare(&value) {
        Ordering::Greater => base,
        Ordering::Less | Ordering::Equal => middle,
      };
      size = size.saturating_sub(half);
    }

    let Ok(found) = self.get(base);
    let value = match found {
      Some(value) => value,
      None => return Ok(None),
    };

    Ok(match compare(&value) {
      Ordering::Equal => Some((base, value)),
      Ordering::Less | Ordering::Greater => None,
    })
  }
}

impl<'a, T: FromData> IntoIterator for LazyArray16<'a, T> {
  type Item = T;
  type IntoIter = LazyArrayIter16<'a, T>;

  fn into_iter(self) -> Self::IntoIter {
    LazyArrayIter16 {
      data: self,
      position: 0,
    }
  }
}

#[derive(Clone, Copy)]
pub struct LazyArrayIter16<'a, T> {
  data: LazyArray16<'a, T>,
  position: u16,
}

impl<T: FromData> Iterator for LazyArrayIter16<'_, T> {
  type Item = T;

  fn next(&mut self) -> Option<Self::Item> {
    let Ok(length) = self.data.len();

    match self.position.cmp(&length) {
      Ordering::Less => {}
      Ordering::Equal | Ordering::Greater => return None,
    }

    let position = self.position;
    let Ok(found) = self.data.get(position);

    self.position = self.position.saturating_add(1);

    found
  }
}

#[derive(Clone, Copy)]
pub struct LazyArray32<'a, T> {
  data: &'a [u8],
  data_type: core::marker::PhantomData<T>,
}

impl<'a, T: FromData> LazyArray32<'a, T> {
  pub fn new(data: &'a [u8]) -> Result<Self, Never> {
    Ok(Self {
      data,
      data_type: core::marker::PhantomData,
    })
  }

  pub fn get(&self, position: u32) -> Result<Option<T>, Never> {
    let Ok(length) = self.len();

    match position.cmp(&length) {
      Ordering::Less => element::<T>(
        self.data,
        u64::from(position).saturating_mul(u64::from(T::SIZE)),
      ),
      Ordering::Equal | Ordering::Greater => Ok(None),
    }
  }

  pub fn len(&self) -> Result<u32, Never> {
    let Ok(many) = count::<T>(self.data);

    wrapped(many)
  }
}

#[derive(Clone, Default, Debug)]
pub struct Stream<'a> {
  data: &'a [u8],
  position: u64,
}

impl<'a> Stream<'a> {
  pub fn new(data: &'a [u8]) -> Result<Self, Never> {
    Ok(Self { data, position: 0 })
  }

  pub fn offset(&self) -> Result<u64, Never> {
    Ok(self.position)
  }

  pub fn tail(&self) -> Result<Option<&'a [u8]>, Never> {
    let Ok(from) = indices_from(self.position..);

    Ok(self.data.get(from))
  }

  pub fn skip<T: FromData>(&mut self) -> Result<(), Never> {
    self.advance(u64::from(T::SIZE))
  }

  pub fn advance(&mut self, bytes: u64) -> Result<(), Never> {
    self.position = self.position.saturating_add(bytes);

    Ok(())
  }

  pub fn advance_checked(&mut self, bytes: u64) -> Result<Option<()>, Never> {
    let Ok(length) = fitted::<_, u64>(self.data.len());
    let end = match self.position.checked_add(bytes) {
      Some(end) => end,
      None => return Ok(None),
    };

    match end.cmp(&length) {
      Ordering::Less | Ordering::Equal => {
        self.position = end;

        Ok(Some(()))
      }
      Ordering::Greater => Ok(None),
    }
  }

  pub fn read<T: FromData>(&mut self) -> Result<Option<T>, Never> {
    let Ok(bytes) = self.read_bytes(u64::from(T::SIZE));

    Ok(bytes.and_then(T::parse))
  }

  pub fn read_at<T: FromData>(data: &[u8], position: u64) -> Result<Option<T>, Never> {
    element::<T>(data, position)
  }

  pub fn read_bytes(&mut self, bytes: u64) -> Result<Option<&'a [u8]>, Never> {
    let end = match self.position.checked_add(bytes) {
      Some(end) => end,
      None => return Ok(None),
    };
    let Ok(span) = indices(self.position..end);
    let data = match self.data.get(span) {
      Some(data) => data,
      None => return Ok(None),
    };

    self.position = end;

    Ok(Some(data))
  }

  pub fn read_array16<T: FromData>(
    &mut self,
    count: u16,
  ) -> Result<Option<LazyArray16<'a, T>>, Never> {
    let bytes = u64::from(count).saturating_mul(u64::from(T::SIZE));
    let Ok(data) = self.read_bytes(bytes);

    Ok(data.map(|bytes_2| {
      let Ok(array) = LazyArray16::new(bytes_2);

      array
    }))
  }

  pub fn read_array32<T: FromData>(
    &mut self,
    count: u32,
  ) -> Result<Option<LazyArray32<'a, T>>, Never> {
    let bytes = u64::from(count).saturating_mul(u64::from(T::SIZE));
    let Ok(data) = self.read_bytes(bytes);

    Ok(data.map(|bytes_2| {
      let Ok(array) = LazyArray32::new(bytes_2);

      array
    }))
  }
}

pub trait Offset {
  fn position(&self) -> Result<u64, Never>;
}

#[derive(Clone, Copy, Debug)]
pub struct Offset16(pub u16);

impl Offset for Offset16 {
  fn position(&self) -> Result<u64, Never> {
    Ok(u64::from(self.0))
  }
}

impl FromData for Offset16 {
  const SIZE: u32 = 2;

  fn parse(data: &[u8]) -> Option<Self> {
    u16::parse(data).map(Offset16)
  }
}

#[derive(Clone, Copy, Debug)]
pub struct Offset32(pub u32);

impl Offset for Offset32 {
  fn position(&self) -> Result<u64, Never> {
    Ok(u64::from(self.0))
  }
}

impl FromData for Offset32 {
  const SIZE: u32 = 4;

  fn parse(data: &[u8]) -> Option<Self> {
    u32::parse(data).map(Offset32)
  }
}
