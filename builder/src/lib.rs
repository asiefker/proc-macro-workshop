use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::Fields::Named;
use syn::{parse_macro_input, parse_quote, DeriveInput, Error, Field, Ident, Type};
use syn::__private::TokenStream2;

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
    let build_method = make_build_method(name, &field_idents);
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
    let inner_type = &field.ty; // This is the 'T' in Option<T>

    // Use parse_quote! to construct the new Type::Path for Option<T>
    // The `#inner_type` fragment is replaced by the actual type
    // TODO Skip if already optional...
    let option_type: Type = parse_quote! {
        Option<#inner_type>
    };

    // Update the field's type
    field.ty = option_type;
}

fn make_builder_method(field: &Field) -> TokenStream2 {
    let ident = &field.ident;
    let typ = &field.ty;
    quote!{
        pub fn #ident(&mut self, #ident: #typ) -> &mut Self {
            self.#ident = Some(#ident);
            self
        }
    }
}

fn make_build_method(name: &Ident, field_idents: &[&Option<Ident>]) -> TokenStream2 {
    let non_check: Vec<TokenStream2> = field_idents.iter().map(|i| make_none_check(i)).collect();
    quote! {
        pub fn build(self) -> Result<#name, Box<dyn std::error::Error + 'static>> {
            #(#non_check)*
            Ok(#name {
                #(#field_idents: self.#field_idents.unwrap()),*
            })
        }
    }
}

fn make_none_check(ident: &Option<Ident>) -> TokenStream2 {
    quote! {
        if self.#ident.is_none() {
            return Err(Box::new(BuilderError(format!("Missing field: {}", stringify!(#ident)))));
        }
    }
}