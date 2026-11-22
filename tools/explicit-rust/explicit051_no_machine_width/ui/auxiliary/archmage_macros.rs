// no-prefer-dynamic
// compile-flags: --emit=link
#![crate_type = "proc-macro"]

extern crate proc_macro;
use proc_macro::{Delimiter, Group, TokenStream, TokenTree};

// A namesake target wrapper can observe the input tokens.
#[proc_macro_attribute]
pub fn arcane(_: TokenStream, input: TokenStream) -> TokenStream {
    let mut items: Vec<_> = input.into_iter().collect();
    let Some(TokenTree::Group(body)) = items.pop() else { return TokenStream::new() };
    let mut wrapped: TokenStream = format!("let _original = {:?};", body.stream().to_string()).parse().unwrap();
    wrapped.extend(body.stream());
    items.push(TokenTree::Group(Group::new(Delimiter::Brace, wrapped)));
    items.into_iter().collect()
}
