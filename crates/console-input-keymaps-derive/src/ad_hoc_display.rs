use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Error;
use syn::Expr;
use syn::Index;
use syn::Token;
use syn::Type;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::punctuated::Punctuated;

pub(crate) fn expand(input: proc_macro::TokenStream) -> Result<TokenStream, Error> {
  let asked = syn::parse::<Input>(input)?;
  let msg = &asked.msg;
  let types = asked.arguments.iter().map(|argument| &argument.ty);
  let values = asked.arguments.iter().map(|argument| &argument.value);

  let (derive, fmt) = match asked.arguments.first() {
    None => (
      quote! {
          #[derive(Copy, Clone)]
      },
      quote! {
          f.write_str(#msg)
      },
    ),
    Some(_) => {
      let indices = (0u32..).zip(&asked.arguments).map(|(position, _)| Index {
        index: position,
        span: Span::call_site(),
      });

      (
        quote! {},
        quote! {
            write!(f, #msg #(, self.#indices)*)
        },
      )
    }
  };

  Ok(quote! {
      {
          #derive
          struct AdHoc(#(#types),*);
          impl std::fmt::Display for AdHoc {
              fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                  #fmt
              }
          }
          AdHoc(#(#values),*)
      }
  })
}

struct Input {
  msg: Expr,
  arguments: Punctuated<Argument, Token![,]>,
}

struct Argument {
  value: Expr,
  ty: Type,
}

impl Parse for Input {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let msg: Expr = input.parse()?;

    let arguments = match ::core::primitive::bool::then_some(input.is_empty(), ()) {
      ::core::option::Option::Some(()) => Punctuated::new(),
      ::core::option::Option::None => {
        let _comma: Token![,] = input.parse()?;

        Punctuated::parse_terminated(input)?
      }
    };

    Ok(Input { msg, arguments })
  }
}

impl Parse for Argument {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let value: Expr = input.parse()?;
    let _arrow: Token![=>] = input.parse()?;
    let ty: Type = input.parse()?;

    Ok(Argument { value, ty })
  }
}
