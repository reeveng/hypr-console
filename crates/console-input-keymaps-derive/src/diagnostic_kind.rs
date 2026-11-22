use proc_macro2::TokenStream;
use quote::quote;
use std::mem;
use syn::Attribute;
use syn::Error;
use syn::ItemEnum;
use syn::Meta;
use syn::Path;
use syn::Variant;
use syn::parse_quote;

pub(crate) fn expand(input: proc_macro::TokenStream) -> Result<TokenStream, Error> {
  let mut item = syn::parse::<ItemEnum>(input)?;
  let mut cases = vec![];

  for variant in &mut item.variants {
    let severity = severity_of(variant)?;
    let name = &variant.ident;

    cases.push(quote! {
        Self::#name => Severity::#severity,
    });
    variant.attrs.push(parse_quote!(
        #[doc = ""]
    ));
    variant.attrs.push(parse_quote!(
        #[doc = "# Severity"]
    ));
    variant.attrs.push(parse_quote!(
        #[doc = ""]
    ));
    variant.attrs.push(parse_quote!(
        #[doc = concat!("[`", stringify!(#severity), "`](Severity::", stringify!(#severity), ")")]
    ));
    variant.attrs.push(parse_quote!(
        #[doc = ""]
    ));
  }

  Ok(quote! {
      #item

      impl DiagnosticKind {
          fn severity_(self) -> Severity {
              match self {
                  #(#cases)*
              }
          }
      }
  })
}

fn severity_of(variant: &mut Variant) -> Result<Path, Error> {
  let held = mem::take(&mut variant.attrs);
  let (said, kept): (Vec<Attribute>, Vec<Attribute>) =
    held
      .into_iter()
      .partition(|attribute| match &attribute.meta {
        Meta::NameValue(written) => written.path.is_ident("severity"),
        Meta::Path(_) | Meta::List(_) => false,
      });

  variant.attrs = kept;

  match said.last().map(|attribute| &attribute.meta) {
    Some(Meta::NameValue(written)) => {
      let value = &written.value;

      syn::parse2::<Path>(quote!(#value))
    }
    Some(Meta::Path(_) | Meta::List(_)) | None => Err(Error::new_spanned(
      &variant.ident,
      format!("no severity attribute found for variant {}", variant.ident),
    )),
  }
}
