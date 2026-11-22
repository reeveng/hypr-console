// run-rustfix
// edition:2024

fn read(bytes: [u8; 8]) -> (u8, u8) {
    (bytes[2 + 3], bytes[6])
}

fn tail(bytes: &[u8; 8]) -> u8 {
    bytes[4]
}

fn borrow_mut(bytes: &mut [String; 3]) -> &mut String {
    &mut bytes[1]
}

fn assign(words: &mut [String; 3], replacement: fn(&[String; 3]) -> String) {
    words[0] = replacement(words);
}

fn suffix<'a>(bytes: &&'a [u8; 8]) -> &'a [u8] {
    &bytes[2 * 3..]
}

fn suffix_mut(bytes: &mut [u8; 8]) -> &mut [u8] {
    &mut bytes[7..]
}

fn whole(bytes: &[u8; 8]) -> &[u8] {
    &bytes[..]
}

fn empty(bytes: &[u8; 8]) -> &[u8] {
    &bytes[8..]
}

fn nested(mut grid: [[u8; 4]; 2]) -> (u8, u8) {
    grid[1][2] += grid[0][1];
    let updated = grid[1][2];
    (updated, grid[1][3])
}

fn row(grid: &[[u8; 4]; 2]) -> [u8; 4] {
    grid[1]
}

fn main() {
    let mut bytes = [0, 1, 2, 3, 4, 5, 6, 7];
    assert_eq!(read(bytes), (5, 6));
    assert_eq!(tail(&bytes), 4);
    assert_eq!(suffix(&&bytes), &[6, 7]);
    assert_eq!(whole(&bytes), &bytes);
    assert!(empty(&bytes).is_empty());
    suffix_mut(&mut bytes).fill(8);
    assert_eq!(suffix(&&bytes), &[6, 8]);
    let mut words = [String::from("one"), String::from("two"), String::from("three")];
    borrow_mut(&mut words).push('!');
    assert!(words.contains(&String::from("two!")));
    assign(&mut words, |values| values.concat());
    assert!(words.contains(&String::from("onetwo!three")));
    assert_eq!(nested([[1, 2, 3, 4], [5, 6, 7, 8]]), (9, 8));
    assert_eq!(row(&[[1, 2, 3, 4], [5, 6, 7, 8]]), [5, 6, 7, 8]);
}
