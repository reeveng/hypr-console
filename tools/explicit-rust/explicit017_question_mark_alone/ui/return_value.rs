// run-rustfix
// edition:2024

fn result(value: Result<u8, String>) -> Result<u8, String> {
    Ok(value?)
}

fn option(value: Option<u8>) -> Option<u8> {
    return Some(value?);
}

fn copied(bytes: &[u8], index: usize) -> Option<u8> {
    let byte = *bytes.get(index)?;
    Some(byte)
}

fn copied_result<'a>(input: Result<&u8, &'a str>) -> Result<u8, &'a str> {
    let byte = *input?;
    Ok(byte)
}

fn main() {
    assert_eq!(result(Ok(8)), Ok(8));
    assert_eq!(result(Err(String::from("error"))), Err(String::from("error")));
    assert_eq!(option(Some(8)), Some(8));
    assert_eq!(option(None), None);
    assert_eq!(copied(&[5], 0), Some(5));
    assert_eq!(copied(&[5], 1), None);
    assert_eq!(copied_result(Ok(&5)), Ok(5));
    assert_eq!(copied_result(Err("error")), Err("error"));
}
