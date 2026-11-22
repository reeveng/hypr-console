// edition:2024
#![allow(dead_code)]
#[derive(Debug)]
enum ReadError { OutOfBounds }

fn partial_reads(start: usize) -> Result<(), ReadError> {
    let tail = &[vec![1], vec![2]][start..];
    let first = &[String::from("font")][0];
    let tail_mut = &mut [vec![1], vec![2]][start..];
    let first_mut = &mut [String::from("font")][0];
    assert_eq!(tail, tail_mut);
    first_mut.push('s');
    assert_eq!(first, "font");
    Ok(())
}

fn main() { partial_reads(1).unwrap(); }
