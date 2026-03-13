use proc_macro::TokenStream;
use std::fmt::Display;
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum MatchPattern {
    Named(String),
    Wildcard,
}

impl Display for MatchPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MatchPattern::Named(name) => write!(f, "{}", name),
            MatchPattern::Wildcard => write!(f, "_"),
        }
    }
}

impl PartialOrd for MatchPattern {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MatchPattern {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (MatchPattern::Wildcard, MatchPattern::Wildcard) => std::cmp::Ordering::Equal,
            (MatchPattern::Wildcard, MatchPattern::Named(_)) => std::cmp::Ordering::Greater,
            (MatchPattern::Named(_), MatchPattern::Wildcard) => std::cmp::Ordering::Less,
            (MatchPattern::Named(a), MatchPattern::Named(b)) => a.cmp(b),
        }
    }
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

        // Collect patterns with their MatchPattern representation and span for sorting
        type PatternInfo<'a> = (MatchPattern, &'a dyn quote::ToTokens);
        let match_patterns: Vec<PatternInfo> = node
            .arms
            .iter()
            .filter_map(|a| {
                let pattern: Option<PatternInfo> = match &a.pat {
                    syn::Pat::TupleStruct(i) => {
                        let name = path_to_string(&&i.path);
                        Some((MatchPattern::Named(name), &i.path as &dyn quote::ToTokens))
                    }
                    syn::Pat::Path(i) => {
                        let name = path_to_string(&&i.path);
                        Some((MatchPattern::Named(name), &i.path as &dyn quote::ToTokens))
                    }
                    syn::Pat::Struct(i) => {
                        let name = path_to_string(&&i.path);
                        Some((MatchPattern::Named(name), &i.path as &dyn quote::ToTokens))
                    }
                    syn::Pat::Ident(i) => Some((
                        MatchPattern::Named(i.ident.to_string()),
                        &i.ident as &dyn quote::ToTokens,
                    )),
                    syn::Pat::Wild(w) => Some((MatchPattern::Wildcard, w as &dyn quote::ToTokens)),
                    x => {
                        if self.result.is_none() {
                            self.result = Some(Error::new_spanned(x, "unsupported by #[sorted]"));
                        }
                        None
                    }
                };
                pattern
            })
            .collect();

        let mut sorted_patterns = match_patterns.clone();
        sorted_patterns.sort_by_key(|(pattern, _)| pattern.clone());

        for ((expected_pattern, expected_span), (actual_pattern, _)) in
            sorted_patterns.iter().zip(&match_patterns)
        {
            if expected_pattern != actual_pattern {
                self.result = Some(Error::new_spanned(
                    *expected_span,
                    format!(
                        "{} should sort before {}",
                        expected_pattern,
                        actual_pattern
                    ),
                ));
                break;
            }
        }

        visit_mut::visit_expr_match_mut(self, node);
    }
}

fn path_to_string(path: &&syn::Path) -> String {
    path.segments
        .iter()
        .map(|ps| ps.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}
