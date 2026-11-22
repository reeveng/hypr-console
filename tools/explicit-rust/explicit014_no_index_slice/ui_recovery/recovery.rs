// run-rustfix
// edition:2024

#[derive(Debug, PartialEq)]
enum Fault {
    Bounds,
}

#[derive(Debug, PartialEq)]
enum Message {
    Invalid(String),
}

fn read(values: &[u8], index: usize) -> Result<u8, Fault> {
    Ok(values[index])
}

fn read_with_overlap(values: &[u8], indexed_value: usize) -> Result<u8, Fault> {
    Ok(values[indexed_value])
}

fn write(values: &mut [[u8; 2]], row: usize) -> Result<(), Fault> {
    values[row] = [1, 42];
    Ok(())
}

fn copy(values: &mut Vec<u8>, end: usize, bytes: &[u8]) -> Result<(), Fault> {
    values[..end].copy_from_slice(bytes);
    Ok(())
}

fn tail(values: &[u8], start: usize) -> Result<&[u8], Message> {
    Ok(&values[start..])
}

fn legacy(values: &[u8], index: usize) -> Result<u8, ()> {
    Ok(values[index])
}

fn write_order(values: &mut [u8], index: usize, effects: &mut Vec<u8>) -> Result<(), Fault> {
    values[{ effects.push(2); index }] += { effects.push(1); 7 };
    Ok(())
}

fn array_place(values: &mut [u8; 2]) -> &mut [u8; 2] {
    values
}

fn fixed_slot(values: &mut [u8; 2]) -> &mut u8 {
    &mut array_place(values)[1]
}

fn main() {
    let mut values = [[1, 2], [3, 4]];
    assert_eq!(write(&mut values, 0), Ok(()));
    assert_eq!(values, [[1, 42], [3, 4]]);
    assert_eq!(write(&mut values, 2), Err(Fault::Bounds));
    let mut pair = [3, 4];
    *fixed_slot(&mut pair) = 12;
    assert_eq!(pair, [3, 12]);

    let mut bytes = vec![1, 2, 3];
    assert_eq!(copy(&mut bytes, 2, &[4, 5]), Ok(()));
    assert_eq!(read(&bytes, 1), Ok(5));
    assert_eq!(read_with_overlap(&bytes, 1), Ok(5));
    assert_eq!(copy(&mut bytes, 4, &[1, 2, 3, 4]), Err(Fault::Bounds));
    assert_eq!(read(&bytes, 3), Err(Fault::Bounds));
    assert_eq!(tail(&bytes, 3), Ok([].as_slice()));
    assert_eq!(tail(&bytes, 4), Err(Message::Invalid("index or range outside the buffer".into())));
    assert_eq!(legacy(&bytes, 4), Err(()));

    let mut effects = Vec::new();
    assert_eq!(write_order(&mut bytes, 0, &mut effects), Ok(()));
    assert_eq!(effects, [1, 2]);
    assert_eq!(bytes, [11, 5, 3]);
}
