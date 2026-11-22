#![forbid(unsafe_code)]

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "quantization and wedge tables share one constant-evaluation range traversal because iterator loops cannot run in const initializers"
  )
)]
macro_rules! const_for {
    ($index:ident in $range:expr, step_by $step:expr => $block:block) => {{
        use std::ops::Range;

        let range: Range<_> = $range;
        let step = $step;
        let mut $index = range.start;
        #[cfg_attr(
          dylint_lib = "explicit053_no_while",
          expect(
            explicit053_no_while,
            reason = "table initialization has a fixed exclusive range end and a positive step, and iterator loops are unavailable in const evaluation"
          )
        )]
        while $index < range.end {
            $block
            $index += step;
        }
    }};
    ($index:ident in $range:expr => $block:block) => {
        const_for!($index in $range, step_by 1 => $block)
    };
}

pub(crate) use const_for;

#[cfg_attr(
  dylint_lib = "explicit080_macro_budget",
  expect(
    explicit080_macro_budget,
    reason = "the wedge tables need a minimum that evaluates each argument once and is callable in const initializers"
  )
)]
macro_rules! const_min {
  ($a:expr, $b:expr) => {{
    let a = $a;
    let b = $b;
    if a < b { a } else { b }
  }};
}

pub(crate) use const_min;
