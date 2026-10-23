//! `#[derive(State)]` for Rapidkoi apps (contract 4, PRP-036).
//!
//! A window's state crosses to Rapidkoi as a CBOR map of named fields, so
//! Rapidkoi can read the ones an app puts in its address. The derive writes
//! that encoding for a struct with named fields, and one typed link field
//! per field (`Counter::count`), which [`rapidkoi::Next::link`] accepts only
//! for simple types: a list or a map can't go in an address.
//!
//! Use it through `rapidkoi::State`, never directly.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

/// Encodes the struct as a map of its field names, decodes it back (a
/// missing field from `Default`, an unknown one skipped), and gives each
/// field a link field of its type.
#[proc_macro_derive(State)]
pub fn derive_state(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => f.named.iter().collect::<Vec<_>>(),
            _ => return error(name, "State needs a struct with named fields"),
        },
        _ => return error(name, "State needs a struct with named fields"),
    };
    let idents: Vec<_> = fields.iter().map(|f| f.ident.clone().expect("named")).collect();
    let types: Vec<_> = fields.iter().map(|f| f.ty.clone()).collect();
    let keys: Vec<String> = idents.iter().map(|i| i.to_string().trim_start_matches("r#").to_string()).collect();
    let count = idents.len() as u64;
    let vis = &input.vis;
    quote! {
        impl ::rapidkoi::State for #name {
            fn encode_state(&self, e: &mut ::rapidkoi::__minicbor::Encoder<::rapidkoi::__Vec<u8>>) {
                let _ = e.map(#count);
                #(
                    let _ = e.str(#keys);
                    let _ = e.encode(&self.#idents);
                )*
            }

            fn decode_state(bytes: &[u8]) -> ::core::option::Option<Self> {
                let mut d = ::rapidkoi::__minicbor::Decoder::new(bytes);
                let n = d.map().ok()??;
                let mut s = <Self as ::core::default::Default>::default();
                for _ in 0..n {
                    match d.str().ok()? {
                        #( #keys => s.#idents = d.decode().ok()?, )*
                        _ => d.skip().ok()?,
                    }
                }
                ::core::option::Option::Some(s)
            }
        }

        #[allow(non_upper_case_globals)]
        impl #name {
            #(
                #[doc = concat!("`", #keys, "` as a link field: for `Next::link`.")]
                #vis const #idents: ::rapidkoi::Field<#name, #types> = ::rapidkoi::Field::new(#keys);
            )*
        }
    }
    .into()
}

fn error(name: &syn::Ident, message: &str) -> TokenStream {
    syn::Error::new(name.span(), message).to_compile_error().into()
}
