// run-rustfix
// edition:2024

macro_rules! load {
    ($slice:expr, $lanes:ty) => {{
        let bytes = $slice;
        u8::try_from(bytes.len()).unwrap()
    }};
}
macro_rules! store {
    ($slice:expr, $value:expr) => {{
        let bytes = $slice;
        bytes.fill($value);
    }};
}
fn row(src: &[u8], dst: &mut [u8], off: usize, len: usize) -> Result<u8, ()> {
    let width = load!(
        &*(<[_]>::get(&*src, off..len).ok_or(())?),
        [u8; 16]
    );
    store!(&mut *(<[_]>::get_mut(&mut *dst, off..len).ok_or(())?), width);
    Ok(width)
}
fn slice_rows(src: &[u8], dst: &mut [u8]) -> Option<()> {
    let row = &*(<[_]>::get(&*src, 1..)?);
    let target = &mut *(<[_]>::get_mut(&mut *dst, 1..)?);
    target.copy_from_slice(row);
    Some(())
}
fn converted(src: &[u8], dst: &mut [u8], end: usize, calls: &mut u8) -> Result<u8, ()> {
    let width = load!(
        <&[u8; 4]>::try_from(&*(src.get(..end).ok_or(())?)).unwrap(),
        [u8; 4]
    );
    store!(<&mut [u8; 4]>::try_from(&mut *(dst.get_mut(..end).ok_or(())?)).unwrap(),
        { *calls += 1; width });
    Ok(width)
}
fn nested(src: &[u8], end: usize) -> Option<u32> {
    let width = u32::from(load!(<&[u8; 4]>::try_from(&*(src.get(..end)?)).unwrap(), [u8; 4]));
    Some(width)
}
fn array_length(_: &[u8; 4]) -> u8 { 4 }
fn fill_array(dst: &mut [u8; 4], value: u8) { dst.fill(value) }
macro_rules! load_array { ($array:expr) => { array_length($array) }; }
macro_rules! store_array { ($array:expr, $value:expr $(,)?) => { fill_array($array, $value) }; }
fn exact(src: &[u8], dst: &mut [u8], end: usize, calls: &mut u8) -> Result<u8, ()> {
    let width = load_array!(src.get(..end).ok_or(()).and_then(|slice| <&[u8; 4]>::try_from(slice).map_err(|_| ()))?);
    store_array!(<&mut [u8; 4]>::try_from(dst).map_err(|_| ())?, { *calls += 1; width });
    Ok(width)
}
fn conditional(src: &[u8], active: bool) -> Result<bool, &'static str> {
    Ok(active && load_array![
        src.get(..4).ok_or("bounds").and_then(|slice| <&[u8; 4]>::try_from(slice).map_err(|_| "length"))?
    ] == 4)
}
fn sequence(dst: &mut [u8], order: &std::cell::Cell<u8>) -> Result<(), ()> {
    fn done(_: u8, _: ()) {}
    done({ order.set(1); 0 }, store_array!(
        <&mut [u8; 4]>::try_from(dst).map_err(|_| ())?,
        { order.set(order.get() * 10 + 2); 7 },
    ));
    Ok(())
}
fn nested_write(rows: &mut [[u8; 4]], row: usize, column: usize) -> Option<()> {
    *<[_]>::get_mut(&mut *{
        let held = rows.get_mut(row)?;
        held
    }, column)? = 9;
    Some(())
}
static TABLE: [u8; 4] = [1, 2, 3, 4];
fn view(bytes: &[u8; 4]) -> &[u8] { bytes }
fn from_table(start: usize, end: usize) -> Option<u8> {
    let part = &*(<[_]>::get(
        &*(<[u8]>::get(view(&TABLE), start..)?),
        ..end,
    )?);
    part.first().copied()
}
fn main() {
    let mut dest = [0; 3];
    assert_eq!(row(&[0; 3], &mut dest, 1, 3), Ok(2));
    assert_eq!(dest, [0, 2, 2]);
    assert_eq!(row(&[], &mut dest, 1, 3), Err(()));
    assert_eq!(slice_rows(&[4, 6, 5], &mut dest), Some(()));
    assert_eq!(dest, [0, 6, 5]);
    let mut calls = 0;
    let mut target = [0; 4];
    assert_eq!(converted(&[1; 4], &mut target, 4, &mut calls), Ok(4));
    assert_eq!((target, calls), ([4; 4], 1));
    assert_eq!(converted(&[1; 4], &mut [], 4, &mut calls), Err(()));
    assert_eq!(calls, 1);
    assert_eq!(converted(&[], &mut target, 4, &mut calls), Err(()));
    assert!(std::panic::catch_unwind(|| converted(&[0; 4], &mut [0; 4], 2, &mut 0)).is_err());
    assert_eq!(nested(&[0; 4], 4), Some(4));
    assert_eq!(nested(&[], 4), None);
    assert_eq!(exact(&[0; 4], &mut target, 4, &mut calls), Ok(4));
    assert_eq!(exact(&[0; 5], &mut target, 5, &mut calls), Err(()));
    assert_eq!(exact(&[0; 4], &mut [0; 3], 4, &mut calls), Err(()));
    assert_eq!(calls, 2);
    assert_eq!(conditional(&[], false), Ok(false));
    assert_eq!(conditional(&[], true), Err("bounds"));
    assert_eq!(conditional(&[0; 4], true), Ok(true));
    let trace = std::cell::Cell::new(0);
    assert_eq!(sequence(&mut [], &trace), Err(()));
    assert_eq!(trace.get(), 1);
    assert_eq!(sequence(&mut target, &trace), Ok(()));
    assert_eq!((target, trace.get()), ([7; 4], 12));
    let mut rows = [[0; 4]];
    assert_eq!(nested_write(&mut rows, 0, 1), Some(()));
    assert_eq!(rows, [[0, 9, 0, 0]]);
    assert_eq!(nested_write(&mut rows, 0, 4), None);
    assert_eq!(nested_write(&mut rows, 1, 0), None);
    assert_eq!(from_table(2, 1), Some(3));
    assert_eq!(from_table(3, 2), None);
}
