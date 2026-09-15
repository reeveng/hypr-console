//! The gate: a word written far out of English's proportion is a word this tree
//! decided on, and `words.conf` is where the deciding is written down.
//!
//! What fails here is never one call site. It is a word that has spread far
//! enough to be a habit and was never argued for, and the fix is one of two
//! things: put it under a heading in `words.conf` with the other words it
//! belongs with, or find the four places that reached for it and give each of
//! them the word that says what it does there.
//!
//! The table is printed either way, because the number nobody is failing is the
//! one worth reading: run `just words` and the words at the top are the tree's
//! own accent, in order.

use std::path::{Path, PathBuf};

use console_vocabulary::counting::counted;
use console_vocabulary::declared::{Declared, WORDS};
use console_vocabulary::elsewhere::Elsewhere;
use console_vocabulary::norm::{Norm, beside};
use console_vocabulary::{Further, Known, LEANING_ON, Measured, TOO_FAR, Vocabulary, known, measured, undeclared};

fn root() -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    from.canonicalize().unwrap_or(from)
}

fn table(every: &[&Measured], vocabulary: &Vocabulary<'_>) -> String {
    every
        .iter()
        .map(|word| {
            let Ok(times) = word.how_far();
            let Ok(known) = known(&word.word, vocabulary);
            let english = match word.in_english {
                Some(rate) => format!("{:.2}", rate.0),
                None => "never".to_string(),
            };
            let under = match known {
                Known::Under(heading) => format!("[{heading}]"),
                Known::Named(name) => {
                    let Ok(spelled) = name.spelled();

                    spelled.to_string()
                },
                Known::TheNegativeOf(root) => format!("not {root}"),
                Known::Nowhere => word.written_in.join(", "),
            };

            format!(
                "  {:22} {:>6} uses  {:>8.0} per million here  {:>8} in English  {times:>8.0}x  {under}\n",
                word.word, word.uses.0, word.here.0, english,
            )
        })
        .collect()
}

#[test]
fn a_word_written_far_out_of_proportion_is_a_word_this_tree_declared() {
    let root = root();
    let Ok(english) = beside();
    let norm = Norm::read(&english).expect("the English rates beside this crate");
    let counted = counted(&root).expect("the words in the tree");
    let declared = Declared::read(&root.join(WORDS)).expect(WORDS);
    let elsewhere = Elsewhere::read(&root).expect("the crates and the programs");
    let vocabulary = Vocabulary { declared: &declared, elsewhere: &elsewhere, norm: &norm };
    let Ok(measured) = measured(&counted, &norm);
    let Ok(undeclared) = undeclared(&measured, &vocabulary);
    let unheard: Vec<&Measured> =
        measured.iter().filter(|word| word.times.is_none()).take(TOP).collect();
    let mut leant_on: Vec<&Measured> = measured
        .iter()
        .filter(|word| match word.times {
            Some(times) => {
                let Ok(further) = times.past(LEANING_ON);

                further == Further::Yes
            },
            None => false,
        })
        .collect();

    leant_on.sort_by_key(|word| std::cmp::Reverse(word.uses));

    let leant_on: Vec<&Measured> = leant_on.into_iter().take(TOP).collect();

    println!(
        "words English has no rate for, by how often this tree writes them:\n{}",
        table(&unheard, &vocabulary)
    );
    println!(
        "ordinary words this tree leans on more than {}x as hard as English does:\n{}",
        LEANING_ON.0,
        table(&leant_on, &vocabulary)
    );

    assert!(
        undeclared.is_empty(),
        "{} word(s) are written more than {}x as often as English writes them and are \
         under no heading in {WORDS}:\n{}",
        undeclared.len(),
        TOO_FAR.0,
        table(&undeclared, &vocabulary)
    );
}

const TOP: usize = 40;

#[test]
fn nothing_is_declared_that_this_tree_never_writes() {
    let root = root();
    let counted = counted(&root).expect("the words in the tree");
    let declared = Declared::read(&root.join(WORDS)).expect(WORDS);
    let Ok(every) = declared.every();

    let unwritten: Vec<String> = every
        .iter()
        .filter(|(word, _)| !counted.words.contains_key(*word))
        .map(|(word, under)| format!("  {word} under [{under}]\n"))
        .collect();

    assert!(
        unwritten.is_empty(),
        "{WORDS} declares {} word(s) this tree does not write:\n{}",
        unwritten.len(),
        unwritten.concat()
    );
}
