// edition:2024

macro_rules! load {
    ($slice:expr, $lanes:ty) => {{
        let bytes = $slice;
        u8::try_from(bytes.len()).unwrap()
    }};
}
macro_rules! conditional {
    ($slice:expr, $take:expr) => { if $take { $slice.len() } else { 0 } };
}
fn skip(src: &[u8], take: bool) -> Option<usize> {
    let width = conditional!(&*(src.get(1..)?), take);
    Some(width)
}
fn short(src: &[u8]) -> Option<bool> {
    let out = false && load!(&*(src.get(8..)?), [u8; 16]) == 3;
    Some(out)
}
fn temporary(src: &[u8], ix: &std::cell::RefCell<usize>) -> Option<u8> {
    let width = load!(&*(src.get(*ix.borrow()..)?), [u8; 16]);
    Some(width)
}
macro_rules! sometimes {
    ($body:expr) => {{ if false { $body; } }};
}
fn forwarded(src: &[u8]) -> Option<()> {
    sometimes!(load!(&*(src.get(10..)?), [u8; 16]));
    Some(())
}
fn choose(src: &[u8], take: bool) -> Option<u8> {
    let width = load!(if take { <&[u8; 4]>::try_from(&*(src.get(..4)?)).unwrap() }
        else { &[0; 4] }, [u8; 4]);
    Some(width)
}
fn pair(left: u8, right: u8) -> u8 { left + right }
fn later(src: &[u8], first: impl FnOnce() -> u8) -> Option<u8> {
    let width = pair(first(), load!(<&[u8; 4]>::try_from(&*(src.get(..4)?)).unwrap(), [u8; 4]));
    Some(width)
}
fn main() {
    assert_eq!(skip(&[], false), Some(0));
    assert_eq!(short(&[]), Some(false));
    assert_eq!(forwarded(&[]), Some(()));
    assert_eq!(choose(&[], false), Some(4));
    let mark = std::cell::Cell::new(false);
    assert_eq!(later(&[], || {mark.set(true); 1}), None);
    assert!(mark.get());
}
