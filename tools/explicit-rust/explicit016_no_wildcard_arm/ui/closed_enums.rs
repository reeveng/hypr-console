// run-rustfix
// edition:2024

use std::cmp::Ordering;

fn category(value: f32) -> u8 {
    match value.classify() {
        ::core::num::FpCategory::Zero => {
            #[cfg(all())]
            let score = 0;
            #[cfg(any())]
            let score = 2;
            score
        }
        _ => 1,
    }
}

fn exactly_equal(value: Option<&Ordering>) -> u8 {
    match value {
        ::core::option::Option::Some(::core::cmp::Ordering::Greater) => 1,
        ::core::option::Option::Some(::core::cmp::Ordering::Less) => 2,
        _ => 3,
    }
}

fn keep(value: Result<Ordering, String>) -> Result<Ordering, String> {
    match value {
        ::core::result::Result::Ok(::core::cmp::Ordering::Less | ::core::cmp::Ordering::Equal) => {
            Ok(Ordering::Greater)
        }
        other => other,
    }
}

fn main() {
    assert_eq!(category(-0.0), 0);
    assert_eq!(category(f32::INFINITY), 1);
    assert_eq!(category(f32::NAN), 1);
    assert_eq!(exactly_equal(Some(&Ordering::Greater)), 1);
    assert_eq!(exactly_equal(Some(&Ordering::Less)), 2);
    assert_eq!(exactly_equal(Some(&Ordering::Equal)), 3);
    assert_eq!(exactly_equal(None), 3);
    assert_eq!(keep(Ok(Ordering::Less)), Ok(Ordering::Greater));
    assert_eq!(keep(Ok(Ordering::Greater)), Ok(Ordering::Greater));
    assert_eq!(keep(Err(String::from("error"))), Err(String::from("error")));
}
