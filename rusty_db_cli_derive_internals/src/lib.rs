use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(TryFrom)]
pub fn try_from_variant(args: TokenStream) -> TokenStream {
    let input = parse_macro_input!(args as DeriveInput);

    let mut try_from_fc = TokenStream2::new();
    let enum_name = &input.ident;

    if let syn::Data::Enum(data_enum) = &input.data {
        for variant in &data_enum.variants {
            let variant_name = &variant.ident;

            if variant.fields.is_empty() {
                continue;
            }
            let first_variant = variant.fields.iter().next().unwrap();

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

#[proc_macro_derive(WithType)]
pub fn typed_for_enum(args: TokenStream) -> TokenStream {
    let input = parse_macro_input!(args as DeriveInput);

    let mut try_from_fc = TokenStream2::new();
    let enum_name = &input.ident;

    if let syn::Data::Enum(data_enum) = &input.data {
        let info = data_enum.variants.iter().map(|variant| {
            let variant_name = &variant.ident;
            quote! {
                #enum_name::#variant_name(val) => val.get_type_info(),
            }
        });

        try_from_fc.extend(quote! {
            impl Typed for #enum_name {
                fn get_type_info(&self) -> TypeInfo {
                    match self {
                        #(#info)*
                    }
                }
            }
        });
    } else {
        return syn::Error::new_spanned(input, "Expected an enum")
            .to_compile_error()
            .into();
    }

    TokenStream::from(try_from_fc)
}
