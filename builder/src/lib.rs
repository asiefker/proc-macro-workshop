use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::Fields::Named;
use syn::{parse_macro_input, parse_quote, DeriveInput, Error, Field, Ident, Type};
use syn::__private::TokenStream2;
use syn::Type::Path;

#[proc_macro_derive(Builder)]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    // println!("{:#?}", ast.ident);
    let name = &ast.ident;
    let builder_name = format_ident!("{}Builder", ast.ident);
    // println!("{}", builder_name);

    let fields = match extract_fields(&ast) {
        Ok(value) => value,
        Err(err) => { return err.to_compile_error().into();}
    };
    let builder_fields = make_field_option(&fields);
    let field_idents: Vec<&Option<Ident>> = fields.iter().map(|f| &f.ident).collect();
    let builder_methods: Vec<TokenStream2> = fields.iter().map(|f| make_builder_method(f)).collect();
    let build_method = make_build_method(name, &fields);
    let generated = quote! {
        #[derive(Debug)]
        pub struct BuilderError(String);

        impl std::fmt::Display for BuilderError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl std::error::Error for BuilderError {}

        pub struct #builder_name {
            #(#builder_fields),*
        }

        impl #builder_name {
            #(#builder_methods)*
            #build_method
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
fn make_field_option(fields: &[Field]) -> Vec<Field>{
    fields.iter().map(|f|{
        let mut nf = f.clone();
        make_field_optional(&mut nf);
        nf
    }).collect()
}

fn extract_fields(ast: &DeriveInput) -> syn::Result<Vec<Field>> {
    if let syn::Data::Struct(syn::DataStruct { fields, .. }) = &ast.data {
        match fields {
            Named(fields_named) => {
                let builder_fields = fields_named.named.pairs().map(|field| {
                    (*field.value()).clone()
                }).collect();
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
    if is_option(field).is_some() {
        return;
    }
    let inner_type = &field.ty; // This is the 'T' in Option<T>

    // Use parse_quote! to construct the new Type::Path for Option<T>
    // The `#inner_type` fragment is replaced by the actual type
    let option_type: Type = parse_quote! {
        Option<#inner_type>
    };

    // Update the field's type
    field.ty = option_type;
}

fn is_option(field: &Field) -> Option<&Type> {
    if let Path(type_path) = &field.ty {
        if let Some(last_segment) = type_path.path.segments.last() {
            if last_segment.ident.to_string() == "Option" {
                // It's an Option, now find the T
                if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner_type)) = args.args.first() {
                        return Some(inner_type); // Return the inner type T
                    }
                }
            }
        }
    }
    None
}

fn make_builder_method(field: &Field) -> TokenStream2 {
    let ident = &field.ident;
    let typ =  is_option(field).unwrap_or( &field.ty);
    quote!{
        pub fn #ident(&mut self, #ident: #typ) -> &mut Self {
            self.#ident = Some(#ident);
            self
        }
    }
}

fn make_build_method(name: &Ident, fields: &[Field]) -> TokenStream2 {
    let x1:(Vec<&Field>, Vec<&Field>) = fields.iter().partition(|x| is_option(x).is_some());
    let (optional, required) = x1;
    let required_idents: Vec<&Option<Ident>> = required.iter().map(|f| &f.ident).collect();
    let optional_idents: Vec<&Option<Ident>> = optional.iter().map(|f| &f.ident).collect();
    quote! {
        pub fn build(&mut self) -> Result<#name, Box<dyn std::error::Error + 'static>> {
            Ok(#name {
                #(#required_idents: self.#required_idents.take().ok_or(BuilderError(format!("Missing field: {}", stringify!(#required_idents))))?),*,
                #(#optional_idents: self.#optional_idents.take()),*
            })
        }
    }
}
