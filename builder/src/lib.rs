use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::Fields::Named;
use syn::{parse_macro_input, parse_quote, DeriveInput, Error, Field, Ident, Type};

#[proc_macro_derive(Builder)]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    // println!("{:#?}", ast.ident);
    let name = &ast.ident;
    let builder_name = format_ident!("{}Builder", ast.ident);
    // println!("{}", builder_name);

    let builder_fields = match build_fields(&ast) {
        Ok(value) => value,
        Err(err) => { return err.to_compile_error().into();}
    };

    let field_idents: Vec<&Option<Ident>> = builder_fields.iter().map(|f| &f.ident).collect();

    let generated = quote! {
        pub struct #builder_name {
            #(#builder_fields),*
        }

       impl #name {
            pub fn builder() -> #builder_name {
                #builder_name {
                    #(#field_idents: None),*
                }
            }
        }

    };

    //eprintln!("TOKENS: {}", generated);
    generated.into()
}

fn build_fields(ast: &DeriveInput) -> syn::Result<Vec<Field>> {
    if let syn::Data::Struct(syn::DataStruct { fields, .. }) = &ast.data {
        match fields {
            Named(fields_named) => {
                let mut builder_fields = Vec::new();
                fields_named.named.pairs().for_each(|field| {
                    let mut f: Field = (*field.value()).clone();
                    make_field_optional(&mut f);
                    builder_fields.push(f);
                });
                return Ok(builder_fields);
            },
            _ => {
                return Err(Error::new_spanned(
                    fields,
                    "Builder only supports structs with named fields",
                ));
            }
        }
    } else {
        return Err(Error::new_spanned(
            ast,
            "Builder can only be derived for structs",
        ));
    }
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
