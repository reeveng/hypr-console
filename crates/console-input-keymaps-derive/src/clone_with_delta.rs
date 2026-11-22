use console_core_never::Never;
use proc_macro2::Ident;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Data;
use syn::DataEnum;
use syn::DeriveInput;
use syn::Error;
use syn::Field;
use syn::Fields;
use syn::GenericParam;
use syn::Generics;
use syn::Index;
use syn::Token;
use syn::WhereClause;
use syn::parse_quote;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;

pub(crate) fn derive(input: proc_macro::TokenStream) -> Result<TokenStream, Error> {
  let mut item = syn::parse::<DeriveInput>(input)?;

  let body = match &item.data {
    Data::Struct(held) => fields_moved(&held.fields),
    Data::Enum(held) => variants_moved(held),
    Data::Union(_) => {
      return Err(Error::new_spanned(
        &item.ident,
        "a union's spans cannot be moved: which of its fields is the one held is not written in it",
      ));
    }
  };

  let Ok(moved) = body;
  let Ok(()) = bound(&mut item.generics);
  let ident = &item.ident;
  let (impl_generics, type_generics, where_clause) = item.generics.split_for_impl();

  Ok(quote! {
      impl #impl_generics crate::xkb::clone_with_delta::CloneWithDelta for #ident #type_generics #where_clause {
          fn clone_with_delta(&self, delta: crate::xkb::span::SpanUnit) -> Self {
              #moved
          }
      }
  })
}

fn bound(generics: &mut Generics) -> Result<(), Never> {
  let Generics {
    params,
    where_clause: held,
    ..
  } = generics;
  let clause = held.get_or_insert_with(|| WhereClause {
    where_token: <Token![where]>::default(),
    predicates: Punctuated::new(),
  });

  for param in params.iter() {
    match param {
      GenericParam::Type(ty) => {
        let ident = &ty.ident;

        clause.predicates.push(parse_quote! {
            #ident: crate::xkb::clone_with_delta::CloneWithDelta
        });
      }
      GenericParam::Lifetime(_) | GenericParam::Const(_) => {}
    }
  }

  Ok(())
}

fn member(field: &Field, position: u32) -> Result<TokenStream, Never> {
  Ok(match &field.ident {
    Some(id) => quote! { #id },
    None => {
      let index = Index {
        index: position,
        span: Span::call_site(),
      };

      quote! { #index }
    }
  })
}

fn binding(field: &Field, position: u32) -> Result<TokenStream, Never> {
  Ok(match &field.ident {
    Some(id) => quote! { #id },
    None => {
      let named = Ident::new(&format!("_{position}"), field.ty.span());

      quote! { #named }
    }
  })
}

fn fields_moved(fields: &Fields) -> Result<TokenStream, Never> {
  let moved = fields.iter().zip(0u32..).map(|(field, position)| {
        let Ok(name) = member(field, position);
        let ty = &field.ty;

        quote! {
            #name: <#ty as crate::xkb::clone_with_delta::CloneWithDelta>::clone_with_delta(&self.#name, delta),
        }
    });

  Ok(quote! {
      Self {
          #(#moved)*
      }
  })
}

fn variants_moved(held: &DataEnum) -> Result<TokenStream, Never> {
  let arms = held.variants.iter().map(|variant| {
        let name = &variant.ident;
        let keys = variant.fields.iter().zip(0u32..).map(|(field, position)| {
            let Ok(key) = member(field, position);
            let Ok(bound_as) = binding(field, position);

            quote! { #key: #bound_as, }
        });
        let fields = variant.fields.iter().zip(0u32..).map(|(field, position)| {
            let Ok(key) = member(field, position);
            let Ok(bound_as) = binding(field, position);
            let ty = &field.ty;

            quote! {
                #key: <#ty as crate::xkb::clone_with_delta::CloneWithDelta>::clone_with_delta(#bound_as, delta),
            }
        });

        quote! {
            Self::#name { #(#keys)* } => Self::#name { #(#fields)* },
        }
    });

  Ok(quote! {
      match self {
          #(#arms)*
      }
  })
}
