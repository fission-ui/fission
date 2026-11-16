use proc_macro2::TokenStream as Tokens;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    Expr, Ident, Token, Type, Visibility,
};

// Simple: state_scope!(AppState.products => ProductState);
// Named:  state_scope! { pub Session(AppState, u64 => ProductState) {
//             get: |root, key| root.editors.get(key),
//             get_mut: |root, key| root.editors.get_mut(key),
//         } }
enum Input {
    Simple {
        vis: Visibility,
        root: syn::Path,
        fields: Vec<Ident>,
        target: Type,
    },
    Named {
        vis: Visibility,
        name: Ident,
        root: Type,
        key: Type,
        target: Type,
        get: syn::ExprClosure,
        get_mut: syn::ExprClosure,
    },
}
impl Parse for Input {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let fork = input.fork();
        let _: Visibility = fork.parse()?;
        let _: syn::Path = fork.parse()?;
        if fork.peek(Token![.]) || fork.peek(Token![=>]) {
            let vis = input.parse()?;
            let root: syn::Path = input.parse()?;
            let mut fields = Vec::new();
            while input.peek(Token![.]) {
                input.parse::<Token![.]>()?;
                fields.push(input.parse()?);
            }
            input.parse::<Token![=>]>()?;
            let target = input.parse()?;
            if input.peek(Token![;]) {
                input.parse::<Token![;]>()?;
            }
            if !input.is_empty() {
                return Err(input.error("unexpected tokens after state scope"));
            }
            Ok(Self::Simple {
                vis,
                root,
                fields,
                target,
            })
        } else {
            let vis = input.parse()?;
            let name = input.parse()?;
            let args;
            syn::parenthesized!(args in input);
            let root = args.parse()?;
            args.parse::<Token![,]>()?;
            let key = args.parse()?;
            args.parse::<Token![=>]>()?;
            let target = args.parse()?;
            if !args.is_empty() {
                return Err(args.error("unexpected scope type arguments"));
            }
            let body;
            syn::braced!(body in input);
            let first: Ident = body.parse()?;
            if first != "get" {
                return Err(syn::Error::new(first.span(), "expected get"));
            }
            body.parse::<Token![:]>()?;
            let get = body.parse()?;
            body.parse::<Token![,]>()?;
            let second: Ident = body.parse()?;
            if second != "get_mut" {
                return Err(syn::Error::new(second.span(), "expected get_mut"));
            }
            body.parse::<Token![:]>()?;
            let get_mut = body.parse()?;
            if body.peek(Token![,]) {
                body.parse::<Token![,]>()?;
            }
            if !body.is_empty() || !input.is_empty() {
                return Err(body.error("unexpected tokens after scope accessors"));
            }
            Ok(Self::Named {
                vis,
                name,
                root,
                key,
                target,
                get,
                get_mut,
            })
        }
    }
}
fn body(c: syn::ExprClosure) -> syn::Result<(syn::Pat, syn::Pat, Expr)> {
    if c.inputs.len() != 2 || c.asyncness.is_some() || c.capture.is_some() {
        return Err(syn::Error::new_spanned(
            c,
            "scope accessors take |root, key| and cannot capture",
        ));
    }
    for p in &c.inputs {
        if !matches!(p, syn::Pat::Ident(i) if i.by_ref.is_none() && i.subpat.is_none() && i.mutability.is_none())
            && !matches!(p, syn::Pat::Wild(_))
        {
            return Err(syn::Error::new_spanned(
                p,
                "scope parameters must be identifiers or _",
            ));
        }
    }
    let mut args = c.inputs.into_iter();
    Ok((args.next().unwrap(), args.next().unwrap(), *c.body))
}
pub fn expand(tokens: Tokens) -> syn::Result<Tokens> {
    let rt = crate::support();
    match syn::parse2::<Input>(tokens)? {
        Input::Simple {
            vis,
            root,
            fields,
            target,
        } => {
            let target_name = match &target {
                Type::Path(p) => p.path.segments.last().unwrap().ident.clone(),
                _ => {
                    return Err(syn::Error::new_spanned(
                        target,
                        "expected a named state type",
                    ))
                }
            };
            let name = format_ident!("__{}DefaultScope", target_name);
            let get = if fields.is_empty() {
                quote!(root)
            } else {
                quote!(&root.#(#fields).*)
            };
            let get_mut = if fields.is_empty() {
                quote!(root)
            } else {
                quote!(&mut root.#(#fields).*)
            };
            Ok(quote! {
                #[doc(hidden)] #vis struct #name;
                impl #rt::Scope for #name {
                    type Root=#root;type Target=#target;type Key=();
                    fn get<'a>(root:&'a Self::Root,_:&())->Option<&'a Self::Target>{Some(#get)}
                    fn get_mut<'a>(root:&'a mut Self::Root,_:&())->Option<&'a mut Self::Target>{Some(#get_mut)}
                }
                impl #rt::DefaultScope for #target {type Scope=#name;}
            })
        }
        Input::Named {
            vis,
            name,
            root,
            key,
            target,
            get,
            get_mut,
        } => {
            let (r, k, b) = body(get)?;
            let (rm, km, bm) = body(get_mut)?;
            Ok(quote! {
                #vis struct #name;
                impl #rt::Scope for #name {
                    type Root=#root;type Target=#target;type Key=#key;
                    fn get<'a>(#r:&'a #root,#k:&#key)->Option<&'a #target>{#b}
                    fn get_mut<'a>(#rm:&'a mut #root,#km:&#key)->Option<&'a mut #target>{#bm}
                }
            })
        }
    }
}
