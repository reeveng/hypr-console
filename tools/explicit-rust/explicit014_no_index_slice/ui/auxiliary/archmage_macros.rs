// no-prefer-dynamic
// compile-flags: --emit=link
#![crate_type = "proc-macro"]

extern crate proc_macro;
use proc_macro::{Delimiter, Group, TokenStream, TokenTree};

// Model the generated signature and retained body spans of the AV1 target wrapper.
#[proc_macro_attribute]
pub fn rite(_: TokenStream, item: TokenStream) -> TokenStream {
    let Some(TokenTree::Group(body)) = item.into_iter().last() else {
        return TokenStream::new();
    };
    let mut function: TokenStream = "fn columns() -> [usize; 8]".parse().unwrap();
    function.extend([TokenTree::Group(Group::new(Delimiter::Brace, body.stream()))]);
    function
}
