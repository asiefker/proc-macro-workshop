use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::Fields::Named;
use syn::Type::Path;
use syn::__private::TokenStream2;
use syn::{parse_macro_input, parse_quote, DeriveInput, Error, Field, Ident, Type};

#[proc_macro_derive(Builder)]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    // println!("{:#?}", ast.ident);
    let name = &ast.ident;
    let builder_name = format_ident!("{}Builder", ast.ident);
    // println!("{}", builder_name);

    let fields = match extract_fields(&ast) {
        Ok(value) => value,
        Err(err) => {
            return err.to_compile_error().into();
        }
    };
    let builder_field_metas: Vec<BuilderFieldMeta> =
        fields.iter().map(to_builder_field).collect();

    let builder_field_defs: Vec<TokenStream2> = builder_field_metas
        .iter()
        .map(|f| f.generate_builder_field())
        .collect();
    let field_idents: Vec<TokenStream2> = builder_field_metas
        .iter()
        .map(|f| f.generate_builder_field_init())
        .collect();
    let builder_methods: Vec<TokenStream2> = builder_field_metas
        .iter()
        .map(|f| f.generate_setter())
        .collect();
    let build_method = make_build_method(name, &builder_field_metas);

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
            #(#builder_field_defs),*
        }

        impl #builder_name {
            #(#builder_methods)*
            #build_method
        }
       impl #name {
            pub fn builder() -> #builder_name {
                #builder_name {
                    #(#field_idents),*
                }
            }
        }

    };

    //eprintln!("TOKENS: {}", generated);
    generated.into()
}

fn extract_fields(ast: &DeriveInput) -> syn::Result<Vec<Field>> {
    if let syn::Data::Struct(syn::DataStruct { fields, .. }) = &ast.data {
        match fields {
            Named(fields_named) => {
                let builder_fields = fields_named
                    .named
                    .pairs()
                    .map(|field| (*field.value()).clone())
                    .collect();
                Ok(builder_fields)
            }
            _ => Err(Error::new_spanned(
                fields,
                "Builder only supports structs with named fields",
            )),
        }
    } else {
        Err(Error::new_spanned(
            ast,
            "Builder can only be derived for structs",
        ))
    }
}

fn is_option(field: &Field) -> Option<&Type> {
    if let Path(type_path) = &field.ty {
        if let Some(last_segment) = type_path.path.segments.last() {
            if last_segment.ident == "Option" {
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

fn make_build_method(name: &Ident, fields: &[BuilderFieldMeta]) -> TokenStream2 {
    let initializers: Vec<TokenStream2> = fields.iter().map(|f| f.generate_initializer()).collect();
    quote! {
        pub fn build(&mut self) -> Result<#name, Box<dyn std::error::Error + 'static>> {
            Ok(#name {
                #(#initializers),*
            })
        }
    }
}

fn to_builder_field(field: &Field) -> BuilderFieldMeta {
    let ident = field.ident.as_ref().unwrap();
    let optional = is_option(field);
    let inner_ty = optional.unwrap_or(&field.ty);
    BuilderFieldMeta {
        optional: optional.is_some(),
        ident: ident.clone(),
        inner_ty: inner_ty.clone(),
    }
}

enum _MultiValue {
    No,
    Yes(Ident),
}

struct BuilderFieldMeta {
    optional: bool,
    ident: Ident,
    inner_ty: Type,
}

impl BuilderFieldMeta {
    fn generate_builder_field_init(&self) -> TokenStream2 {
        let ident = &self.ident;
        quote!(#ident: None)
    }

    fn generate_initializer(&self) -> TokenStream2 {
        let ident = &self.ident;
        if self.optional {
            quote!(#ident: self.#ident.take())
        } else {
            quote!(#ident: self.#ident.take().ok_or(BuilderError(format!("Missing field: {}", stringify!(#ident))))?)
        }
    }

    fn generate_builder_field(&self) -> TokenStream2 {
        let ident = &self.ident;
        let ty = &self.inner_ty;
        parse_quote! {
           #ident: Option<#ty>
        }
    }

    fn generate_setter(&self) -> TokenStream2 {
        let ident = &self.ident;
        let typ = &self.inner_ty;
        quote! {
            pub fn #ident(&mut self, #ident: #typ) -> &mut Self {
                self.#ident = Some(#ident);
                self
            }
        }
    }
}
