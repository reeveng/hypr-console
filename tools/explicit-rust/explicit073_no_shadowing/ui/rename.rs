// run-rustfix

struct Pair {
    value: u8,
}

enum Either {
    Left(u8),
    Right(u8),
}

mod defaults {
    #[allow(non_upper_case_globals)]
    pub const limit_2: u8 = 3;
}
use defaults::*;

fn repeated(value: u8) -> u8 {
    let value_2 = 7;
    let value = value + value_2;
    let read = || value;
    let mut value = read();
    value += 1;
    value
}

fn shorthand(value: u8) -> Pair {
    let pair = Pair { value };
    let Pair { ref value } = pair;
    let value = *value;
    Pair { value }
}

fn alternatives(value: u8, either: Either) -> u8 {
    (match either {
        Either::Left(value) | Either::Right(value) => value,
    }) + value
}

fn keyword(r#type: u8) -> u8 {
    let r#type = r#type + 1;
    r#type
}

fn imported(limit: u8) -> u8 {
    let limit = limit + 1;
    limit
}

fn standard_macros(value: u8) -> (u8, String) {
    use crate::defaults::limit_2;
    let value = value + limit_2;
    assert_eq!(vec![value], vec![7]);
    debug_assert_ne!(value, 0);
    assert_eq!(matches!(Some(value), Some(7)), true);
    (value, format!("{:02}", value))
}

fn scrutinee(value: u8) -> u8 {
    match { let value = value + 1; value } {
        result => result,
    }
}

fn separate_branch(input: Option<u8>) -> u8 {
    if let Some(value) = input {
        value
    } else {
        let value = 3;
        value
    }
}

fn main() {
    assert_eq!(repeated(10), 18);
    assert_eq!(shorthand(8).value, 8);
    assert_eq!(alternatives(2, Either::Left(12)), 14);
    assert_eq!(alternatives(3, Either::Right(12)), 15);
    assert_eq!(keyword(4), 5);
    assert_eq!(imported(4), 5);
    assert_eq!(standard_macros(4), (7, "07".to_owned()));
    assert_eq!(limit_2, 3);
    assert_eq!(scrutinee(4), 5);
    assert_eq!(separate_branch(None), 3);
}
