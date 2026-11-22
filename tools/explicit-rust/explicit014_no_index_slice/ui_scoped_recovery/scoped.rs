enum Fault {
    Bounds,
}

fn selected(values: &[u8], index: usize) -> Result<u8, Fault> {
    Ok(values[index])
}

fn outside_scope(values: &[u8], index: usize) -> Result<u8, Fault> {
    Ok(values[index])
}

fn character(text: &str, end: usize) -> Option<&str> {
    Some(&text[..end])
}

fn update(values: &mut [u8], index: usize, value: u8) -> Option<()> {
    values[index] = value;
    Some(())
}

fn option_without_policy(values: &[u8], index: usize) -> Option<u8> {
    Some(values[index])
}

fn main() {
    let _ = selected(&[0], 0);
    let _ = outside_scope(&[0], 0);
    assert_eq!(character("é", 1), None);
    assert_eq!(character("é", 2), Some("é"));
    let mut values = [0];
    assert_eq!(update(&mut values, 1, 8), None);
    assert_eq!(update(&mut values, 0, 8), Some(()));
    assert_eq!(values, [8]);
    let _ = option_without_policy(&values, 0);
}
