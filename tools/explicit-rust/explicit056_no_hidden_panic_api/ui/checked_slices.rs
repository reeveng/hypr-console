// run-rustfix
// edition:2024

fn swapped(items: &mut [String; 4]) {
    items.swap(2, 0);
}

fn split(data: &[u8], at: usize) -> (&[u8], &[u8]) {
    let (front, back) = data.split_at(at.min(data.len()));
    (front, back)
}

struct Held {
    data: Vec<u8>,
}

fn split_vec(held: &Held, at: usize) -> (&[u8], &[u8]) {
    let (front, back) = held.data.split_at(core::cmp::min(held.data.len(), at));
    (front, back)
}

fn split_fixed(data: &[u8; 3], at: usize) -> (&[u8], &[u8]) {
    let (front, back) = data.split_at(at.min(data.len()));
    (front, back)
}

fn main() {
    let mut items = ["one", "two", "three", "four"].map(String::from);
    swapped(&mut items);
    assert_eq!(items[0], "three");
    assert_eq!(items[2], "one");

    let mut fixed = [1, 2, 3];
    fixed.swap(0, 1);
    assert_eq!(fixed, [2, 1, 3]);

    let data = [1, 2, 3];
    assert_eq!(split(&data, 1), (&data[..1], &data[1..]));
    let (front, back) = split(&data, 100);
    assert_eq!(front, &data);
    assert!(back.is_empty());
    assert_eq!(back.as_ptr(), data.as_ptr().wrapping_add(data.len()));

    let held = Held { data: data.to_vec() };
    let (front, back) = split_vec(&held, 100);
    assert_eq!(front, &data);
    assert!(back.is_empty());
    assert_eq!(back.as_ptr(), held.data.as_ptr().wrapping_add(data.len()));

    let (front, back) = split_fixed(&data, 100);
    assert_eq!(front, &data);
    assert_eq!(back.as_ptr(), data.as_ptr().wrapping_add(data.len()));
}
