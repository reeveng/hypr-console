// edition:2024
#![allow(dead_code)]

fn wrong_table<'a>(table: &[i32], other: &'a [i32], key: i32) -> Option<&'a i32> {
    table.binary_search(&key).ok().map(|index| &other[index])
}

fn changes_position(table: &[i32], key: i32) -> Option<i32> {
    table.binary_search(&key).ok().map(|index| table[index + 1])
}

fn mutable_method(table: &mut [String], key: &String) -> Option<()> {
    table.binary_search(key).ok().map(|index| table[index].clear())
}

fn preceding_effect(table: &[i32], key: i32) -> Option<(i32, i32)> {
    table.binary_search(&key).ok().map(|index| (std::hint::black_box(0), table[index]))
}

fn main() {}
