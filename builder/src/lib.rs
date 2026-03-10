use proc_macro::{TokenStream};
use quote::{format_ident, quote};
use syn::{parse_macro_input, DeriveInput};
use syn::Fields::Named;

#[proc_macro_derive(Builder)]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    // println!("{:#?}", ast.ident);
    let name = &ast.ident;
    let builder_name = format_ident!("{}Builder", ast.ident);
    println!("{}", builder_name);

    // if let syn::Data::Struct(syn::DataStruct { fields, .. }) = &ast.data {
    //         match fields {
    //             Named(fields_named) => {
    //                 fields_named.named.pairs().for_each(|field| {
    //                     println!("{:#?} {:#?}", field.value().ty, field.value().ident);
    //                 })
    //             },
    //             _ => panic!("Builder only works for named fields.")
    //         }
    // } else {
    //     println!("Not Data match")
    // }
    let generated = quote! {
        pub struct #builder_name {
            executable: Option<String>,
        }

       impl #name {
            pub fn builder() -> #builder_name {
                #builder_name {
                    executable: None,
                }
            }
        }

    };
    //
    // eprintln!("TOKENS: {}", generated);
    generated.into()
}
