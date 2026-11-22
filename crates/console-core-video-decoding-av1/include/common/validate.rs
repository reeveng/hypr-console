#![forbid(unsafe_code)]
#[cfg(feature = "c-ffi")]
use console_core_never::Never;

#[cfg(feature = "c-ffi")]
pub(crate) fn parent_type_name_of<T>(_: &T) -> Result<&'static str, Never> {
  let function = core::any::type_name::<T>();

  Ok(match function.strip_suffix("::f") {
    Some(containing_name) => containing_name,
    None => function,
  })
}

#[cfg(feature = "c-ffi")]
#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "this helper places a named item at the validation site so its type carries the containing function name"
  )
)]
macro_rules! func_name {
  () => {{
    fn f() -> ::core::result::Result<(), ::console_core_never::Never> {
      Ok(())
    }
    $crate::include::common::validate::parent_type_name_of(&f)
  }};
}

#[cfg(feature = "c-ffi")]
pub(crate) use func_name;

pub trait ValidatedIntoResult<T, E> {
  fn into_result(self) -> Result<T, E>;
}

impl<T, E> ValidatedIntoResult<T, E> for Result<T, E> {
  fn into_result(self) -> Result<T, E> {
    self
  }
}

impl<E> ValidatedIntoResult<(), E> for (bool, E) {
  fn into_result(self) -> Result<(), E> {
    let (ok, e) = self;

    match ::core::primitive::bool::then_some(ok, ()) {
      ::core::option::Option::Some(()) => Ok(()),
      ::core::option::Option::None => Err(e),
    }
  }
}

impl ValidatedIntoResult<(), ()> for bool {
  fn into_result(self) -> Result<(), ()> {
    (self, ()).into_result()
  }
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "validation needs its condition text and source location from the invocation, alongside the caller's optional recovery block"
  )
)]
macro_rules! validate_input {
  ($condition:expr, $block:block) => {{
    use $crate::include::common::validate::ValidatedIntoResult;
    $condition.into_result().map_err(|e| {
      #[cfg(feature = "c-ffi")]
      {
        let Ok(function) = $crate::include::common::validate::func_name!();
        console_core_logging::warn!(
          "Input validation check `{}` failed in `fn {}` in `{}:{}:{}`!",
          stringify!($condition),
          function,
          file!(),
          line!(),
          column!(),
        );
      }
      $block;
      e
    })
  }};

  ($condition:expr) => {
    validate_input!($condition, {})
  };
}

pub(crate) use validate_input;
