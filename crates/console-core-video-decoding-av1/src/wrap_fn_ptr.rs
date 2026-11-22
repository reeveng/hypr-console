#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the DSP entry points use the same zero-sized default dispatch marker with distinct module and type identities"
  )
)]
macro_rules! wrap_fn_ptr {
    ($vis:vis $name:ident) => {
        $vis mod $name {
            use $crate::src::enum_map::DefaultValue;

            #[derive(Clone, Copy, PartialEq, Eq)]
            pub struct Fn(());

            impl DefaultValue for Fn {
                const DEFAULT: Self = Fn(());
            }
        }
    };
}

pub(crate) use wrap_fn_ptr;
