//! The words an enum spells, derived rather than written again.
//!
//! A `match` over every variant with a string on the right of each arm is the
//! most copied shape in this tree. `word`, `tag`, `name`, `written`, `said`,
//! `says`, `needs`, `tab`, `key`, `flag` -- every crate that learned to say
//! something out loud wrote one, and the next one will write another. They are
//! the same function with different nouns in it: one arm per variant, a literal
//! on the right, wrapped in `Ok` because EXPLICIT002 asks an infallible
//! function to say so. The list of variants it is written out of is a list the
//! compiler is already holding.
//!
//! So the words go on the variants and the function is derived:
//!
//! ```text
//! #[derive(Clone, Copy, Words)]
//! pub enum Layer {
//!     #[words(tag = "us", name = "latin", written = "ABC")]
//!     Latin,
//!     #[words(tag = "th", name = "thai", written = "ไทย")]
//!     Thai,
//! }
//! ```
//!
//! which is `tag`, `name` and `written`, each a `const fn` over every variant,
//! and `from_tag`, `from_name` and `from_written`, the way back from a word to
//! the variant that says it, or `None` for a word none of them says. Beside
//! them is `VARIANTS`, every variant in the order it is declared, which is the
//! list a crate wrote out by hand whenever it had to show every word at once --
//! a menu, a usage line, a test over each -- and which drifted from the enum
//! the day a variant was added to one and not the other. strum calls it the
//! same thing. A list in some other order, the order a menu shows, is a
//! different list and keeps its own name. Each is as
//! public as the enum that carries the words -- a word is as
//! reachable as the thing it is a word for. What a reader gains is that the
//! three words for one variant are on one line instead of a page apart in three
//! matches, which is where they used to drift: a variant added to the enum and
//! to two of the three functions was a compile error only because the match had
//! to be exhaustive, and that is the whole of what was keeping them together.
//!
//! **Why not a trait.** A trait names the accessor once for the whole tree, and
//! these are not one accessor. `tag` is the layout xkb knows a keyboard by and
//! `written` is what someone reads on the key that changes it; `said` is what
//! goes on a wire and `needs` is a device a profile wants. Each name is the
//! crate's own sentence about what the word is for, and a trait would spend all
//! of them on one `word()` and leave the distinction in a comment. A trait also
//! holds one word per type, and an enum here says up to three.
//!
//! **What it refuses.** Anything but an enum, because a variant is what carries
//! a word. A variant with a field, because what it spells would depend on the
//! value. A variant that says nothing, or that says a word no other variant
//! says, or says one twice -- the accessor is total, so a word missing from one
//! variant is an arm someone would have to invent, and the derive will not
//! invent it.
//!
//! **What it needs.** The signature it writes names `console_core_never::Never`,
//! so a crate deriving this depends on `console-core-never` as well. Every
//! crate that spelled one of these by hand already did.

use std::collections::{BTreeMap, BTreeSet};

use proc_macro::TokenStream;
use proc_macro2::TokenStream as Written;
use quote::{ToTokens, format_ident, quote};
use syn::punctuated::Punctuated;
use syn::{Data, DeriveInput, Error, Fields, Ident, LitStr, MetaNameValue, Token, Variant};

#[cfg_attr(dylint_lib = "explicit002_infallible_result", allow(explicit002_infallible_result, reason = "a derive answers the compiler in token streams, and a `Result` does not cross that boundary any more than it crosses an `extern` one"))]
#[proc_macro_derive(Words, attributes(words))]
pub fn words(asked: TokenStream) -> TokenStream {
    let read = syn::parse::<DeriveInput>(asked);
    let written = read.and_then(|enumeration| expand_derive(&enumeration));

    match written {
        Ok(spelled) => proc_macro::TokenStream::from(spelled),
        Err(fault) => proc_macro::TokenStream::from(fault.to_compile_error()),
    }
}

struct Spelled {
    variant: Ident,
    words: Vec<(Ident, LitStr)>,
}

struct Word {
    name: Ident,
    over: Vec<(Ident, LitStr)>,
}

fn expand_derive(asked: &DeriveInput) -> Result<Written, Error> {
    let held = match &asked.data {
        Data::Enum(held) => Ok(&held.variants),
        Data::Struct(_) | Data::Union(_) => Err(Error::new_spanned(
            &asked.ident,
            "only an enum spells words: a variant is what carries one",
        )),
    };

    let variants = held?;
    let mut spelled = Vec::new();

    for variant in variants {
        let said = variant_words(variant)?;

        spelled.push(said);
    }

    let every = gather_words(&asked.ident, &spelled)?;
    let holding = &asked.ident;
    let declared = spelled.iter().map(|said| {
        let variant = &said.variant;

        quote! { #holding::#variant }
    });
    let visibility = &asked.vis;
    let (outside, inside, clause) = asked.generics.split_for_impl();

    let each = every.into_iter().map(|word| {
        let name = word.name;
        let reverse = format_ident!("from_{}", name);
        let arms = word.over.iter().map(|(variant, said)| quote! { #holding::#variant => #said, });
        let pairs = word.over.iter().map(|(variant, said)| quote! { (#said, #holding::#variant) });

        quote! {
            #visibility const fn #name(self) -> ::core::result::Result<&'static str, ::console_core_never::Never> {
                ::core::result::Result::Ok(match self {
                    #(#arms)*
                })
            }

            #[allow(dead_code, reason = "the way back from a word is written for every word an enum says, and an enum only read aloud never takes it")]
            #visibility fn #reverse(said: &str) -> ::core::result::Result<::core::option::Option<Self>, ::console_core_never::Never> {
                ::core::result::Result::Ok(
                    [#(#pairs),*].into_iter().find(|(word, _)| *word == said).map(|(_, variant)| variant),
                )
            }
        }
    });

    Ok(quote! {
        impl #outside #holding #inside #clause {
            #[allow(dead_code, reason = "every variant is listed for every enum that says words, and an enum only ever read one word at a time never walks it")]
            #visibility const VARIANTS: &'static [Self] = &[#(#declared),*];

            #(#each)*
        }
    })
}

fn variant_words(variant: &Variant) -> Result<Spelled, Error> {
    let unit = match &variant.fields {
        Fields::Unit => Ok(()),
        Fields::Named(_) | Fields::Unnamed(_) => Err(Error::new_spanned(
            variant,
            "a variant that carries a value has no one word: what it spells would depend on what is in it",
        )),
    };

    unit?;

    let mut words = Vec::new();

    for attribute in &variant.attrs {
        let mine = attribute.path().is_ident("words");

        match mine {
            true => {
                let read = attribute
                    .parse_args_with(Punctuated::<MetaNameValue, Token![,]>::parse_terminated);

                let said = read?;

                for one in said {
                    let named = match one.path.get_ident() {
                        Some(name) => Ok(name.clone()),
                        None => Err(Error::new_spanned(
                            &one.path,
                            "a word is said as `name = \"the word\"`",
                        )),
                    };

                    let name = named?;
                    let word = syn::parse2::<LitStr>(one.value.to_token_stream())?;

                    words.push((name, word));
                }
            }
            false => {}
        }
    }

    for (name, _) in &words {
        let once = match words.iter().filter(|(called, _)| called == name).count() {
            1 => Ok(()),
            _ => Err(Error::new_spanned(name, format!("`{name}` is said twice by this variant"))),
        };

        once?;
    }

    Ok(Spelled { variant: variant.ident.clone(), words })
}

fn gather_words(holding: &Ident, spelled: &[Spelled]) -> Result<Vec<Word>, Error> {
    let found = match spelled.first() {
        Some(first) => Ok(first),
        None => Err(Error::new_spanned(holding, "an enum with no variants has nothing to spell")),
    };

    let first = found?;

    let any = match first.words.is_empty() {
        true => Err(Error::new_spanned(
            &first.variant,
            "a variant says what it is called: `#[words(word = \"the word\")]`",
        )),
        false => Ok(()),
    };

    any?;

    let spoken: Vec<BTreeMap<String, &LitStr>> = spelled
        .iter()
        .map(|one| one.words.iter().map(|(called, word)| (called.to_string(), word)).collect())
        .collect();

    let asked: BTreeSet<String> = first.words.iter().map(|(called, _)| called.to_string()).collect();

    for one in spelled {
        for (name, _) in &one.words {
            let known = asked.contains(&name.to_string());

            let shared = match known {
                true => Ok(()),
                false => Err(Error::new_spanned(
                    name,
                    format!("`{}` says nothing called `{name}`, and every variant says the same words", first.variant),
                )),
            };

            shared?;
        }
    }

    let mut every = Vec::new();

    for (name, _) in &first.words {
        let mut words = Vec::new();

        for (one, said) in spelled.iter().zip(&spoken) {
            let found = said.get(&name.to_string());

            let word = match found {
                Some(word) => Ok(*word),
                None => Err(Error::new_spanned(
                    &one.variant,
                    format!("every variant says `{name}` and this one does not"),
                )),
            };

            let said = word?;

            words.push((one.variant.clone(), said.clone()));
        }

        every.push(Word { name: name.clone(), over: words });
    }

    Ok(every)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand(from: &str) -> Result<String, Error> {
        let read = syn::parse_str::<DeriveInput>(from)?;
        let written = expand_derive(&read)?;

        Ok(written.to_string())
    }

    #[test]
    fn one_function_is_written_for_each_word_over_every_variant() -> Result<(), Box<dyn std::error::Error>> {
        let written = expand(
            r#"
            pub enum Layer {
                #[words(tag = "us", written = "ABC")]
                Latin,
                #[words(tag = "th", written = "ไทย")]
                Thai,
            }
            "#,
        )?;

        assert!(written.contains("pub const fn tag"), "{written}");
        assert!(written.contains("pub const fn written"), "{written}");
        assert!(written.contains(r#"Layer :: Latin => "us""#), "{written}");
        assert!(written.contains(r#"Layer :: Thai => "th""#), "{written}");
        assert!(written.contains(r#"Layer :: Thai => "ไทย""#), "{written}");

        Ok(())
    }

    #[test]
    fn every_variant_stands_in_the_order_the_enum_declares_it() -> Result<(), Box<dyn std::error::Error>> {
        let written = expand(
            r#"
            pub enum Layer {
                #[words(tag = "us")]
                Latin,
                #[words(tag = "th")]
                Thai,
                #[words(tag = "el")]
                Greek,
            }
            "#,
        )?;

        assert!(written.contains("pub const VARIANTS"), "{written}");
        assert!(written.contains("[Layer :: Latin , Layer :: Thai , Layer :: Greek]"), "{written}");

        Ok(())
    }

    #[test]
    fn a_word_is_as_public_as_the_enum_that_says_it() -> Result<(), Box<dyn std::error::Error>> {
        let written = expand(
            r#"
            enum Note {
                #[words(word = "awake")]
                Awake,
            }
            "#,
        )?;

        assert!(written.contains("const fn word"), "{written}");
        assert!(!written.contains("pub const fn word"), "{written}");

        Ok(())
    }

    #[test]
    fn a_variant_that_says_nothing_the_others_say_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let fault = match expand(
            r#"
            pub enum Layer {
                #[words(tag = "us", written = "ABC")]
                Latin,
                #[words(tag = "th")]
                Thai,
            }
            "#,
        ) {
            Err(fault) => fault,
            Ok(written) => return Err(Box::from(format!("a variant that says one of the two words was spelled rather than refused: {written}"))),
        };

        assert!(fault.to_string().contains("written"), "{fault}");

        Ok(())
    }

    #[test]
    fn a_word_nothing_else_says_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let fault = match expand(
            r#"
            pub enum Layer {
                #[words(tag = "us")]
                Latin,
                #[words(tag = "th", flag = "thailand")]
                Thai,
            }
            "#,
        ) {
            Err(fault) => fault,
            Ok(written) => return Err(Box::from(format!("a word only one variant says was spelled rather than refused: {written}"))),
        };

        assert!(fault.to_string().contains("flag"), "{fault}");

        Ok(())
    }

    #[test]
    fn a_word_said_twice_by_one_variant_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let fault = match expand(
            r#"
            pub enum Layer {
                #[words(tag = "us", tag = "en")]
                Latin,
            }
            "#,
        ) {
            Err(fault) => fault,
            Ok(written) => return Err(Box::from(format!("one variant, one word, two answers was spelled rather than refused: {written}"))),
        };

        assert!(fault.to_string().contains("twice"), "{fault}");

        Ok(())
    }

    #[test]
    fn a_variant_carrying_a_value_has_no_one_word() -> Result<(), Box<dyn std::error::Error>> {
        let fault = match expand(
            r#"
            pub enum Expiry {
                #[words(said = "0")]
                Stays,
                #[words(said = "many")]
                Milliseconds(u32),
            }
            "#,
        ) {
            Err(fault) => fault,
            Ok(written) => return Err(Box::from(format!("a variant with a field in it was spelled rather than refused: {written}"))),
        };

        assert!(fault.to_string().contains("carries a value"), "{fault}");

        Ok(())
    }

    #[test]
    fn a_variant_that_says_nothing_at_all_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let fault = match expand(
            r#"
            pub enum Layer {
                Latin,
            }
            "#,
        ) {
            Err(fault) => fault,
            Ok(written) => return Err(Box::from(format!("an enum with no words on it was spelled rather than refused: {written}"))),
        };

        assert!(fault.to_string().contains("says what it is called"), "{fault}");

        Ok(())
    }

    #[test]
    fn only_an_enum_spells_words() -> Result<(), Box<dyn std::error::Error>> {
        let fault = match expand("pub struct Where { pub latitude: f64 }") {
            Err(fault) => fault,
            Ok(written) => return Err(Box::from(format!("a struct has no variants to carry a word was spelled rather than refused: {written}"))),
        };

        assert!(fault.to_string().contains("only an enum"), "{fault}");

        Ok(())
    }

    #[test]
    fn a_variant_may_carry_another_attribute_as_well() -> Result<(), Box<dyn std::error::Error>> {
        let written = expand(
            r#"
            pub enum Kind {
                #[words(said = "key")]
                #[serde(rename = "key")]
                Key,
            }
            "#,
        )?;

        assert!(written.contains(r#"Kind :: Key => "key""#), "{written}");

        Ok(())
    }
}
