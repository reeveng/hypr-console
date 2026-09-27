//! The gate: a word written far out of English's proportion is a word this tree
//! decided on, and `words.conf` is where the deciding is written down.
//!
//! What fails here is never one call site. It is a word that has spread far
//! enough to be a habit and was never argued for, and the fix is one of two
//! things: put it under a heading in `words.conf` with the other words it
//! belongs with, or find the four places that reached for it and give each of
//! them the word that says what it does there.
//!
//! The table is printed either way, because the number no one is failing is the
//! one worth reading: run `just words` and the words at the top are the tree's
//! own accent, in order.

use std::error::Error;
use std::path::{Path, PathBuf};

use console_core_never::Never;
use console_core_number_conversion::index;
use console_vocabulary::counting::count;
use console_vocabulary::declared::{Declared, WORDS};
use console_vocabulary::elsewhere::Elsewhere;
use console_vocabulary::norm::{Norm, beside};
use console_vocabulary::{
    Further, Known, LEANING_ON, Measured, TOO_FAR, Vocabulary, known, measure, outside, undeclared,
};

fn root() -> Result<PathBuf, std::io::Error> {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize()
}

fn table(every: &[&Measured], vocabulary: &Vocabulary<'_>) -> Result<String, Never> {
    Ok(every
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
                    let Ok(spelled) = name.description();

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
        .collect())
}

#[test]
fn a_word_written_far_out_of_proportion_is_a_word_this_tree_declared() -> Result<(), Box<dyn Error>> {
    let root = root()?;
    let Ok(english) = beside();
    let norm = Norm::read(&english)?;
    let counted = count(&root)?;
    let declared = Declared::read(&root.join(WORDS))?;
    let elsewhere = Elsewhere::read(&root)?;
    let vocabulary = Vocabulary { declared: &declared, elsewhere: &elsewhere, norm: &norm };
    let Ok(measured) = measure(&counted, &norm);
    let Ok(undeclared) = undeclared(&measured, &vocabulary);
    let Ok(top) = index(TOP);
    let unheard: Vec<&Measured> =
        measured.iter().filter(|word| word.times.is_none()).take(top).collect();
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

    let leant_on: Vec<&Measured> = leant_on.into_iter().take(top).collect();

    let Ok(unheard_table) = table(&unheard, &vocabulary);
    let Ok(leant_on_table) = table(&leant_on, &vocabulary);
    let Ok(undeclared_table) = table(&undeclared, &vocabulary);

    println!("words English has no rate for, by how often this tree writes them:\n{unheard_table}");
    println!(
        "ordinary words this tree leans on more than {}x as hard as English does:\n{leant_on_table}",
        LEANING_ON.0,
    );

    assert!(
        undeclared.is_empty(),
        "{} word(s) are written more than {}x as often as English writes them and are \
         under no heading in {WORDS}:\n{}",
        undeclared.len(),
        TOO_FAR.0,
        undeclared_table
    );
    Ok(())
}

const TOP: u32 = 40;

#[test]
fn a_word_a_heading_gave_one_place_is_not_written_in_another() -> Result<(), Box<dyn Error>> {
    let root = root()?;
    let counted = count(&root)?;
    let declared = Declared::read(&root.join(WORDS))?;
    let Ok(outside) = outside(&counted, &declared);
    let said: String = outside
        .iter()
        .map(|word| {
            format!(
                "  {} belongs to {} and is written in {}\n",
                word.word,
                word.only.join(", "),
                word.written_in.join(", ")
            )
        })
        .collect();

    assert!(
        outside.is_empty(),
        "{} word(s) are declared in {WORDS} for one place and written outside it:\n{said}",
        outside.len()
    );
    Ok(())
}

#[test]
fn nothing_is_declared_that_this_tree_never_writes() -> Result<(), Box<dyn Error>> {
    let root = root()?;
    let counted = count(&root)?;
    let declared = Declared::read(&root.join(WORDS))?;
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
    Ok(())
}
