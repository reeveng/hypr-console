// edition:2024
#![allow(dead_code)]
fn unchecked(bytes:&[u8]) -> &[u8;4] { <&[u8;4]>::try_from(bytes).unwrap() }
fn effects(bytes:&[u8], error:impl FnOnce()->()) -> Result<&[u8;4],()> {
    Ok(<&[u8;4]>::try_from(bytes.get(..4).ok_or(error())?).unwrap())
}
fn closure(bytes:&[u8]) -> Result<[u8;4],()> {
    let value = bytes.get(..4).ok_or(())?;
    let read = || *<&[u8;4]>::try_from(value).unwrap();
    Ok(read())
}
fn main(){}
