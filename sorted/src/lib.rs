use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Error, Item};

#[proc_macro_attribute]
pub fn sorted(args: TokenStream, input: TokenStream) -> TokenStream {
    let macro_tokens: TokenStream2 = args.into();
    let ast = parse_macro_input!(input as Item);
    sorted_impl(&macro_tokens, ast).unwrap_or_else(|err| err.to_compile_error().into())
}

fn sorted_impl(macro_tokens: &TokenStream2, item: Item) -> syn::Result<TokenStream> {
    let Item::Enum(item_enum) = &item else {
        return Err(Error::new_spanned(
            macro_tokens,
            "expected enum or match expression",
        ));
    };

    let mut sorted: Vec<&syn::Ident> = item_enum.variants.iter().map(|v| &v.ident).collect();
    sorted.sort();

    for (variant, expected) in item_enum.variants.iter().zip(&sorted) {
        if variant.ident != **expected {
            return Err(Error::new_spanned(
                expected,
                format!("{} should sort before {}", expected, variant.ident),
            ));
        }
    }

    Ok(quote! { #item }.into())
}
