use std::any::type_name;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use regex::Regex;
use syn::{parse_macro_input, DeriveInput, Fields};

fn type_of<T>(_: &T) -> &'static str {
    type_name::<T>()
}

#[proc_macro_derive(TryFrom)]
pub fn try_from_variant(args: TokenStream) -> TokenStream {
    let input = parse_macro_input!(args as DeriveInput);

    let mut try_from_fc = TokenStream2::new();
    let enum_name = &input.ident;

    if let syn::Data::Enum(data_enum) = &input.data {
        for variant in &data_enum.variants {
            let variant_name = &variant.ident;
            let first_variant = variant.fields.iter().next().unwrap();



            //let boxed_value = Regex::new("Box < (.) >")
            //    .unwrap()
            //    .captures(&variant_name_str);

            //let value = if let Some(boxed_value) = boxed_value {
            //    boxed_value.get(0).unwrap().as_str()
            //} else {
            //    variant_name_str.as_str()
            //};

            //try_from_fc.extend(quote! {
            //    fn hello() {
            //        dbg!(stringify!(#first_variant));
            //    }
            //});

            try_from_fc.extend(quote! {
                impl TryFrom<#enum_name> for #first_variant {
                    type Error = ();

                    fn try_from(value: #enum_name) -> Result<#first_variant, Self::Error> {
                        if let #enum_name::#variant_name(val) = value {
                            Ok(val)
                        } else {
                            Err(())
                        }
                    }
                }
            });
        }
    } else {
        return syn::Error::new_spanned(input, "Expected an enum")
            .to_compile_error()
            .into();
    }

    TokenStream::from(try_from_fc)
}
