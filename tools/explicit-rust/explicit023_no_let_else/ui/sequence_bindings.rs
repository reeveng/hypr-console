// run-rustfix
// edition:2024

enum Never {}

fn present(value: u8) -> Option<u8> {
    Some(value)
}

fn checked(value: u8) -> Result<Option<u8>, Never> {
    Ok(present(value))
}

fn pair(values: (Option<u8>, Option<u8>)) -> u8 {
    let (Some(left), Some(right)) = values else {
        return 0;
    };
    left + right
}

fn staged_pair() -> u8 {
    let (Some(_left), Some(right)) = (
        present(5),
        {
            let Ok(value) = checked(8);
            value
        },
    ) else {
        return 0;
    };
    _left + right
}

fn read_row(row: &[Option<u8>; 3]) -> u8 {
    let [
        Some(first),
        Some(second),
        Some(third),
    ] = row.each_ref().map(Option::as_ref) else {
        return 0;
    };
    first + second + third
}

fn singleton(value: (Option<u8>,)) -> u8 {
    let (Some(value),) = value else {
        return 0;
    };
    value
}

fn main() {
    assert_eq!(pair((Some(10), Some(20))), 30);
    assert_eq!(pair((Some(10), None)), 0);
    assert_eq!(staged_pair(), 13);
    assert_eq!(read_row(&[Some(1), Some(2), Some(3)]), 6);
    assert_eq!(read_row(&[None, Some(2), Some(3)]), 0);
    assert_eq!(singleton((Some(10),)), 10);
}
