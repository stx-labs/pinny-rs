use crate::config::Config;

use proc_macro::{Span, TokenStream};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::collections::HashSet;
use syn::{
    parse::{Parse, Parser},
    punctuated::Punctuated,
    spanned::Spanned,
    token::Comma,
    Attribute, Error, Ident, ItemFn, Meta, MetaNameValue, PathArguments,
    PathSegment, Result,
};

/// Representation of a comma separated tag label list
pub type TagLabels = Punctuated<Ident, Comma>;

/// implementation for procedural macro #[`pinny::tag`(..)]
///
/// This also help to convert from `proc_macro::TokenStream` to `proc_macro2::TokenStream`
pub fn macro_impl(attrs: TokenStream, item: TokenStream) -> TokenStream {
    match Config::get_instance() {
        Ok(config) => {
            resolve_tag(attrs.into(), item.into(), &config.allowed_tags)
                .unwrap_or_else(Error::into_compile_error)
                .into()
        }
        Err(error) => Error::new(Span::call_site().into(), error.to_string())
            .into_compile_error()
            .into(),
    }
}

/// A tag macro used like this:
///
/// #[tag(tag1, tag2)]
/// #[test]
/// fn `test_hello()` {
///   assert!(true);
/// }
///
/// produce the following params:
/// - `args`: `tag1, tag2`
/// - `item`: `#[test] fn test_hello() { assert!(true); }`
pub fn resolve_tag(
    args: TokenStream2,
    item: TokenStream2,
    allowed_labels: &Vec<String>,
) -> Result<TokenStream2> {
    let mut tags = parse_tag_labels(args)?;
    let fn_item = ItemFn::parse.parse2(item)?;

    validate_no_tag_attr(&fn_item)?;
    validate_tag_labels(&tags, allowed_labels)?;

    let ItemFn {
        attrs: mut fn_attrs,
        vis,
        sig: mut fn_sign,
        block: fn_block,
    } = fn_item;

    // Implement tags list delimeter for anti-clash or strict test filtering
    // the new test path become like this: <test_path>::original_test_name::t::tags_path::t

    // start tags delimiter: inserting an initial artificial module
    let tags_delimiter = "t";
    let first_tag = Ident::new(tags_delimiter, Span::call_site().into());
    tags.insert(0, first_tag.clone());

    // end tags delimiter: renaming the test function
    let original_test_name = fn_sign.ident;
    let new_test_name = Ident::new(tags_delimiter, original_test_name.span());
    fn_sign.ident = new_test_name.clone();

    disambiguate_any_test_attr(&mut fn_attrs);

    //
    let mut result = quote! {
      #(#fn_attrs)*
      pub #fn_sign {
        #fn_block
      }
    };

    //
    for tag in tags.iter().rev() {
        result = quote! {
          pub mod #tag {
            use super::*;
            #result
          }
        };
    }

    let mut tags_path: TokenStream2 = quote! { #first_tag };
    for next_tag in tags.iter().skip(1) {
        tags_path = quote! { #tags_path::#next_tag };
    }

    // Wrap everything in a module named after the test.
    // Allowing to still reference to the original test name symbol in code.
    // Importing the standard prelude because of the rewrite done
    // for `#[test]` to #[self::test] to disambiguate between std rust
    // and custom test attribute in `disambiguate_any_test_attr(..)`.
    // Not importing `std::prelude::v1::test` directly,
    // because that would conflict with potential user imports.
    result = quote! {
      use std::prelude::v1::*;
      #[allow(unused_imports)]
      #vis use #original_test_name::#tags_path::#new_test_name as #original_test_name;
      #[doc(hidden)]
      pub mod #original_test_name {
        use super::*;
        #result
      }
    };
    Ok(result)
}

/// No further tag on the function must exists.
///
/// Return error in case more then one tag attribute is used.
pub fn validate_no_tag_attr(func: &ItemFn) -> Result<()> {
    let tag_count = func.attrs.iter().filter(|attr| is_tag_attr(attr)).count();
    if tag_count > 0 {
        return Err(Error::new_spanned(
            &func.sig,
            "Only one #[pinny::tag] per function is allowed.".to_string(),
        ));
    }
    Ok(())
}

/// Check if labels declared in tag attribute are:
/// - uniques (no duplication)
/// - allowed (by configuration)
///
/// Return error in case the a tag is not valid.
pub fn validate_tag_labels(
    tags: &TagLabels,
    allowed_labels: &Vec<String>,
) -> Result<()> {
    let mut uniques = HashSet::new();

    for each_tag in tags {
        let each_tag_str = each_tag.to_string();

        // Check for duplicated tag
        if !uniques.insert(each_tag_str.clone()) {
            return Err(Error::new_spanned(
                tags,
                format!("Duplicated tag '{each_tag_str}'."),
            ));
        }

        // Check for not allowed tag
        if !allowed_labels.contains(&each_tag_str) {
            return Err(Error::new_spanned(
                tags,
                format!(
                    "Invalid tag '{each_tag_str}'. Allowed tags are: {allowed_labels:?}."),
            ));
        }
    }

    Ok(())
}

/// Parse a list of tag labels e.g. (`tag1, tag2`).
///
/// Return error in case the list is empty.
pub fn parse_tag_labels(args: TokenStream2) -> Result<TagLabels> {
    let tags = TagLabels::parse_terminated.parse2(args)?;
    if tags.is_empty() {
        Err(Error::new_spanned(
            &tags,
            "at least one tag is required: #[pinny::tag(<tags...>)]",
        ))
    } else {
        Ok(tags)
    }
}

/// Check if an attribute is a tag macro type
pub fn is_tag_attr(attr: &Attribute) -> bool {
    let path = match &attr.meta {
        Meta::Path(path) => path, //path-only attribute #[tag]
        Meta::List(list) => &list.path, //with args attrites #[tag(..)] or #[pinny::tag(..)]
        Meta::NameValue(_) => return false,
    };

    let segments: Vec<_> = path.segments.iter().map(|s| &s.ident).collect();
    match segments.as_slice() {
        [ident] if *ident == "tag" => true,
        [first, second] if *first == "pinny" && *second == "tag" => true,
        _ => false,
    }
}

/// Rewrite any `#[test]` attribute to use `#[self::test]` syntax.
///
/// This is necessary in order to properly support both
/// std rust and custom `#[test]` attributes, by the fact
/// rust's prelude contains such an attribute we could risk ambiguities
pub fn disambiguate_any_test_attr(attrs: &mut [Attribute]) {
    for attr in attrs.iter_mut() {
        let span = attr.meta.span();
        let path = match &mut attr.meta {
            Meta::NameValue(MetaNameValue { path, .. }) | Meta::Path(path) => {
                path
            }
            Meta::List(list) => &mut list.path,
        };

        if path.leading_colon.is_none()
            && path.segments.len() == 1
            && path.segments[0].ident == "test"
        {
            let segment = PathSegment {
                ident: Ident::new("self", span),
                arguments: PathArguments::None,
            };
            path.segments.insert(0, segment);
        }
    }
}
