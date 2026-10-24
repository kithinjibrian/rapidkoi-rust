//! Derives for Rapidkoi apps: `#[derive(State)]` (PRP-036; nested,
//! PRP-039; `kinds`, contract 7) and `#[derive(Message)]` (PRP-039).
//!
//! A window's state crosses to Rapidkoi as a CBOR map of named fields, so
//! Rapidkoi can read the ones an app puts in its address. `State` writes
//! that encoding for a struct with named fields, as a whole state and as a
//! field inside another (so a path like `editor.task` finds its way), and
//! one typed link field per field (`Counter::count`), which
//! `rapidkoi::Next::link` accepts only for simple types.
//!
//! `Message` writes an enum as the messages a screen sends: a unit variant
//! as its name, one with fields as `[name, fields…]`.
//!
//! Use them through `rapidkoi::State` and `rapidkoi::Message`, never directly.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

/// Encodes the struct as a map of its field names, decodes it back (a
/// missing field from `Default`, an unknown one skipped), as a whole state
/// and as a nested field, and gives each field a link field of its type.
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
    let cbor = quote! { ::rapidkoi::__minicbor };
    quote! {
        impl ::rapidkoi::State for #name {
            fn encode_state(&self, e: &mut #cbor::Encoder<::rapidkoi::__Vec<u8>>) {
                let _ = e.encode(self);
            }

            fn decode_state(bytes: &[u8]) -> ::core::option::Option<Self> {
                #cbor::decode(bytes).ok()
            }

            // Each field's kind, picked by its type (contract 7, `rapidkoi::__kinds`).
            fn kinds() -> ::rapidkoi::__Vec<(::rapidkoi::__String, &'static str)> {
                #[allow(unused_imports)]
                use ::rapidkoi::__kinds::{NestedKind as _, NoKind as _, SimpleKind as _};
                let mut out = ::rapidkoi::__Vec::new();
                #( (&&&::rapidkoi::__kinds::Probe::<#types>::new()).kinds_into(#keys, &mut out); )*
                out
            }
        }

        // Inside another state, as a map of its names too (PRP-039).
        impl #cbor::Encode<()> for #name {
            fn encode<W: #cbor::encode::Write>(
                &self,
                e: &mut #cbor::Encoder<W>,
                _: &mut (),
            ) -> ::core::result::Result<(), #cbor::encode::Error<W::Error>> {
                e.map(#count)?;
                #(
                    e.str(#keys)?;
                    e.encode(&self.#idents)?;
                )*
                ::core::result::Result::Ok(())
            }
        }

        impl<'b> #cbor::Decode<'b, ()> for #name {
            fn decode(
                d: &mut #cbor::Decoder<'b>,
                _: &mut (),
            ) -> ::core::result::Result<Self, #cbor::decode::Error> {
                let n = d.map()?.ok_or_else(|| #cbor::decode::Error::message("a state is a map"))?;
                let mut s = <Self as ::core::default::Default>::default();
                for _ in 0..n {
                    match d.str()? {
                        #( #keys => s.#idents = d.decode()?, )*
                        _ => d.skip()?,
                    }
                }
                ::core::result::Result::Ok(s)
            }
        }

        #[allow(non_upper_case_globals)]
        impl #name {
            #(
                #[doc = concat!("`", #keys, "` as a link field: for `Next::link`, or `.then` from a parent's.")]
                #vis const #idents: ::rapidkoi::Field<#name, #types> = ::rapidkoi::Field::new(#keys);
            )*
        }
    }
    .into()
}

/// Writes an enum as messages: a unit variant as its name, one with fields
/// as `[name, fields…]` (each field a `Message` too), and reads it back.
#[proc_macro_derive(Message)]
pub fn derive_message(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let variants = match &input.data {
        Data::Enum(e) => e.variants.iter().collect::<Vec<_>>(),
        _ => return error(name, "Message needs an enum"),
    };
    let mut writes = Vec::new();
    let mut reads_unit = Vec::new();
    let mut reads_fields = Vec::new();
    for v in variants {
        let vname = &v.ident;
        let key = vname.to_string();
        match &v.fields {
            Fields::Unit => {
                writes.push(quote! { #name::#vname => { let _ = e.str(#key); } });
                reads_unit.push(quote! { #key => ::core::option::Option::Some(#name::#vname), });
            }
            Fields::Unnamed(f) => {
                let n = f.unnamed.len();
                let binds: Vec<_> = (0..n).map(|i| quote::format_ident!("f{}", i)).collect();
                let len = (n + 1) as u64;
                writes.push(quote! {
                    #name::#vname(#(#binds),*) => {
                        let _ = e.array(#len);
                        let _ = e.str(#key);
                        #( ::rapidkoi::Message::write(#binds, e); )*
                    }
                });
                let reads: Vec<_> = (0..n).map(|_| quote! { ::rapidkoi::Message::read(d)? }).collect();
                reads_fields.push(quote! {
                    (#key, #len) => ::core::option::Option::Some(#name::#vname(#(#reads),*)),
                });
            }
            Fields::Named(f) => {
                let idents: Vec<_> = f.named.iter().map(|x| x.ident.clone().expect("named")).collect();
                let len = (idents.len() + 1) as u64;
                writes.push(quote! {
                    #name::#vname { #(#idents),* } => {
                        let _ = e.array(#len);
                        let _ = e.str(#key);
                        #( ::rapidkoi::Message::write(#idents, e); )*
                    }
                });
                reads_fields.push(quote! {
                    (#key, #len) => ::core::option::Option::Some(#name::#vname { #(#idents: ::rapidkoi::Message::read(d)?),* }),
                });
            }
        }
    }
    let cbor = quote! { ::rapidkoi::__minicbor };
    quote! {
        impl ::rapidkoi::Message for #name {
            fn write(&self, e: &mut #cbor::Encoder<::rapidkoi::__Vec<u8>>) {
                match self {
                    #(#writes)*
                }
            }

            fn read(d: &mut #cbor::Decoder) -> ::core::option::Option<Self> {
                match d.datatype().ok()? {
                    #cbor::data::Type::String => match d.str().ok()? {
                        #(#reads_unit)*
                        _ => ::core::option::Option::None,
                    },
                    #cbor::data::Type::Array => {
                        let n = d.array().ok()??;
                        let key = d.str().ok()?;
                        match (key, n) {
                            #(#reads_fields)*
                            _ => ::core::option::Option::None,
                        }
                    }
                    _ => ::core::option::Option::None,
                }
            }
        }
    }
    .into()
}

fn error(name: &syn::Ident, message: &str) -> TokenStream {
    syn::Error::new(name.span(), message).to_compile_error().into()
}
