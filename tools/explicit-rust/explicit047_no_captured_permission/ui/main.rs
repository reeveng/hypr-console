// UI test for EXPLICIT047 — a closure takes what it writes rather than holding
// the permission to write it.

// BAD EXPLICIT047 — the count and the right to add to it are one value.
fn counts_them(words: &[&str]) -> usize {
    let mut said = 0;

    //~v EXPLICIT047_NO_CAPTURED_PERMISSION
    let mut add = |word: &str| said += word.len();

    for word in words {
        add(word);
    }

    said
}

// GOOD — what is written is an argument, so the call says it writes.
fn says_it(words: &[&str]) -> usize {
    let add = |said: &mut usize, word: &str| *said += word.len();
    let mut said = 0;

    for word in words {
        add(&mut said, word);
    }

    said
}

// GOOD — nothing is written, so nothing is carried.
fn reads_only(words: &[&str]) -> usize {
    let longest = 4;
    let long = |word: &&str| word.len() > longest;

    words.iter().filter(|word| long(word)).count()
}

fn main() {}
