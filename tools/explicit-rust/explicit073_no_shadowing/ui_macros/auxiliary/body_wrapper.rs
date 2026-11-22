// no-prefer-dynamic
// compile-flags: --emit=link

#![crate_type = "proc-macro"]

extern crate proc_macro;

use proc_macro::{Delimiter, Group, TokenStream, TokenTree};

#[proc_macro_attribute]
pub fn keep_body(_: TokenStream, item: TokenStream) -> TokenStream {
    let Some(TokenTree::Group(body)) = item.into_iter().last() else {
        return TokenStream::new();
    };
    let mut wrapped: TokenStream = "fn wrapped(input: u8) -> u8".parse().unwrap();
    wrapped.extend([TokenTree::Group(Group::new(Delimiter::Brace, body.stream()))]);
    wrapped
}
