use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::visit_mut::{self, VisitMut};
use syn::{parse_macro_input, Error, ExprMatch, Item, Meta};

#[proc_macro_attribute]
pub fn sorted(args: TokenStream, input: TokenStream) -> TokenStream {
    let macro_tokens: TokenStream2 = args.into();
    let ast = parse_macro_input!(input as Item);
    sorted_impl(&macro_tokens, ast).unwrap_or_else(|err| err.to_compile_error().into())
}

fn sorted_impl(macro_tokens: &TokenStream2, item: Item) -> syn::Result<TokenStream> {
    let Item::Enum(item_enum) = &item else {
        return std::result::Result::Err(Error::new_spanned(
            macro_tokens,
            "expected enum or match expression",
        ));
    };

    let mut sorted: Vec<&syn::Ident> = item_enum.variants.iter().map(|v| &v.ident).collect();
    sorted.sort();

    for (variant, expected) in item_enum.variants.iter().zip(&sorted) {
        if variant.ident != **expected {
            return std::result::Result::Err(Error::new_spanned(
                expected,
                format!("{} should sort before {}", expected, variant.ident),
            ));
        }
    }

    std::result::Result::Ok(quote! { #item }.into())
}

#[proc_macro_attribute]
pub fn check(args: TokenStream, input: TokenStream) -> TokenStream {
    let macro_tokens: TokenStream2 = args.into();
    let ast = parse_macro_input!(input as Item);
    check_impl(&macro_tokens, ast).unwrap_or_else(|err| err.to_compile_error().into())
}

fn check_impl(macro_tokens: &TokenStream2, mut item: Item) -> syn::Result<TokenStream> {
    let Item::Fn(ref mut func) = &mut item else {
        return std::result::Result::Err(Error::new_spanned(macro_tokens, "expected function"));
    };

    let mut ms = MatchSort::new();
    ms.visit_item_fn_mut(func);
    let output = quote! {#item};

    if let std::option::Option::Some(err) = ms.result {
        let error_tokens = err.to_compile_error();
        return Ok(quote! {
            #error_tokens
            #output
        }
        .into());
    }
    Ok(output.into())
}

struct MatchSort {
    result: Option<syn::Error>,
}

impl MatchSort {
    fn new() -> Self {
        MatchSort { result: None }
    }
}

impl VisitMut for MatchSort {
    fn visit_expr_match_mut(&mut self, node: &mut ExprMatch) {
        // Do we have a sorted attribute on the match?
        let sorted = node.attrs.iter().position(|a| {
            if let Meta::Path(p) = &a.meta {
                p.is_ident("sorted")
            } else {
                false
            }
        });
        let Some(idx) = sorted else {
            // continue with normal processing
            visit_mut::visit_expr_match_mut(self, node);
            return;
        };

        // Remove the attribute from the match
        node.attrs.remove(idx);

        // Get the idents from the match arms. Skip other types of Arms.
        let match_idents: Vec<_> = node
            .arms
            .iter()
            .filter_map(|a| match &a.pat {
                syn::Pat::TupleStruct(i) => {
                    eprintln!("path: {:?}", i.path);
                    Some(&i.path)
                },
                syn::Pat::Path(i) => Some(&i.path),
                syn::Pat::Struct(i) => Some(&i.path),
                x => {
                    if self.result.is_none() {
                        self.result = Some(Error::new_spanned(x, "unsupported by #[sorted]"));
                    }
                    None
                },
            })
            .collect();
        let mut sorted_idents = match_idents.clone();
        sorted_idents.sort_by_key(path_to_string);
        for (variant, expected) in sorted_idents.iter().zip(&match_idents) {
            if variant != expected {
                self.result = Some(Error::new_spanned(
                    expected,
                    format!("{} should sort before {}", path_to_string(expected), path_to_string(variant)),
                ));
            }
        }
        visit_mut::visit_expr_match_mut(self, node);
    }
}

fn path_to_string(path: &&syn::Path) -> String {
    path.segments.iter().map(|ps| ps.ident.to_string()).collect::<Vec<_>>().join("::")
}