use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, GenericArgument, ItemFn, Pat, PathArguments, ReturnType, Type};

pub fn expand(function: ItemFn) -> syn::Result<TokenStream> {
    if function.sig.asyncness.is_none() {
        return Err(syn::Error::new_spanned(
            &function.sig.fn_token,
            "fission_effect requires an async function",
        ));
    }
    if !function.sig.generics.params.is_empty() || function.sig.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            &function.sig.generics,
            "fission_effect functions cannot be generic",
        ));
    }

    let (ok_ty, err_ty) = result_types(&function.sig.output)?;
    let mut arg_names = Vec::new();
    let mut arg_types = Vec::new();
    for input in &function.sig.inputs {
        let FnArg::Typed(argument) = input else {
            return Err(syn::Error::new_spanned(
                input,
                "fission_effect functions cannot have a receiver",
            ));
        };
        let Pat::Ident(pattern) = argument.pat.as_ref() else {
            return Err(syn::Error::new_spanned(
                &argument.pat,
                "fission_effect parameters must use simple identifier patterns",
            ));
        };
        arg_names.push(pattern.ident.clone());
        arg_types.push((*argument.ty).clone());
    }

    let name = &function.sig.ident;
    let module = format_ident!("__fission_effect_{}", name);
    let visibility = &function.vis;
    let request_ty = request_type(&arg_types);
    let destructure = request_destructure(&arg_names);
    let call_args = &arg_names;
    let rt = crate::support();

    Ok(quote! {
        #function

        #[doc(hidden)]
        #visibility mod #module {
            #[derive(Clone, Copy, Debug)]
            pub struct Definition;

            impl #rt::core::AppEffect for Definition {
                type Request = #request_ty;
                type Ok = #ok_ty;
                type Err = #err_ty;

                const NAME: &'static str = concat!(module_path!(), "::", stringify!(#name));

                fn run(
                    __request: Self::Request,
                    _ctx: #rt::core::JobCtx,
                ) -> #rt::core::BoxFuture<Result<Self::Ok, Self::Err>> {
                    #destructure
                    ::std::boxed::Box::pin(async move { super::#name(#(#call_args),*).await })
                }
            }
        }
    })
}

fn result_types(output: &ReturnType) -> syn::Result<(Type, Type)> {
    let ReturnType::Type(_, ty) = output else {
        return Err(syn::Error::new_spanned(
            output,
            "fission_effect must return Result<Ok, Err>",
        ));
    };
    let Type::Path(path) = ty.as_ref() else {
        return Err(syn::Error::new_spanned(
            ty,
            "fission_effect must return Result<Ok, Err>",
        ));
    };
    let Some(segment) = path.path.segments.last() else {
        return Err(syn::Error::new_spanned(
            ty,
            "fission_effect must return Result<Ok, Err>",
        ));
    };
    if segment.ident != "Result" {
        return Err(syn::Error::new_spanned(
            ty,
            "fission_effect must return Result<Ok, Err>",
        ));
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(syn::Error::new_spanned(
            ty,
            "fission_effect must return Result<Ok, Err>",
        ));
    };
    let types = arguments
        .args
        .iter()
        .filter_map(|argument| match argument {
            GenericArgument::Type(ty) => Some(ty.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    if types.len() != 2 {
        return Err(syn::Error::new_spanned(
            ty,
            "fission_effect must return Result<Ok, Err>",
        ));
    }
    Ok((types[0].clone(), types[1].clone()))
}

fn request_type(types: &[Type]) -> TokenStream {
    match types {
        [] => quote!(()),
        [ty] => quote!(#ty),
        _ => quote!((#(#types,)*)),
    }
}

fn request_destructure(names: &[syn::Ident]) -> TokenStream {
    match names {
        [] => quote!(let () = __request;),
        [name] => quote!(let #name = __request;),
        _ => quote!(let (#(#names,)*) = __request;),
    }
}
