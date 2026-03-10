use proc_macro::{TokenStream};
use quote::{format_ident, quote};
use syn::{parse_macro_input, parse_quote, DeriveInput, Field, Type};
use syn::Fields::Named;

#[proc_macro_derive(Builder)]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    // println!("{:#?}", ast.ident);
    let name = &ast.ident;
    let builder_name = format_ident!("{}Builder", ast.ident);
    println!("{}", builder_name);

    let mut field_names = Vec::new();
    if let syn::Data::Struct(syn::DataStruct { fields, .. }) = &ast.data {
            match fields {
                Named(fields_named) => {
                    fields_named.named.pairs().for_each(|field| {
                        let mut f: Field = (*field.value()).clone();
                        make_field_optional(&mut f);
                        field_names.push(f);

                    })
                },
                _ => panic!("Builder only works for named fields.")
            }
    } else {
        println!("Not Data match")
    }
    let generated = quote! {
        pub struct #builder_name {
            #(#field_names),*
        }

       impl #name {
            pub fn builder() -> #builder_name {
                #builder_name {
                    executable: None,
                    args: None,
                    env: None,
                    current_dir: None,
                }
            }
        }

    };
    //
    eprintln!("TOKENS: {}", generated);
    generated.into()
}

fn make_field_optional(field: &mut Field) {
    let inner_type = &field.ty; // This is the 'T' in Option<T>

    // Use parse_quote! to construct the new Type::Path for Option<T>
    // The `#inner_type` fragment is replaced by the actual type
    let option_type: Type = parse_quote! {
        Option<#inner_type>
    };

    // Update the field's type
    field.ty = option_type;
}