use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as Tokens, TokenTree};
use quote::{quote, ToTokens};
use std::collections::HashSet;
use syn::{
    parse_macro_input, parse_quote, Attribute, Data, DeriveInput, Expr, Fields, Ident, LitBool,
    LitStr, Path, Result, Type, WherePredicate,
};

#[proc_macro_derive(UbfSerialize, attributes(ubf))]
pub fn derive_ubf_serialize(input: TokenStream) -> TokenStream {
    expand(parse_macro_input!(input as DeriveInput), false, true)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
#[proc_macro_derive(UbfDeserialize, attributes(ubf))]
pub fn derive_ubf_deserialize(input: TokenStream) -> TokenStream {
    expand(parse_macro_input!(input as DeriveInput), false, false)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
#[proc_macro_derive(ViewSerialize, attributes(view))]
pub fn derive_view_serialize(input: TokenStream) -> TokenStream {
    expand(parse_macro_input!(input as DeriveInput), true, true)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
#[proc_macro_derive(ViewDeserialize, attributes(view))]
pub fn derive_view_deserialize(input: TokenStream) -> TokenStream {
    expand(parse_macro_input!(input as DeriveInput), true, false)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[derive(Default)]
struct FieldAttr {
    field: Option<Expr>,
    occ: Option<Expr>,
    size: Option<Expr>,
    nested: bool,
    ptr: bool,
    view: bool,
    flatten: bool,
    group: bool,
    default: bool,
    skip: bool,
}
struct Field {
    ident: Ident,
    ty: Type,
    attr: FieldAttr,
}

fn boolean(meta: syn::meta::ParseNestedMeta<'_>) -> Result<bool> {
    if meta.input.peek(syn::Token![=]) {
        Ok(meta.value()?.parse::<LitBool>()?.value)
    } else {
        Ok(true)
    }
}

fn field_attr(attrs: &[Attribute], is_view: bool) -> Result<FieldAttr> {
    let namespace = if is_view { "view" } else { "ubf" };
    let mut result = FieldAttr::default();
    let mut seen = HashSet::new();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident(namespace)) {
        attr.parse_nested_meta(|meta| {
            let ident = meta
                .path
                .get_ident()
                .ok_or_else(|| meta.error("expected a mapping attribute"))?
                .to_string();
            let key = if ident == "occurrence" { "occ" } else { &ident };
            if !seen.insert(key.to_string()) {
                return Err(meta.error("duplicate mapping attribute"));
            }
            match key {
                "field" => result.field = Some(meta.value()?.parse()?),
                "occ" => result.occ = Some(meta.value()?.parse()?),
                "skip" => result.skip = boolean(meta)?,
                "flatten" => result.flatten = boolean(meta)?,
                "nested" if !is_view => result.nested = boolean(meta)?,
                "ptr" if !is_view => result.ptr = boolean(meta)?,
                "view" if !is_view => result.view = boolean(meta)?,
                "group" if !is_view => result.group = boolean(meta)?,
                "default" if !is_view => result.default = boolean(meta)?,
                "size" if !is_view => result.size = Some(meta.value()?.parse()?),
                _ => return Err(meta.error("unsupported mapping attribute")),
            }
            Ok(())
        })?;
    }
    if result.nested && (result.ptr || result.view) {
        return Err(syn::Error::new(
            Span::call_site(),
            "nested selects BFLD_UBF and cannot be combined with ptr or view",
        ));
    }
    if result.group
        && (result.nested
            || result.ptr
            || result.view
            || result.flatten
            || result.field.is_some()
            || result.size.is_some()
            || result.default)
    {
        return Err(syn::Error::new(
            Span::call_site(),
            "group feeds from the parent buffer and takes only an optional occurrence",
        ));
    }
    if result.size.is_some() && ((!result.nested && !result.ptr) || result.view) {
        return Err(syn::Error::new(
            Span::call_site(),
            "size is only valid for nested UBF or UBF pointer mappings",
        ));
    }
    if result.skip && seen.len() > 1 {
        return Err(syn::Error::new(
            Span::call_site(),
            "skip cannot be combined with other field attributes",
        ));
    }
    if result.flatten
        && (result.field.is_some()
            || result.occ.is_some()
            || result.size.is_some()
            || result.nested
            || result.ptr
            || result.view
            || result.default)
    {
        return Err(syn::Error::new(
            Span::call_site(),
            "flatten cannot have a field, occurrence, storage mode, size, or default",
        ));
    }
    Ok(result)
}

fn fields(data: &Data, is_view: bool) -> Result<Vec<Field>> {
    let Data::Struct(data) = data else {
        return Err(syn::Error::new(
            Span::call_site(),
            "buffer mappings require a struct",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            &data.fields,
            "buffer mappings require named fields",
        ));
    };
    fields
        .named
        .iter()
        .map(|field| {
            Ok(Field {
                ident: field.ident.clone().unwrap(),
                ty: field.ty.clone(),
                attr: field_attr(&field.attrs, is_view)?,
            })
        })
        .collect()
}

fn contains_ident(tokens: Tokens, name: &Ident) -> bool {
    tokens.into_iter().any(|token| match token {
        TokenTree::Ident(ident) => ident == *name,
        TokenTree::Group(group) => contains_ident(group.stream(), name),
        _ => false,
    })
}

fn expand(input: DeriveInput, is_view: bool, serialize: bool) -> Result<Tokens> {
    let namespace = if is_view { "view" } else { "ubf" };
    let mut krate: Path = parse_quote!(::endurox_rs);
    let mut view_name: Option<LitStr> = None;
    let mut container_group = false;
    let mut seen = HashSet::new();
    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident(namespace))
    {
        attr.parse_nested_meta(|meta| {
            let key = meta
                .path
                .get_ident()
                .ok_or_else(|| meta.error("expected a container attribute"))?
                .to_string();
            if !seen.insert(key.clone()) {
                return Err(meta.error("duplicate container attribute"));
            }
            if key == "crate" {
                krate = meta.value()?.parse::<LitStr>()?.parse()?;
            } else if key == "name" && is_view {
                let name = meta.value()?.parse::<LitStr>()?;
                if name.value().is_empty() || name.value().contains('\0') {
                    return Err(meta.error("VIEW name must be nonempty and contain no NUL"));
                }
                view_name = Some(name);
            } else if key == "group" && !is_view {
                container_group = boolean(meta)?;
            } else {
                return Err(meta.error("unsupported container attribute"));
            }
            Ok(())
        })?;
    }
    if is_view && view_name.is_none() {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "missing #[view(name = \"COMPILED_VIEW\")]",
        ));
    }
    let mapped = fields(&input.data, is_view)?;
    let name = input.ident;
    let generics = input.generics;
    let type_params: Vec<Ident> = generics.type_params().map(|p| p.ident.clone()).collect();
    let mut statements = Vec::new();
    let mut predicates: Vec<WherePredicate> = Vec::new();
    // Parallel "group" body (only emitted with #[ubf(group)] on the struct):
    // the same fields addressed at `base + occ` in the parent buffer, plus a
    // tail-clear and the anchor field used to count a flat vector on read.
    let mut group_statements = Vec::new();
    let mut group_predicates: Vec<WherePredicate> = Vec::new();
    let mut group_checks = Vec::new();
    let mut clear_statements = Vec::new();
    let mut anchor: Option<(Tokens, Tokens)> = None;
    for field in mapped {
        let ident = field.ident;
        let ty = field.ty;
        let attr = field.attr;
        let label = LitStr::new(&format!("{name}.{ident}"), ident.span());
        let error = quote!(|error| #krate::UbfError::new(error.code, ::std::format!("{}: {}", #label, error.message)));
        let dependent = type_params
            .iter()
            .any(|p| contains_ident(ty.to_token_stream(), p))
            && !contains_ident(ty.to_token_stream(), &name)
            && !contains_ident(ty.to_token_stream(), &Ident::new("Self", Span::call_site()));

        if container_group && (attr.flatten || attr.group) {
            return Err(syn::Error::new_spanned(
                &ident,
                "a #[ubf(group)] struct cannot contain a flatten or a group field: \
                 that would need a second occurrence axis the flat buffer has not",
            ));
        }

        if attr.skip {
            if !serialize {
                statements.push(quote!(#ident: ::std::default::Default::default(),));
                if container_group {
                    group_statements.push(quote!(#ident: ::std::default::Default::default(),));
                }
                if dependent {
                    predicates.push(parse_quote!(#ty: ::std::default::Default));
                    if container_group {
                        group_predicates.push(parse_quote!(#ty: ::std::default::Default));
                    }
                }
            }
            continue;
        }
        let occ = attr
            .occ
            .as_ref()
            .map(|v| quote!(#v))
            .unwrap_or_else(|| quote!(0));
        let g_occ = quote!(#krate::ubf_group_field_check(true, base, #occ).map_err(#error)?);
        let size = attr
            .size
            .as_ref()
            .map(|v| quote!(#v))
            .unwrap_or_else(|| quote!(1024));
        let field_id = if attr.flatten || attr.group {
            quote!()
        } else if let Some(field_id) = &attr.field {
            quote!(#field_id)
        } else if is_view {
            let cname = LitStr::new(&ident.to_string(), ident.span());
            quote!(#cname)
        } else {
            return Err(syn::Error::new_spanned(
                &ident,
                "missing #[ubf(field = ...)]",
            ));
        };

        let marker = if attr.view {
            Some(if attr.ptr {
                quote!(#krate::PointerView)
            } else {
                quote!(#krate::EmbeddedView)
            })
        } else if attr.ptr {
            Some(quote!(#krate::PointerUbf))
        } else if attr.nested {
            Some(quote!(#krate::EmbeddedUbf))
        } else {
            None
        };

        let (bound, call) = if attr.group {
            if serialize {
                (
                    quote!(#krate::UbfGroupFieldSerialize),
                    quote!(#krate::UbfGroupFieldSerialize::ubf_group_write_field(&self.#ident, ubf, #occ, realloc)),
                )
            } else {
                (
                    quote!(#krate::UbfGroupFieldDeserialize),
                    quote!(<#ty as #krate::UbfGroupFieldDeserialize>::ubf_group_read_field(ubf, #occ)),
                )
            }
        } else if attr.flatten {
            match (is_view, serialize) {
                (false, true) => (
                    quote!(#krate::UbfSerialize),
                    quote!(#krate::UbfSerialize::ubf_serialize(&self.#ident, ubf, realloc)),
                ),
                (false, false) => (
                    quote!(#krate::UbfDeserialize),
                    quote!(<#ty as #krate::UbfDeserialize>::ubf_deserialize(ubf)),
                ),
                (true, true) => (
                    quote!(#krate::ViewSerialize),
                    quote!(#krate::ViewSerialize::view_serialize(&self.#ident, view)),
                ),
                (true, false) => (
                    quote!(#krate::ViewDeserialize),
                    quote!(<#ty as #krate::ViewDeserialize>::view_deserialize(view)),
                ),
            }
        } else if let Some(marker) = &marker {
            if serialize {
                (
                    quote!(#krate::UbfMappedSerialize<#marker>),
                    quote!(#krate::UbfMappedSerialize::<#marker>::ubf_write_mapped(&self.#ident, ubf, #field_id, #occ, #size, realloc)),
                )
            } else {
                (
                    quote!(#krate::UbfMappedDeserialize<#marker>),
                    quote!(<#ty as #krate::UbfMappedDeserialize<#marker>>::ubf_read_mapped(ubf, #field_id, #occ)),
                )
            }
        } else {
            match (is_view, serialize) {
                (false, true) => (
                    quote!(#krate::UbfFieldSerialize),
                    quote!(#krate::UbfFieldSerialize::ubf_write_field(&self.#ident, ubf, #field_id, #occ, realloc)),
                ),
                (false, false) => (
                    quote!(#krate::UbfFieldDeserialize),
                    quote!(<#ty as #krate::UbfFieldDeserialize>::ubf_read_field(ubf, #field_id, #occ)),
                ),
                (true, true) => (
                    quote!(#krate::ViewFieldSerialize),
                    quote!(#krate::ViewFieldSerialize::view_write_field(&self.#ident, view, #field_id, #occ)),
                ),
                (true, false) => (
                    quote!(#krate::ViewFieldDeserialize),
                    quote!(<#ty as #krate::ViewFieldDeserialize>::view_read_field(view, #field_id, #occ)),
                ),
            }
        };
        if dependent {
            predicates.push(syn::parse2(quote!(#ty: #bound))?);
        }
        if serialize {
            statements.push(quote!(#call.map_err(#error)?;));
        } else if attr.default {
            if dependent {
                predicates.push(parse_quote!(#ty: ::std::default::Default));
            }
            statements.push(quote!(#ident: {
                if #krate::ubf_mapping_present(ubf, #field_id, #occ).map_err(#error)? {
                    #call.map_err(#error)?
                } else { ::std::default::Default::default() }
            },));
        } else {
            statements.push(quote!(#ident: #call.map_err(#error)?,));
        }

        // Group container body: identical mapping addressed at `base + occ`.
        // Reached only for scalar and nested/ptr/view fields (flatten and group
        // fields were rejected above), so every field has a real field id.
        if container_group {
            group_checks.push(quote!(
                #krate::ubf_group_field_check(<#ty as #bound>::SINGLE, base, #occ).map_err(#error)?;
            ));
            let g_call = if let Some(marker) = &marker {
                if serialize {
                    quote!(#krate::UbfMappedSerialize::<#marker>::ubf_write_occurrence(&self.#ident, ubf, #field_id, #g_occ, #size, realloc))
                } else {
                    quote!(<#ty as #krate::UbfMappedDeserialize<#marker>>::ubf_read_mapped(ubf, #field_id, #g_occ))
                }
            } else if serialize {
                quote!(#krate::UbfFieldSerialize::ubf_write_field(&self.#ident, ubf, #field_id, #g_occ, realloc))
            } else {
                quote!(<#ty as #krate::UbfFieldDeserialize>::ubf_read_field(ubf, #field_id, #g_occ))
            };
            if dependent {
                group_predicates.push(syn::parse2(quote!(#ty: #bound))?);
            }
            if serialize {
                group_statements.push(quote!(#g_call.map_err(#error)?;));
                clear_statements.push(quote!(
                    #krate::ubf_group_clear(ubf, #field_id,
                        #krate::ubf_group_field_check(true, from, #occ).map_err(#error)?
                    ).map_err(#error)?;
                ));
            } else if attr.default {
                if dependent {
                    group_predicates.push(parse_quote!(#ty: ::std::default::Default));
                }
                group_statements.push(quote!(#ident: {
                    if #krate::ubf_mapping_present(ubf, #field_id, #g_occ).map_err(#error)? {
                        #g_call.map_err(#error)?
                    } else { ::std::default::Default::default() }
                },));
            } else {
                group_statements.push(quote!(#ident: #g_call.map_err(#error)?,));
            }
            if anchor.is_none() {
                anchor = Some((field_id.clone(), occ.clone()));
            }
        }
    }
    let mut normal_generics = generics.clone();
    normal_generics
        .make_where_clause()
        .predicates
        .extend(predicates);
    let (impl_generics, ty_generics, where_clause) = normal_generics.split_for_impl();
    let body = if serialize {
        quote!(#(#statements)* Ok(()))
    } else {
        quote!(Ok(Self { #(#statements)* }))
    };

    let mut group_generics = generics.clone();
    group_generics
        .make_where_clause()
        .predicates
        .extend(group_predicates);
    let (g_impl, g_ty, g_where) = group_generics.split_for_impl();
    let group_impl = if !container_group || is_view {
        quote!()
    } else if serialize {
        quote! {
            impl #g_impl #krate::UbfGroupSerialize for #name #g_ty #g_where {
                fn ubf_validate_group(base: i32) -> #krate::UbfResult<()> {
                    #krate::ubf_occurrence(base, 0)?;
                    #(#group_checks)* Ok(())
                }
                fn ubf_write_at(&self, ubf: &mut #krate::TypedUbf<'_>, base: i32, realloc: bool) -> #krate::UbfResult<()> {
                    <Self as #krate::UbfGroupSerialize>::ubf_validate_group(base)?;
                    #(#group_statements)* Ok(())
                }
                fn ubf_clear_from(ubf: &mut #krate::TypedUbf<'_>, from: i32) -> #krate::UbfResult<()> {
                    <Self as #krate::UbfGroupSerialize>::ubf_validate_group(from)?;
                    #(#clear_statements)* Ok(())
                }
            }
            impl #g_impl #krate::UbfGroupFieldSerialize for #name #g_ty #g_where {
                fn ubf_group_write_field(&self, ubf: &mut #krate::TypedUbf<'_>, base: i32, realloc: bool) -> #krate::UbfResult<()> {
                    #krate::UbfGroupSerialize::ubf_write_at(self, ubf, base, realloc)
                }
            }
        }
    } else {
        let (anchor, anchor_offset) = anchor.ok_or_else(|| {
            syn::Error::new_spanned(
                &name,
                "a #[ubf(group)] struct needs at least one field with an explicit \
                 field id to anchor its occurrence count",
            )
        })?;
        quote! {
            impl #g_impl #krate::UbfGroupDeserialize for #name #g_ty #g_where {
                const ANCHOR: i32 = #anchor;
                const ANCHOR_OFFSET: i32 = #anchor_offset;
                fn ubf_validate_group(base: i32) -> #krate::UbfResult<()> {
                    #krate::ubf_occurrence(base, 0)?;
                    #(#group_checks)* Ok(())
                }
                fn ubf_read_at(ubf: &#krate::TypedUbf<'_>, base: i32) -> #krate::UbfResult<Self> {
                    <Self as #krate::UbfGroupDeserialize>::ubf_validate_group(base)?;
                    Ok(Self { #(#group_statements)* })
                }
            }
            impl #g_impl #krate::UbfGroupFieldDeserialize for #name #g_ty #g_where {
                fn ubf_group_read_field(ubf: &#krate::TypedUbf<'_>, base: i32) -> #krate::UbfResult<Self> {
                    <Self as #krate::UbfGroupDeserialize>::ubf_read_at(ubf, base)
                }
            }
        }
    };

    Ok(match (is_view, serialize) {
        (false, true) => quote! {
            impl #impl_generics #krate::UbfSerialize for #name #ty_generics #where_clause {
                fn ubf_serialize(&self, ubf: &mut #krate::TypedUbf<'_>, realloc: bool) -> #krate::UbfResult<()> { #body }
            }
            #group_impl
        },
        (false, false) => quote! {
            impl #impl_generics #krate::UbfDeserialize for #name #ty_generics #where_clause {
                fn ubf_deserialize(ubf: &#krate::TypedUbf<'_>) -> #krate::UbfResult<Self> { #body }
            }
            #group_impl
        },
        (true, true) => quote! {
            impl #impl_generics #krate::ViewSerialize for #name #ty_generics #where_clause {
                const VIEW_NAME: &'static str = #view_name;
                fn view_serialize(&self, view: &mut #krate::TypedView<'_>) -> #krate::UbfResult<()> {
                    #krate::check_view(view, #view_name)?;
                    #body
                }
            }
        },
        (true, false) => quote! {
            impl #impl_generics #krate::ViewDeserialize for #name #ty_generics #where_clause {
                const VIEW_NAME: &'static str = #view_name;
                fn view_deserialize(view: &#krate::TypedView<'_>) -> #krate::UbfResult<Self> {
                    #krate::check_view(view, #view_name)?;
                    #body
                }
            }
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conflicting_storage_and_duplicate_occurrences_are_rejected() {
        for input in [
            quote!(
                struct Bad {
                    #[ubf(field = 1, nested, ptr)]
                    child: Child,
                }
            ),
            quote!(
                struct Bad {
                    #[ubf(field = 1, occ = 0, occurrence = 1)]
                    n: i64,
                }
            ),
            quote!(
                struct Bad {
                    #[ubf(flatten, field = 1)]
                    child: Child,
                }
            ),
            quote!(
                struct Bad {
                    #[ubf(field = 1, size = 256)]
                    child: i64,
                }
            ),
            quote!(
                struct Bad {
                    #[ubf(field = 1, group)]
                    child: Child,
                }
            ),
        ] {
            assert!(expand(syn::parse2(input).unwrap(), false, true).is_err());
        }
    }
    #[test]
    fn group_container_rejects_a_second_occurrence_axis() {
        // A #[ubf(group)] struct cannot itself hold a group or flatten field.
        for serialize in [true, false] {
            assert!(expand(
                parse_quote!(
                    #[ubf(group)]
                    struct Bad {
                        #[ubf(group)]
                        rows: Vec<Row>,
                    }
                ),
                false,
                serialize,
            )
            .is_err());
        }
        // A group struct still round-trips as a normal struct and as a group.
        for serialize in [true, false] {
            assert!(expand(
                parse_quote!(
                    #[ubf(group)]
                    struct Ok {
                        #[ubf(field = 1)]
                        id: i64,
                    }
                ),
                false,
                serialize,
            )
            .is_ok());
        }
    }
    #[test]
    fn view_requires_an_explicit_schema_name() {
        assert!(expand(
            parse_quote!(
                struct Bad {
                    n: i64,
                }
            ),
            true,
            true
        )
        .is_err());
    }
    #[test]
    fn unsupported_struct_shapes_are_rejected() {
        assert!(expand(
            parse_quote!(
                enum Bad {
                    A,
                }
            ),
            false,
            false
        )
        .is_err());
        assert!(expand(
            parse_quote!(
                struct Bad(i64);
            ),
            false,
            false
        )
        .is_err());
    }
}
