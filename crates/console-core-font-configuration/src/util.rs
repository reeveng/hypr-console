#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one attribute walk assigns fields of different types in optional patterns and rejects the pattern on an invalid attribute"
  )
)]
macro_rules! parse_attrs_opt {
    ($node:expr, { $($key:expr => $lvalue:expr,)+ } $(, { $($str_key:expr => $str_lvalue:expr,)+ } )?) => {
        for attr in ({ let Ok(explicit_value) = $node.attributes(); explicit_value }) {
            match ({ let Ok(explicit_value) = attr.name(); explicit_value }) {
                $(
                    $key => {
                        let Ok(value) = attr.value();
                        $lvalue = match value.parse() {
                            Ok(parsed) => parsed,
                            Err(_invalid_attribute) => return None,
                        };
                    },
                )+
                $(
                    $(
                        $str_key => { let Ok(value) = attr.value(); $str_lvalue = String::from(value); },
                    )+
                )?
                _ => {}
            }
        }
    };
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "one attribute walk assigns fields of different types while preserving each field parser and propagating its error"
  )
)]
macro_rules! parse_attrs {
    ($node:expr, { $($key:expr => $lvalue:expr,)+ } $(, { $($str_key:expr => $str_lvalue:expr,)+ } )?) => {
        for attr in ({ let Ok(explicit_value) = $node.attributes(); explicit_value }) {
            match ({ let Ok(explicit_value) = attr.name(); explicit_value }) {
                $(
                    $key => {
                        let Ok(value) = attr.value();
                        let parsed = value.parse()?;
                        $lvalue = parsed;
                    },
                )+
                $(
                    $(
                        $str_key => { let Ok(value) = attr.value(); $str_lvalue = String::from(value); },
                    )+
                )?
                _ => {}
            }
        }
    };
}

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the table pairs enum variants with XML spellings and generates the corresponding FromStr implementation with the caller’s fallback"
  )
)]
macro_rules! parse_enum {
    (
        $ty:ty,
        $(
            ($variant:ident, $text:expr),
        )+
        |$arg:ident| $fallback:expr,
    ) => {
        impl core::str::FromStr for $ty {
            type Err = crate::Error;

            fn from_str($arg: &str) -> crate::Result<$ty> {
                match $arg {
                    $(
                        $text => Ok(<$ty>::$variant),
                    )+
                    _ => {
                        $fallback
                    }
                }
            }
        }
    };
    (
        $ty:ty,
        $(
            ($variant:ident, $text:expr),
        )+
    ) => {
        parse_enum! {
            $ty,
            $(
                ($variant, $text),
            )+
            |s| Err(crate::Error::ParseEnumError(core::any::type_name::<$ty>(), String::from(s))),
        }
    };
}
