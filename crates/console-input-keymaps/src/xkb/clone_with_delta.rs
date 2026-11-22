use crate::xkb::span::Span;
use crate::xkb::span::SpanUnit;

pub(crate) trait CloneWithDelta: Sized {
  fn clone_with_delta(&self, delta: SpanUnit) -> Self;
}

impl CloneWithDelta for Span {
  fn clone_with_delta(&self, delta: SpanUnit) -> Self {
    *self + delta
  }
}

impl<T> CloneWithDelta for Vec<T>
where
  T: CloneWithDelta,
{
  fn clone_with_delta(&self, delta: SpanUnit) -> Self {
    self.iter().map(|t| t.clone_with_delta(delta)).collect()
  }
}

impl<T> CloneWithDelta for Option<T>
where
  T: CloneWithDelta,
{
  fn clone_with_delta(&self, delta: SpanUnit) -> Self {
    self.as_ref().map(|t| t.clone_with_delta(delta))
  }
}

impl<T> CloneWithDelta for Box<T>
where
  T: CloneWithDelta,
{
  fn clone_with_delta(&self, delta: SpanUnit) -> Self {
    Box::new((**self).clone_with_delta(delta))
  }
}

impl<T> CloneWithDelta for Box<[T]>
where
  T: CloneWithDelta,
{
  fn clone_with_delta(&self, delta: SpanUnit) -> Self {
    self.iter().map(|t| t.clone_with_delta(delta)).collect()
  }
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  allow(
    explicit080_macro_budget,
    reason = "a value with no span in it is moved by being copied, and every kind of one says so through the same impl, written once rather than once per kind"
  )
)]
macro_rules! copy {
  ($ty:ty) => {
    impl CloneWithDelta for $ty {
      fn clone_with_delta(&self, _delta: SpanUnit) -> Self {
        *self
      }
    }
  };
}

copy!(u32);
copy!(u64);
copy!(bool);
copy!(f64);
copy!(i64);
