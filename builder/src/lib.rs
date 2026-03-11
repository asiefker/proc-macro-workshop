use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::Fields::Named;
use syn::Type::Path;
use syn::{parse_macro_input, parse_quote, DeriveInput, Error, Field, Ident, Type};

#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    derive_impl(ast).unwrap_or_else(|err| err.to_compile_error().into())
}

fn derive_impl(ast: DeriveInput) -> syn::Result<TokenStream> {
    let name = &ast.ident;
    let builder_name = format_ident!("{}Builder", ast.ident);

    let fields = extract_fields(&ast)?;
    let builder_field_metas = fields
        .iter()
        .map(to_builder_field)
        .collect::<syn::Result<Vec<BuilderFieldMeta>>>()?;

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
    Ok(generated.into())
}

fn extract_fields(ast: &DeriveInput) -> syn::Result<Vec<Field>> {
    if let syn::Data::Struct(syn::DataStruct { fields, .. }) = &ast.data {
        match fields {
            Named(fields_named) => Ok(fields_named.named.iter().cloned().collect()),
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

/// If field is of expected type, return Some<T>, else None.
/// For example `Option<String>` returns `Some(String)`.
fn is_container<'a>(field: &'a Field, expected_type: &str) -> Option<&'a Type> {
    if let Path(type_path) = &field.ty {
        if let Some(last_segment) = type_path.path.segments.last() {
            if last_segment.ident == expected_type {
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
        pub fn build(&mut self) -> std::result::Result<#name, std::boxed::Box<dyn std::error::Error + 'static>> {
            std::result::Result::Ok(#name {
                #(#initializers),*
            })
        }
    }
}

fn to_builder_field(field: &Field) -> syn::Result<BuilderFieldMeta> {
    let ident = field
        .ident
        .as_ref()
        .ok_or_else(|| Error::new_spanned(field, "field must have a name"))?;
    let optional = is_container(field, "Option");
    let inner_ty = optional.unwrap_or(&field.ty);
    let each = parse_builder_each_attr(field)?;

    Ok(BuilderFieldMeta {
        optional: optional.is_some(),
        ident: ident.clone(),
        inner_ty: inner_ty.clone(),
        each,
    })
}

fn parse_builder_each_attr(field: &Field) -> syn::Result<Option<(Ident, Type)>> {
    let Some(attr) = field.attrs.iter().find(|a| a.path().is_ident("builder")) else {
        return Ok(None);
    };

    let t = is_container(field, "Vec").ok_or(Error::new_spanned(
        field,
        "each attribute can only be used on Vec fields",
    ))?;

    attr.parse_args_with(|input: syn::parse::ParseStream| {
        let key: syn::Ident = input.parse()?;
        if key != "each" {
            return Err(syn::Error::new_spanned(
                &attr.meta,
                "expected `builder(each = \"...\")`",
            ));
        }
        input.parse::<syn::Token![=]>()?;
        let name: syn::LitStr = input.parse()?;
        Ok(Some((format_ident!("{}", name.value()), t.clone())))
    })
}

struct BuilderFieldMeta {
    optional: bool,
    ident: Ident,
    inner_ty: Type,
    each: Option<(Ident, Type)>,
}

impl BuilderFieldMeta {
    fn generate_builder_field_init(&self) -> TokenStream2 {
        let ident = &self.ident;
        quote!(#ident: std::option::Option::None)
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
           #ident: std::option::Option<#ty>
        }
    }

    fn generate_setter(&self) -> TokenStream2 {
        let ident = &self.ident;
        let typ = &self.inner_ty;
        let default_setter = quote! {
            pub fn #ident(&mut self, #ident: #typ) -> &mut Self {
                self.#ident = std::option::Option::Some(#ident);
                self
            }
        };
        match &self.each {
            None => default_setter,
            Some((name, v_type)) => {
                let v_typ = &v_type;
                if name.eq(ident) {
                    // replace
                    quote! {
                        pub fn #ident(&mut self, #ident: #v_typ) -> &mut Self {
                            self.#ident.get_or_insert_with(std::vec::Vec::new).push(#name);
                            self
                        }
                    }
                } else {
                    // Both
                    quote! {
                        pub fn #name(&mut self, #name: #v_typ) -> &mut Self {
                            self.#ident.get_or_insert_with(std::vec::Vec::new).push(#name);
                            self
                        }
                        #default_setter
                    }
                }
            }
        }
    }
}
