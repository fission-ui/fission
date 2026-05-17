//! Procedural macros for the Fission UI framework.
//!
//! Provides:
//! - `#[derive(Action)]` to generate `Action` trait implementations
//! - `#[fission_action]` to inject the standard Fission action derives
//! - `#[fission_reducer]` to generate an action type from an ergonomic reducer
//! - `#[derive(Widget)]` (currently a no-op placeholder)

use proc_macro::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream, Parser},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    Attribute, DeriveInput, FnArg, GenericParam, Ident, Item, ItemFn, LitStr, Meta, Pat, PatIdent,
    PatType, Path, ReturnType, Token, Type, TypeReference, Visibility,
};

/// Derives the `Action` trait for a struct.
///
/// Generates:
/// 1. An `impl Action for <Name>`.
/// 2. A lazily initialized action ID computed from the fully qualified type path.
///
/// # Requirements
///
/// - The struct should derive `Serialize` and `Deserialize` for dispatch.
#[proc_macro_derive(Action)]
pub fn derive_action(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let full_path_str = quote! { concat!(module_path!(), "::", stringify!(#name)) };
    let fission_core_path = fission_core_path();

    let expanded = quote! {
        #[automatically_derived]
        impl #impl_generics #fission_core_path::action::Action for #name #ty_generics #where_clause {
            fn static_id() -> #fission_core_path::action::ActionId {
                static ACTION_ID: ::std::sync::OnceLock<#fission_core_path::action::ActionId> = ::std::sync::OnceLock::new();
                *ACTION_ID.get_or_init(|| #fission_core_path::action::ActionId::from_name(#full_path_str))
            }
        }
    };

    expanded.into()
}

/// Injects the standard Fission action derives onto a struct or enum.
///
/// By default this adds:
///
/// - `fission_macros::Action`
/// - `serde::Serialize`
/// - `serde::Deserialize`
/// - `Debug`
/// - `Clone`
/// - `PartialEq`
/// - `Eq`
///
/// Use `#[fission_action(no_eq)]` for payloads that cannot implement `Eq`.
#[proc_macro_attribute]
pub fn fission_action(attr: TokenStream, item: TokenStream) -> TokenStream {
    let include_eq = match parse_fission_action_args(attr) {
        Ok(include_eq) => include_eq,
        Err(error) => return error.to_compile_error().into(),
    };

    let item = parse_macro_input!(item as Item);

    let expanded = match item {
        Item::Struct(mut item_struct) => {
            if let Err(error) = merge_action_derives(&mut item_struct.attrs, include_eq) {
                return error.to_compile_error().into();
            }
            quote! { #item_struct }
        }
        Item::Enum(mut item_enum) => {
            if let Err(error) = merge_action_derives(&mut item_enum.attrs, include_eq) {
                return error.to_compile_error().into();
            }
            quote! { #item_enum }
        }
        other => {
            return syn::Error::new_spanned(
                other,
                "#[fission_action] can only be applied to a struct or enum",
            )
            .to_compile_error()
            .into();
        }
    };

    TokenStream::from(expanded)
}

/// Generates a Fission action type plus a canonical reducer wrapper from a
/// compact reducer function.
///
/// ```rust,ignore
/// #[fission_reducer(Increment)]
/// fn increment(state: &mut CounterState) {
///     state.count += 1;
/// }
///
/// #[fission_reducer(SetCount)]
/// fn set_count(state: &mut CounterState, value: i32) {
///     state.count = value;
/// }
/// ```
///
/// The first parameter must be `state: &mut State`. Any following parameters
/// become tuple payload fields on the generated action, except an optional
/// final `ctx: &mut ReducerContext<State>` parameter, which is forwarded to the
/// reducer body.
#[proc_macro_attribute]
pub fn fission_reducer(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as FissionReducerArgs);
    let input = parse_macro_input!(item as ItemFn);

    match expand_fission_reducer(args, input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Reserved derive macro for future widget code generation. Currently a no-op.
#[proc_macro_derive(Widget, attributes(widget))]
pub fn derive_widget(_input: TokenStream) -> TokenStream {
    quote!().into()
}

fn parse_fission_action_args(attr: TokenStream) -> syn::Result<bool> {
    let parser = Punctuated::<Path, Token![,]>::parse_terminated;
    let args = parser.parse(attr)?;
    let mut include_eq = true;

    for arg in args {
        if arg.is_ident("no_eq") {
            include_eq = false;
        } else {
            return Err(syn::Error::new_spanned(
                arg,
                "unsupported #[fission_action(...)] option; supported: no_eq",
            ));
        }
    }

    Ok(include_eq)
}

struct FissionReducerArgs {
    action_ident: Ident,
    include_eq: bool,
}

impl Parse for FissionReducerArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let action_ident = input.parse()?;
        let mut include_eq = true;

        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            let option: Ident = input.parse()?;
            if option == "no_eq" {
                include_eq = false;
            } else {
                return Err(syn::Error::new_spanned(
                    option,
                    "unsupported #[fission_reducer(...)] option; supported: no_eq",
                ));
            }
        }

        Ok(Self {
            action_ident,
            include_eq,
        })
    }
}

struct ReducerParam {
    ident: Ident,
    ty: Type,
}

fn expand_fission_reducer(
    args: FissionReducerArgs,
    mut input: ItemFn,
) -> syn::Result<proc_macro2::TokenStream> {
    validate_reducer_signature(&input)?;

    let fn_vis = input.vis.clone();
    let fn_ident = input.sig.ident.clone();
    let impl_ident = format_ident!("__fission_reducer_{}_impl", fn_ident);
    let attrs = input.attrs.clone();
    input.attrs.clear();
    input.vis = Visibility::Inherited;
    input.sig.ident = impl_ident.clone();

    let mut params = input.sig.inputs.iter();
    let state_arg = params.next().ok_or_else(|| {
        syn::Error::new_spanned(&input.sig, "reducer must accept state as first argument")
    })?;
    let state_ty = extract_state_type(state_arg)?;

    let mut payload_params = Vec::new();
    let mut has_ctx = false;
    let remaining: Vec<&FnArg> = params.collect();
    for (index, arg) in remaining.iter().enumerate() {
        let typed = typed_arg(arg)?;
        if is_reducer_context_type(&typed.ty) {
            if index != remaining.len() - 1 {
                return Err(syn::Error::new_spanned(
                    &typed.ty,
                    "ReducerContext parameter must be the final reducer argument",
                ));
            }
            has_ctx = true;
            continue;
        }

        payload_params.push(ReducerParam {
            ident: pat_ident(&typed.pat)?,
            ty: (*typed.ty).clone(),
        });
    }

    let action_ident = args.action_ident;
    let action = render_generated_action(&fn_vis, &action_ident, &payload_params, args.include_eq);
    let action_arg = if payload_params.is_empty() {
        quote!(_action: #action_ident)
    } else {
        quote!(action: #action_ident)
    };
    let destructure = render_action_destructure(&action_ident, &payload_params);
    let call_args = render_impl_call_args(&payload_params, has_ctx);
    let fission_core_path = fission_core_path();

    Ok(quote! {
        #action

        #input

        #(#attrs)*
        #fn_vis fn #fn_ident(
            state: &mut #state_ty,
            #action_arg,
            ctx: &mut #fission_core_path::ReducerContext<#state_ty>,
        ) {
            #destructure
            #impl_ident(#call_args);
        }
    })
}

fn validate_reducer_signature(input: &ItemFn) -> syn::Result<()> {
    if input.sig.asyncness.is_some() {
        return Err(syn::Error::new_spanned(
            input.sig.asyncness,
            "#[fission_reducer] does not support async reducers",
        ));
    }
    if input.sig.constness.is_some() {
        return Err(syn::Error::new_spanned(
            input.sig.constness,
            "#[fission_reducer] does not support const reducers",
        ));
    }
    if input.sig.unsafety.is_some() {
        return Err(syn::Error::new_spanned(
            input.sig.unsafety,
            "#[fission_reducer] does not support unsafe reducers",
        ));
    }
    if input.sig.abi.is_some() {
        return Err(syn::Error::new_spanned(
            &input.sig.abi,
            "#[fission_reducer] does not support extern reducers",
        ));
    }
    if !input.sig.generics.params.is_empty() {
        let span_target: &GenericParam = input.sig.generics.params.first().unwrap();
        return Err(syn::Error::new_spanned(
            span_target,
            "#[fission_reducer] does not support generic reducers",
        ));
    }
    if !matches!(input.sig.output, ReturnType::Default) {
        return Err(syn::Error::new_spanned(
            &input.sig.output,
            "#[fission_reducer] reducers must return ()",
        ));
    }

    Ok(())
}

fn typed_arg(arg: &FnArg) -> syn::Result<&PatType> {
    match arg {
        FnArg::Typed(typed) => Ok(typed),
        FnArg::Receiver(receiver) => Err(syn::Error::new_spanned(
            receiver,
            "#[fission_reducer] can only be applied to free functions",
        )),
    }
}

fn pat_ident(pat: &Pat) -> syn::Result<Ident> {
    match pat {
        Pat::Ident(PatIdent { ident, .. }) => Ok(ident.clone()),
        other => Err(syn::Error::new_spanned(
            other,
            "#[fission_reducer] parameters must use simple identifiers",
        )),
    }
}

fn extract_state_type(arg: &FnArg) -> syn::Result<Type> {
    let typed = typed_arg(arg)?;
    let ty = typed.ty.as_ref();
    match ty {
        Type::Reference(TypeReference {
            mutability: Some(_),
            elem,
            ..
        }) => Ok((**elem).clone()),
        other => Err(syn::Error::new_spanned(
            other,
            "#[fission_reducer] first parameter must be state: &mut State",
        )),
    }
}

fn is_reducer_context_type(ty: &Type) -> bool {
    match ty {
        Type::Reference(TypeReference {
            mutability: Some(_),
            elem,
            ..
        }) => match elem.as_ref() {
            Type::Path(path) => path
                .path
                .segments
                .last()
                .map(|segment| segment.ident == "ReducerContext")
                .unwrap_or(false),
            _ => false,
        },
        _ => false,
    }
}

fn render_generated_action(
    vis: &Visibility,
    action_ident: &Ident,
    payload_params: &[ReducerParam],
    include_eq: bool,
) -> proc_macro2::TokenStream {
    let derive_attrs = render_action_derive_attrs(include_eq);

    if payload_params.is_empty() {
        return quote! {
            #derive_attrs
            #vis struct #action_ident;
        };
    }

    let field_vis = match vis {
        Visibility::Inherited => quote!(),
        _ => quote!(#vis),
    };
    let field_tys = payload_params.iter().map(|param| &param.ty);

    quote! {
        #derive_attrs
        #vis struct #action_ident(#(#field_vis #field_tys),*);
    }
}

fn render_action_derive_attrs(include_eq: bool) -> proc_macro2::TokenStream {
    let action_path = action_derive_path();
    let serialize_path = serde_derive_path("Serialize");
    let deserialize_path = serde_derive_path("Deserialize");
    let eq = include_eq.then(|| quote!(, ::core::cmp::Eq));
    let serde_crate = fission_serde_crate_path().map(|crate_path| {
        let crate_path = LitStr::new(&crate_path, proc_macro2::Span::call_site());
        quote!(#[serde(crate = #crate_path)])
    });

    quote! {
        #[derive(
            #action_path,
            #serialize_path,
            #deserialize_path,
            ::core::fmt::Debug,
            ::core::clone::Clone,
            ::core::cmp::PartialEq
            #eq
        )]
        #serde_crate
    }
}

fn render_action_destructure(
    action_ident: &Ident,
    payload_params: &[ReducerParam],
) -> proc_macro2::TokenStream {
    if payload_params.is_empty() {
        return quote!();
    }

    let payload_idents = payload_params.iter().map(|param| &param.ident);
    quote! {
        let #action_ident(#(#payload_idents),*) = action;
    }
}

fn render_impl_call_args(
    payload_params: &[ReducerParam],
    has_ctx: bool,
) -> proc_macro2::TokenStream {
    let payload_idents = payload_params.iter().map(|param| &param.ident);
    match (payload_params.is_empty(), has_ctx) {
        (true, false) => quote!(state),
        (true, true) => quote!(state, ctx),
        (false, false) => quote!(state, #(#payload_idents),*),
        (false, true) => quote!(state, #(#payload_idents,)* ctx),
    }
}

fn merge_action_derives(attrs: &mut Vec<Attribute>, include_eq: bool) -> syn::Result<()> {
    let mut existing = std::collections::BTreeSet::new();

    for attr in attrs.iter().filter(|attr| attr.path().is_ident("derive")) {
        let derives = attr.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)?;
        for derive in derives {
            if let Some(segment) = derive.segments.last() {
                existing.insert(segment.ident.to_string());
            }
        }
    }

    let standard_derives: Vec<Path> = vec![
        action_derive_path(),
        serde_derive_path("Serialize"),
        serde_derive_path("Deserialize"),
        parse_quote!(::core::fmt::Debug),
        parse_quote!(::core::clone::Clone),
        parse_quote!(::core::cmp::PartialEq),
    ];
    let mut missing: Vec<Path> = standard_derives
        .into_iter()
        .filter(|path| {
            path.segments
                .last()
                .map(|segment| !existing.contains(&segment.ident.to_string()))
                .unwrap_or(true)
        })
        .collect();

    if include_eq && !existing.contains("Eq") {
        missing.push(parse_quote!(::core::cmp::Eq));
    }

    if !missing.is_empty() {
        attrs.insert(0, parse_quote!(#[derive(#(#missing),*)]));
    }

    if let Some(crate_path) = fission_serde_crate_path() {
        ensure_serde_crate_attr(attrs, &crate_path)?;
    }

    Ok(())
}

fn action_derive_path() -> Path {
    if let Ok(found) = crate_name("fission") {
        return match found {
            FoundCrate::Itself => parse_quote!(::fission::macros::Action),
            FoundCrate::Name(name) => {
                let crate_ident = format_ident!("{}", name);
                parse_quote!(::#crate_ident::macros::Action)
            }
        };
    }

    if let Ok(found) = crate_name("fission-macros") {
        return match found {
            FoundCrate::Itself => parse_quote!(crate::Action),
            FoundCrate::Name(name) => {
                let crate_ident = format_ident!("{}", name);
                parse_quote!(::#crate_ident::Action)
            }
        };
    }

    parse_quote!(Action)
}

fn fission_core_path() -> Path {
    if let Ok(found) = crate_name("fission") {
        return match found {
            FoundCrate::Itself => parse_quote!(::fission::core),
            FoundCrate::Name(name) => {
                let crate_ident = format_ident!("{}", name);
                parse_quote!(::#crate_ident::core)
            }
        };
    }

    if let Ok(found) = crate_name("fission-core") {
        return match found {
            FoundCrate::Itself => parse_quote!(::fission_core),
            FoundCrate::Name(name) => {
                let crate_ident = format_ident!("{}", name);
                parse_quote!(::#crate_ident)
            }
        };
    }

    parse_quote!(fission_core)
}

fn serde_derive_path(derive_name: &str) -> Path {
    let derive_ident = format_ident!("{}", derive_name);

    if let Ok(found) = crate_name("fission") {
        return match found {
            FoundCrate::Itself => parse_quote!(::fission::serde::#derive_ident),
            FoundCrate::Name(name) => {
                let crate_ident = format_ident!("{}", name);
                parse_quote!(::#crate_ident::serde::#derive_ident)
            }
        };
    }

    if let Ok(found) = crate_name("serde") {
        return match found {
            FoundCrate::Itself => parse_quote!(::serde::#derive_ident),
            FoundCrate::Name(name) => {
                let crate_ident = format_ident!("{}", name);
                parse_quote!(::#crate_ident::#derive_ident)
            }
        };
    }

    parse_quote!(serde::#derive_ident)
}

fn fission_serde_crate_path() -> Option<String> {
    crate_name("fission").ok().map(|found| match found {
        FoundCrate::Itself => "::fission::serde".to_string(),
        FoundCrate::Name(name) => format!("::{name}::serde"),
    })
}

fn ensure_serde_crate_attr(attrs: &mut Vec<Attribute>, crate_path: &str) -> syn::Result<()> {
    if has_serde_crate_attr(attrs)? {
        return Ok(());
    }

    let crate_path = LitStr::new(crate_path, proc_macro2::Span::call_site());
    let insert_index = attrs
        .iter()
        .position(|attr| attr.path().is_ident("derive"))
        .map(|index| index + 1)
        .unwrap_or(0);
    attrs.insert(insert_index, parse_quote!(#[serde(crate = #crate_path)]));
    Ok(())
}

fn has_serde_crate_attr(attrs: &[Attribute]) -> syn::Result<bool> {
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("serde")) {
        let metas = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        for meta in metas {
            if meta.path().is_ident("crate") {
                return Ok(true);
            }
        }
    }

    Ok(false)
}
