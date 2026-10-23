use crate::ast::{Call, Component, Effect, Value};
use proc_macro2::{Span, TokenStream as Tokens, TokenTree};
use quote::{format_ident, quote, ToTokens};
use std::collections::{BTreeSet, HashSet};
use syn::{Expr, Ident, Lit, LitStr, Member, Pat};

pub fn component(input: Component) -> syn::Result<Tokens> {
    let rt = crate::support();
    let Component {
        attrs,
        vis,
        name,
        target,
        scope,
        fields,
        editor,
        state,
        cx,
        setup,
        body,
    } = input;
    let derives_clone = attrs.iter().any(|attr| {
        attr.path().is_ident("derive")
            && attr
                .parse_args_with(
                    syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
                )
                .is_ok_and(|paths| paths.iter().any(|path| path.is_ident("Clone")))
    });
    let clone_derive = (!derives_clone).then(|| quote!(#[derive(Clone)]));
    let setup = setup.map(|block| block.stmts).unwrap_or_default();
    let root_scope_ty = format_ident!("__{}RootScope", name);
    let (scope_ty, root_scope_item) = match &scope {
        Some((p, _)) => (quote!(#p), None),
        None => (
            quote!(#root_scope_ty),
            Some(quote! {
                #[doc(hidden)]
                struct #root_scope_ty;
                impl #rt::Scope for #root_scope_ty {
                    type Root = #target;
                    type Target = #target;
                    type Key = ();

                    fn get<'__a>(root: &'__a #target, _key: &()) -> Option<&'__a #target> {
                        Some(root)
                    }

                    fn get_mut<'__a>(root: &'__a mut #target, _key: &()) -> Option<&'__a mut #target> {
                        Some(root)
                    }
                }
            }),
        ),
    };
    let key = scope
        .as_ref()
        .map(|(_, e)| quote!(::std::clone::Clone::clone(&(#e))))
        .unwrap_or(quote!(()));
    let name_string = quote!(concat!(module_path!(), "::", stringify!(#name)));
    let state_lookup = if scope.is_none()
        && matches!(&target, syn::Type::Tuple(tuple) if tuple.elems.is_empty())
    {
        quote!(let #state: &() = &();)
    } else {
        quote! {
            let #state:&#target=match <#scope_ty as #rt::Scope>::get(__view.state(),&__scope_key){
                Some(state)=>state,
                None=>panic!(
                    "Fission Compose component `{}` could not resolve its state scope",
                    #name_string,
                ),
            };
        }
    };
    let root = quote!(<#scope_ty as #rt::Scope>::Root);
    let props: Vec<_> = fields.iter().filter(|f| f.local.is_none()).collect();
    let locals: Vec<_> = fields.iter().filter(|f| f.local.is_some()).collect();
    let view_ty = format_ident!("__{}View", name);
    let local_defs = locals.iter().map(|f| {
        let id = &f.name;
        let ty = &f.ty;
        quote!(pub #id:#ty)
    });
    let local_handles=locals.iter().map(|f|{
        let id=&f.name;let ty=&f.ty;let init=f.local.as_ref().unwrap();let handle=handle_name(id);
        quote!(let #handle=#rt::core::StateField::<#ty>::new_with(#name_string,stringify!(#id),||#init);)
    });
    let local_values = locals.iter().map(|f| {
        let id = &f.name;
        let handle = handle_name(id);
        quote!(#id:#handle.get())
    });
    let prop_fields = props.iter().map(|f| {
        let a = &f.attrs;
        let id = &f.name;
        let ty = &f.ty;
        quote!(#(#a)* pub #id:#ty)
    });
    let prop_args = props.iter().map(|f| {
        let id = &f.name;
        let ty = &f.ty;
        quote!(#id:#ty)
    });
    let prop_init = props.iter().map(|f| &f.name);
    let component_definition = if props.is_empty() {
        quote!(#(#attrs)* #clone_derive #vis struct #name;)
    } else {
        quote!(
            #(#attrs)*
            #clone_derive
            #vis struct #name {
                #(#prop_fields,)*
            }
        )
    };
    let constructor_body = if props.is_empty() {
        quote!(Self)
    } else {
        quote!(Self{#(#prop_init,)*})
    };
    let default_impl = props.is_empty().then(|| {
        quote!(
            impl ::std::default::Default for #name{fn default()->Self{Self}}
        )
    });
    let mut emit = Emitter {
        rt: rt.clone(),
        component: name.clone(),
        scope: scope_ty.clone(),
        target: quote!(#target),
        root: root.clone(),
        editor: editor.clone(),
        state: state.clone(),
        cx: cx.clone(),
        fields: &fields,
        items: Vec::new(),
        methods: Vec::new(),
        symbols: HashSet::new(),
    };
    let tree = emit.bare(&body, "view")?;
    let extra_items = emit.items;
    let methods = emit.methods;
    Ok(quote! {
        #component_definition
        #[doc(hidden)]
        struct #view_ty<'__a>{__props:&'__a #name,#(#local_defs,)*}
        impl ::std::ops::Deref for #view_ty<'_>{
            type Target=#name;fn deref(&self)->&Self::Target{self.__props}
        }
        impl #name {
            pub fn new(#(#prop_args),*)->Self{#constructor_body}
            #(#methods)*
        }
        #default_impl
        #root_scope_item
        #(#extra_items)*
        impl From<#name> for #rt::core::Widget {
            #[allow(unused_variables)]
            fn from(__component:#name)->Self{
                let #editor=&__component;
                let __scope_key:<#scope_ty as #rt::Scope>::Key=#key;
                let (__build,__view)=#rt::core::build::current::<#root>();
                #state_lookup
                #rt::identity::BuildSession::run(||{
                    let __base=#rt::identity::current_id();
                    let __owner=#rt::identity::child_id(__base,#name_string,&__scope_key);
                    #rt::identity::BuildSession::claim(__owner,#name_string);
                    #rt::core::build::with_implicit_widget_id(__owner,||{
                        let __mount=#rt::Mount::new(#name_string,__view);
                        #(#local_handles)*
                        let #editor=#view_ty{__props:&__component,#(#local_values,)*};
                        let #cx=#rt::view_cx::ViewCx{build:__build,view:__view};
                        let _=(&#editor,&#state,&#cx);
                        #(#setup)*
                        let __tree:#rt::core::Widget=(#tree).into();
                        #rt::core::ui::Composite {
                            id: Some(__owner),
                            child: __tree,
                            ..::std::default::Default::default()
                        }.into()
                    })
                })
            }
        }
    })
}
fn handle_name(id: &Ident) -> Ident {
    format_ident!("__local_{}", id)
}

fn is_static_str(ty: &syn::Type) -> bool {
    let syn::Type::Reference(reference) = ty else {
        return false;
    };
    reference
        .lifetime
        .as_ref()
        .is_some_and(|lifetime| lifetime.ident == "static")
        && matches!(reference.elem.as_ref(), syn::Type::Path(path) if path.qself.is_none() && path.path.is_ident("str"))
}

fn captured_field_type(field: &crate::ast::Field) -> Tokens {
    if is_static_str(&field.ty) {
        quote!(::std::string::String)
    } else {
        let ty = &field.ty;
        quote!(#ty)
    }
}

fn captured_field_value(field: &crate::ast::Field, editor: &Ident) -> Tokens {
    let id = &field.name;
    if is_static_str(&field.ty) {
        quote!(::std::string::ToString::to_string(#editor.#id))
    } else {
        quote!(::std::clone::Clone::clone(&#editor.#id))
    }
}

struct Emitter<'a> {
    rt: Tokens,
    component: Ident,
    scope: Tokens,
    target: Tokens,
    root: Tokens,
    editor: Ident,
    state: Ident,
    cx: Ident,
    fields: &'a [crate::ast::Field],
    items: Vec<Tokens>,
    methods: Vec<Tokens>,
    symbols: HashSet<String>,
}
impl Emitter<'_> {
    fn bare(&mut self, v: &Value, path: &str) -> syn::Result<Tokens> {
        let rt = self.rt.clone();
        match v {
            Value::Effect(effect) => Err(syn::Error::new_spanned(
                &effect.operation,
                "effect is only valid as an event property such as on_press:",
            )),
            Value::Reduce(block) => Err(syn::Error::new_spanned(
                block,
                "reduce is only valid as an event property such as on_press:",
            )),
            Value::Call(c) => self.call(c, path),
            Value::Rust(e) | Value::Read(e) => Ok(borrowed_read(e, &self.editor, &self.state)),
            Value::ExplicitRust(e) | Value::Source(e) => Ok(quote!(#e)),
            Value::To(e) => Err(syn::Error::new_spanned(
                e,
                "to(...) is only valid on the right-hand side of bind(event):",
            )),
            Value::Array(vs) => {
                let mut pieces = Vec::new();
                for v in vs {
                    pieces.push(self.append(v, path)?);
                }
                Ok(
                    quote!({let mut __children:Vec<#rt::core::Widget>=Vec::new();#(#pieces)* __children}),
                )
            }
            Value::Optional(value) => {
                let value = self.bare(value, path)?;
                if v.is_plain_rust() {
                    Ok(quote!(Some(#value)))
                } else {
                    Ok(quote!(Some((#value).into())))
                }
            }
            Value::If { cond, yes, no } => {
                // Conditional branches share the same semantic namespace. This lets
                // a keyed component intentionally survive switching between branches.
                let yes = self.bare(yes, path)?;
                if let Some(no) = no {
                    let no = self.bare(no, path)?;
                    if v.is_plain_rust() {
                        Ok(quote!(if #cond { #yes } else { #no }))
                    } else {
                        Ok(quote!(if #cond{
                            let __branch:#rt::core::Widget=(#yes).into();
                            __branch
                        }else{
                            let __branch:#rt::core::Widget=(#no).into();
                            __branch
                        }))
                    }
                } else {
                    Ok(quote!(if #cond{
                        let __branch:#rt::core::Widget=(#yes).into();
                        Some(__branch)
                    }else{None}))
                }
            }
            Value::For { .. } => {
                let p = self.append(v, path)?;
                Ok(quote!({let mut __children:Vec<#rt::core::Widget>=Vec::new();#p __children}))
            }
            Value::Match { expr, arms } => {
                let mut out = Vec::new();
                let plain_rust = v.is_plain_rust();
                for (pat, guard, value) in arms {
                    let guard = guard.as_ref().map(|g| quote!(if #g));
                    let body = self.bare(value, path)?;
                    if plain_rust {
                        out.push(quote!(#pat #guard => #body));
                    } else {
                        out.push(quote!(#pat #guard=>{
                            let __branch:#rt::core::Widget=(#body).into();
                            __branch
                        }));
                    }
                }
                Ok(quote!(match #expr{#(#out,)*}))
            }
        }
    }
    fn append(&mut self, v: &Value, path: &str) -> syn::Result<Tokens> {
        Ok(match v {
            Value::If { cond, yes, no } => {
                let yes = self.append(yes, path)?;
                if let Some(no) = no {
                    let no = self.append(no, path)?;
                    quote!(if #cond{#yes}else{#no})
                } else {
                    quote!(if #cond{#yes})
                }
            }
            Value::For { pat, iter, body } => {
                if !matches!(body.as_ref(),Value::Call(c) if c.named("key").is_some()) {
                    return Err(syn::Error::new_spanned(
                        iter,
                        "a dynamic child must be a component/control with an explicit key: for item in items { Row(key: item.id, ...) }",
                    ));
                }
                let body = self.append(body, path)?;
                quote!(for #pat in #iter{#body})
            }
            Value::Array(values) => {
                let mut pieces = Vec::new();
                for v in values {
                    pieces.push(self.append(v, path)?);
                }
                quote!(#(#pieces)*)
            }
            Value::Optional(value) => {
                let value = self.bare(value, path)?;
                if v.is_plain_rust() {
                    quote!(__children.push((Some(#value)).into());)
                } else {
                    quote!(__children.push((Some((#value).into())).into());)
                }
            }
            _ => {
                let child = self.bare(v, path)?;
                quote!(__children.push((#child).into());)
            }
        })
    }
    fn value(&mut self, v: &Value, path: &str) -> syn::Result<Tokens> {
        if let Some(expr) = v.rust() {
            return Ok(borrowed_read(expr, &self.editor, &self.state));
        }
        self.bare(v, path)
    }

    fn property_value(&mut self, v: &Value, path: &str, property: &str) -> syn::Result<Tokens> {
        if property != "children" {
            if let Value::Array(values) = v {
                let values = values
                    .iter()
                    .map(|value| {
                        value
                            .rust()
                            .map(|expr| property_expression(expr, &self.editor, &self.state))
                            .ok_or_else(|| {
                                syn::Error::new(
                                    Span::call_site(),
                                    "only children may contain component expressions; this property array must contain Rust expressions",
                                )
                            })
                    })
                    .collect::<syn::Result<Vec<_>>>()?;
                return Ok(quote!([#(#values),*]));
            }
        }
        if let Value::Rust(expr) = v {
            return Ok(property_expression(expr, &self.editor, &self.state));
        }
        self.value(v, path)
    }
    fn binding(&mut self, v: &Value) -> syn::Result<Tokens> {
        let rt = self.rt.clone();
        if let Value::Source(e) = v {
            return Ok(quote!(#e));
        }
        let Some(expr) = v.rust() else {
            return Err(syn::Error::new(
                Span::call_site(),
                "bind requires a state/local field or source(existing_binding)",
            ));
        };
        if let Some(fields) = place(expr, &self.state) {
            let scope = &self.scope;
            let target = &self.target;
            let symbol = format!(
                "state.{}",
                fields
                    .iter()
                    .map(Ident::to_string)
                    .collect::<Vec<_>>()
                    .join(".")
            );
            let access = fields
                .iter()
                .fold(quote!(__s), |path, field| quote!(#path.#field));
            return Ok(quote!(#rt::binding::state_source::<#scope,_>(
                __build,&__mount,__scope_key.clone(),#symbol,&(#expr),
                |__s:&#target|&#access,|__s:&mut #target,__v|{#access=__v;},
            )));
        }
        if let Some(fields) = place(expr, &self.editor) {
            if fields.len() == 1
                && self
                    .fields
                    .iter()
                    .any(|f| f.name == fields[0] && f.local.is_some())
            {
                let h = handle_name(&fields[0]);
                let symbol = format!("local.{}", fields[0]);
                return Ok(quote!(#rt::binding::local_source(__build,&__mount,#h.clone(),#symbol)));
            }
        }
        Err(syn::Error::new_spanned(
            expr,
            "bind requires a directly writable state or local field; use source(existing_binding) for a custom binding",
        ))
    }
    fn call(&mut self, c: &Call, parent_path: &str) -> syn::Result<Tokens> {
        let rt = self.rt.clone();
        let widget = &c.path;
        let widget_name = widget.to_token_stream().to_string().replace(' ', "");
        let authored_shape = call_shape(c);
        let automatic_key = format!("shape:{:016x}", stable_hash(&authored_shape));
        let key = if let Some(key) = c.named("key") {
            let expr = key
                .rust()
                .ok_or_else(|| syn::Error::new_spanned(widget, "key must be a Rust expression"))?;
            quote!(::std::clone::Clone::clone(&(#expr)))
        } else {
            let automatic_key = LitStr::new(&automatic_key, Span::call_site());
            quote!(#automatic_key)
        };
        let static_key = c
            .named("key")
            .and_then(Value::rust)
            .and_then(string_literal);
        let path = format!(
            "{parent_path}/{widget_name}[{}]",
            static_key.as_deref().unwrap_or(&automatic_key)
        );
        let mut fields = Vec::new();
        let mut events = Vec::new();
        let mut bindings = Vec::new();
        let mut positional = Vec::new();
        for arg in &c.args {
            if let Some(binding) = &arg.binding {
                let Value::To(target) = &arg.value else {
                    return Err(syn::Error::new_spanned(
                        &binding.port,
                        "an event binding must use to(state.field) or to(editor.local)",
                    ));
                };
                let port = &binding.port;
                let property = port.to_string();
                let event = match &binding.event {
                    Some(event) => quote!(#event),
                    None => default_edit_event(port)?,
                };
                let source = self.binding(&Value::Rust(target.clone()))?;
                events.push(quote!(
                    #rt::identity::in_slot(#property,||{
                        __widget.#port=
                            #rt::binding::update_for::<_,#event,_>(#property,#source);
                    });
                ));
                continue;
            }
            let (method, property) = if let Some(id) = &arg.name {
                if id == "key" || id == "handler" {
                    continue;
                }
                (id.clone(), id.to_string())
            } else {
                let argument_path = format!("{path}/arg{}", positional.len());
                let value = self.value(&arg.value, &argument_path)?;
                // A positional value may be an ordinary imported widget rather than
                // another Compose call. Give its conversion a stable identity scope
                // distinct from the widget whose constructor receives it. The scope
                // is derived from authored shape rather than argument order, so
                // inserting an unrelated constructor argument cannot renumber it.
                let slot = LitStr::new(
                    &format!("argument:{:016x}", stable_hash(&value_shape(&arg.value))),
                    Span::call_site(),
                );
                positional.push(quote!(#rt::identity::in_slot(#slot, || #value)));
                continue;
            };
            let argument_path = format!("{path}/{property}");
            let value = if property == "bind" {
                self.binding(&arg.value)?
            } else if property.starts_with("on_") {
                if let Value::Effect(effect) = &arg.value {
                    let handler_name=c.named("handler").and_then(Value::rust).and_then(string_literal)
                        .or_else(||static_key.clone())
                        .or_else(||c.named("key").is_none().then(||automatic_key.clone()))
                        .ok_or_else(||syn::Error::new_spanned(&effect.operation,"an effect handler needs a literal key: \"save\"; with a dynamic control key, add handler: \"save\""))?;
                    let handler = self.effect_handler(
                        effect,
                        &property,
                        &format!("{parent_path}/{widget_name}/{handler_name}/{property}"),
                    )?;
                    quote!((#handler).into())
                } else if let Value::Reduce(block) = &arg.value {
                    let handler_name=c.named("handler").and_then(Value::rust).and_then(string_literal)
                        .or_else(||static_key.clone())
                        .or_else(||c.named("key").is_none().then(||automatic_key.clone()))
                        .ok_or_else(||syn::Error::new_spanned(block,"a reduce handler needs a literal key: \"reset\"; with a dynamic control key, add handler: \"reset\""))?;
                    let editor = &self.editor;
                    let state = &self.state;
                    let cx = &self.cx;
                    let closure: syn::ExprClosure =
                        syn::parse_quote!(|#editor, #state, #cx| #block);
                    let handler = self.handler(
                        &closure,
                        &property,
                        &format!("{parent_path}/{widget_name}/{handler_name}/{property}"),
                    )?;
                    quote!((#handler).into())
                } else {
                    match arg.value.rust() {
                        Some(Expr::Closure(closure)) => {
                            let handler_name=c.named("handler").and_then(Value::rust).and_then(string_literal)
                            .or_else(||static_key.clone())
                            .or_else(||c.named("key").is_none().then(||automatic_key.clone()))
                            .ok_or_else(||syn::Error::new_spanned(closure,"an inline handler needs a literal key: \"save\"; with a dynamic control key, add handler: \"save\""))?;
                            let handler = self.handler(
                                closure,
                                &property,
                                &format!("{parent_path}/{widget_name}/{handler_name}/{property}"),
                            )?;
                            quote!((#handler).into())
                        }
                        Some(e) => borrowed_read(e, &self.editor, &self.state),
                        None if arg.value.is_plain_rust() => {
                            self.bare(&arg.value, &argument_path)?
                        }
                        None => {
                            return Err(syn::Error::new_spanned(
                                widget,
                                "an event takes an inline handler or an already-typed Event value",
                            ));
                        }
                    }
                }
            } else {
                self.property_value(&arg.value, &argument_path, &property)?
            };
            // Argument values are built in a named slot, never a declaration ordinal.
            if property == "bind" {
                bindings.push(quote!(let __widget=#rt::ComposeBind::compose_bind(__widget,#rt::identity::in_slot(#property,||#value));));
            } else if property.starts_with("on_") {
                events.push(quote!(
                    #rt::identity::in_slot(#property,||{
                        __widget.#method=Some(#value);
                    });
                ));
            } else {
                fields.push(quote!(
                    #rt::identity::in_slot(#property,||{
                        __widget.#method=(#value).into();
                    });
                ));
            }
        }
        let base = if positional.is_empty() {
            quote!(::std::default::Default::default())
        } else {
            quote!(#widget::new(#(#positional),*))
        };
        let modifiers = c.modifiers.iter().map(|modifier| {
            let name = &modifier.name;
            let args = &modifier.args;
            quote!(let __widget=__widget.#name(#args);)
        });
        let convert_widget = if c.named("id").is_some() {
            // A named `id:` is the conventional Fission widget identity. Do not
            // let the enclosing component's explicit ID replace it during
            // conversion. `key:` remains the independent Compose structural
            // identity used for children, bindings, and handlers above.
            quote! {
                let __widget: #rt::core::Widget =
                    #rt::core::build::without_widget_id(|| __widget.into());
                __widget
            }
        } else {
            quote! {
                let __widget: #rt::core::Widget = __widget.into();
                #rt::core::ui::WidgetIdExt::id(__widget, #rt::identity::node().id)
            }
        };
        Ok(quote!({
            let __key=#key;
            #rt::identity::with_node(stringify!(#widget),&__key,||{
                let mut __widget:#widget=#base;
                #(#fields)*
                #(#events)*
                #(#modifiers)*
                #(#bindings)*
                #convert_widget
            })
        }))
    }
    fn handler(
        &mut self,
        closure: &syn::ExprClosure,
        port: &str,
        symbol: &str,
    ) -> syn::Result<Tokens> {
        let rt = self.rt.clone();
        let name = self.component.clone();
        let scope = self.scope.clone();
        let target = self.target.clone();
        let root = self.root.clone();
        if closure.inputs.len() != 3
            || closure.capture.is_some()
            || closure.asyncness.is_some()
            || !matches!(closure.output, syn::ReturnType::Default)
        {
            return Err(syn::Error::new_spanned(
                closure,
                "inline handlers use |editor, state, cx| { ... }; no move, async, return-type annotation, or captured render locals",
            ));
        }
        let args: Vec<_> = closure
            .inputs
            .iter()
            .map(parameter)
            .collect::<syn::Result<_>>()?;
        let event = match port {
            "on_press" | "on_submit" => quote!(#rt::events::Press),
            "on_input" => quote!(#rt::events::TextChanged),
            "on_toggle" => quote!(#rt::events::Toggle),
            // Other action slots (`on_close`, `on_dismiss`, `on_select`, and
            // so on) carry the same ActionEnvelope. Callback factories such as
            // `Fn(value) -> ActionEnvelope` remain typed Rust expressions until
            // Compose has an explicit event-payload contract for them.
            _ => quote!(#rt::events::Press),
        };
        if !self.symbols.insert(symbol.into()) {
            return Err(syn::Error::new_spanned(
                closure,
                "duplicate inline handler identity; give the control or handler a distinct semantic name",
            ));
        }
        let mut used = BTreeSet::new();
        if let Some(editor) = &args[0] {
            scan_fields(closure.body.to_token_stream(), editor, &mut used)?;
        }
        // Extracted functions deliberately cannot capture function/loop-local variables.
        // Avoid changing meaning through shadowing the three event-time parameters.
        let mut check = HandlerCheck {
            params: args.iter().flatten().cloned().collect(),
            error: None,
        };
        syn::visit::Visit::visit_expr(&mut check, &closure.body);
        if let Some(e) = check.error {
            return Err(e);
        }
        for field in &used {
            if !self.fields.iter().any(|f| f.name == field.as_str()) {
                return Err(syn::Error::new_spanned(
                    closure,
                    format!(
                        "{field} is not a declared prop or local field; event editor access is limited to declared fields"
                    ),
                ));
            }
        }
        let captures: Vec<_> = self
            .fields
            .iter()
            .filter(|f| f.local.is_none() && used.contains(&f.name.to_string()))
            .collect();
        let locals: Vec<_> = self
            .fields
            .iter()
            .filter(|f| f.local.is_some() && used.contains(&f.name.to_string()))
            .collect();
        let suffix = format!("{:016x}", stable_hash(symbol));
        let action = format_ident!("__{}_Event_{}", name, suffix);
        let capture = format_ident!("__{}_Capture_{}", name, suffix);
        let facade = format_ident!("__{}_EventEditor_{}", name, suffix);
        let method = format_ident!("__event_{}", suffix);
        let ser_path = syn::LitStr::new(
            &format!("{}::serde", rt.to_string().replace(' ', "")),
            Span::call_site(),
        );
        let prop_defs = captures.iter().map(|f| {
            let id = &f.name;
            let ty = captured_field_type(f);
            quote!(#id:#ty)
        });
        let local_defs = locals.iter().map(|f| {
            let id = &f.name;
            let ty = &f.ty;
            quote!(#id:#ty)
        });
        self.items.push(quote!{
            #[allow(non_camel_case_types)]
            #[derive(Clone,#rt::serde::Serialize,#rt::serde::Deserialize)]
            #[serde(crate=#ser_path)]
            struct #capture{#(#prop_defs,)*}
            #[allow(non_camel_case_types)]
            #[derive(#rt::serde::Serialize,#rt::serde::Deserialize)]
            #[serde(crate=#ser_path)]
            struct #action{
                key:<#scope as #rt::Scope>::Key,incarnation:u64,props:#capture,
            }
            impl ::std::fmt::Debug for #action {
                fn fmt(&self, formatter:&mut ::std::fmt::Formatter<'_>)->::std::fmt::Result {
                    formatter.write_str(stringify!(#action))
                }
            }
            impl #rt::core::Action for #action{
                fn static_id()->#rt::core::ActionId{
                    #rt::core::ActionId::from_name(concat!(module_path!(),"::",stringify!(#name),"::",#symbol))
                }
            }
            #[allow(non_camel_case_types)]
            struct #facade<'a>{__props:&'a #capture,#(#local_defs,)*}
            impl ::std::ops::Deref for #facade<'_>{
                type Target=#capture;fn deref(&self)->&Self::Target{self.__props}
            }
        });
        let editor_arg = args[0]
            .clone()
            .unwrap_or_else(|| format_ident!("__unused_editor"));
        let state_arg = args[1]
            .clone()
            .unwrap_or_else(|| format_ident!("__unused_state"));
        let cx_arg = args[2]
            .clone()
            .unwrap_or_else(|| format_ident!("__unused_context"));
        let body = &closure.body;
        self.methods.push(quote!{
            #[allow(unused_variables)]
            fn #method(#editor_arg:&mut #facade<'_>,#state_arg:&mut #target,#cx_arg:&mut #rt::core::ReducerContext<#root>){#body}
        });
        let editor = &self.editor;
        let props = captures.iter().map(|f| {
            let id = &f.name;
            let value = captured_field_value(f, editor);
            quote!(#id: #value)
        });
        let keys = locals.iter().map(|f| {
            let id = &f.name;
            let h = handle_name(id);
            let k = format_ident!("__key_{}", id);
            quote!(let #k=#h.key().clone();)
        });
        let local_reads = locals.iter().map(|f| {
            let id = &f.name;
            let ty = &f.ty;
            let k = format_ident!("__key_{}", id);
            quote!(let Some(#id)=__live.snapshot::<#ty>(&#k)else{return;};)
        });
        let local_values = locals.iter().map(|f| &f.name);
        let local_writes = locals.iter().map(|f| {
            let id = &f.name;
            let k = format_ident!("__key_{}", id);
            quote!(if !__live.replace(&#k,__editor.#id){return;})
        });
        Ok(quote!({
            let __node=#rt::identity::node().id;
            let __control_mount=__mount.control(__node,#symbol);
            let __action=#action{key:__scope_key.clone(),incarnation:__control_mount.incarnation,props:#capture{#(#props,)*}};
            let __live=__control_mount.clone();let __expected_key=__scope_key.clone();
            #(#keys)*
            let __handler=#rt::events::HandlerFn(move|__root:&mut #root,__event:#action,__cx:&mut #rt::core::ReducerContext<#root>|{
                if __event.key!=__expected_key||!__live.alive(__event.incarnation){return;}
                let Some(__state)=<#scope as #rt::Scope>::get_mut(__root,&__event.key)else{return;};
                #(#local_reads)*
                let mut __editor=#facade{__props:&__event.props,#(#local_values,)*};
                #name::#method(&mut __editor,__state,__cx);
                #(#local_writes)*
            });
            let __envelope=__build.bind_scoped(__node,#symbol,__action,__handler);
            #rt::identity::record(&__control_mount,__node,#symbol,&__envelope);
            #rt::Event::<#event>::new(__envelope)
        }))
    }

    fn effect_handler(&mut self, effect: &Effect, port: &str, symbol: &str) -> syn::Result<Tokens> {
        let rt = self.rt.clone();
        let name = self.component.clone();
        let scope = self.scope.clone();
        let target = self.target.clone();
        let root = self.root.clone();
        if port != "on_press" && port != "on_submit" {
            return Err(syn::Error::new_spanned(
                &effect.operation,
                "effects currently require a press-like event property",
            ));
        }
        if !self.symbols.insert(symbol.into()) {
            return Err(syn::Error::new_spanned(
                &effect.operation,
                "duplicate effect handler identity; give the control a distinct semantic key",
            ));
        }

        let editor = &self.editor;
        let state = &self.state;
        let cx = &self.cx;
        let mut used = BTreeSet::new();
        for argument in &effect.args {
            validate_effect_argument(argument, editor, state, cx)?;
            scan_fields(argument.to_token_stream(), editor, &mut used)?;
        }
        if let Some(start) = &effect.start {
            scan_fields(start.to_token_stream(), editor, &mut used)?;
        }
        scan_fields(effect.ok.to_token_stream(), editor, &mut used)?;
        scan_fields(effect.err.to_token_stream(), editor, &mut used)?;
        for field in &used {
            if !self
                .fields
                .iter()
                .any(|candidate| candidate.name == field.as_str())
            {
                return Err(syn::Error::new_spanned(
                    &effect.operation,
                    format!("{field} is not a declared prop or local field"),
                ));
            }
        }

        let captures: Vec<_> = self
            .fields
            .iter()
            .filter(|field| field.local.is_none() && used.contains(&field.name.to_string()))
            .collect();
        let locals: Vec<_> = self
            .fields
            .iter()
            .filter(|field| field.local.is_some() && used.contains(&field.name.to_string()))
            .collect();
        let suffix = format!("{:016x}", stable_hash(symbol));
        let request_action = format_ident!("__{}_EffectRequest_{}", name, suffix);
        let ok_action = format_ident!("__{}_EffectOk_{}", name, suffix);
        let err_action = format_ident!("__{}_EffectErr_{}", name, suffix);
        let capture = format_ident!("__{}_EffectCapture_{}", name, suffix);
        let facade = format_ident!("__{}_EffectEditor_{}", name, suffix);
        let start_method = format_ident!("__effect_start_{}", suffix);
        let ok_method = format_ident!("__effect_ok_{}", suffix);
        let err_method = format_ident!("__effect_err_{}", suffix);
        let ser_path = LitStr::new(
            &format!("{}::serde", rt.to_string().replace(' ', "")),
            Span::call_site(),
        );

        let mut definition = effect.operation.clone();
        let operation_name = definition
            .segments
            .last()
            .expect("effect operation path must not be empty")
            .ident
            .clone();
        definition.segments.pop();
        definition
            .segments
            .push(syn::PathSegment::from(format_ident!(
                "__fission_effect_{}",
                operation_name
            )));
        definition
            .segments
            .push(syn::PathSegment::from(format_ident!("Definition")));

        let prop_defs = captures.iter().map(|field| {
            let id = &field.name;
            let ty = captured_field_type(field);
            quote!(#id:#ty)
        });
        let local_defs = locals.iter().map(|field| {
            let id = &field.name;
            let ty = &field.ty;
            quote!(#id:#ty)
        });
        let action_definition = |action: &Ident| {
            quote! {
                #[allow(non_camel_case_types)]
                #[derive(Clone,#rt::serde::Serialize,#rt::serde::Deserialize)]
                #[serde(crate=#ser_path)]
                struct #action {
                    key:<#scope as #rt::Scope>::Key,
                    incarnation:u64,
                    props:#capture,
                }
                impl ::std::fmt::Debug for #action {
                    fn fmt(&self, formatter:&mut ::std::fmt::Formatter<'_>)->::std::fmt::Result {
                        formatter.write_str(stringify!(#action))
                    }
                }
                impl #rt::core::Action for #action {
                    fn static_id()->#rt::core::ActionId {
                        #rt::core::ActionId::from_name(concat!(module_path!(),"::",stringify!(#action)))
                    }
                }
            }
        };
        let request_action_definition = action_definition(&request_action);
        let ok_action_definition = action_definition(&ok_action);
        let err_action_definition = action_definition(&err_action);
        self.items.push(quote! {
            #[allow(non_camel_case_types)]
            #[derive(Clone,#rt::serde::Serialize,#rt::serde::Deserialize)]
            #[serde(crate=#ser_path)]
            struct #capture { #(#prop_defs,)* }
            #request_action_definition
            #ok_action_definition
            #err_action_definition
            #[allow(non_camel_case_types)]
            struct #facade<'a> { __props:&'a #capture, #(#local_defs,)* }
            impl ::std::ops::Deref for #facade<'_> {
                type Target=#capture;
                fn deref(&self)->&Self::Target { self.__props }
            }
        });

        let request = match effect.args.len() {
            0 => quote!(()),
            1 => {
                let argument = effect.args.first().expect("one effect argument");
                quote!(#argument)
            }
            _ => {
                let arguments = &effect.args;
                quote!((#arguments))
            }
        };
        let start = effect.start.as_ref();
        let ok_pattern = &effect.ok_pattern;
        let ok = &effect.ok;
        let err_pattern = &effect.err_pattern;
        let err = &effect.err;
        self.methods.push(quote! {
            #[allow(unused_variables)]
            fn #start_method(
                #editor:&mut #facade<'_>,
                #state:&mut #target,
                #cx:&mut #rt::core::ReducerContext<#root>,
            ) -> <#definition as #rt::core::AppEffect>::Request {
                let __request = #request;
                #start
                __request
            }
            #[allow(unused_variables)]
            fn #ok_method(
                #editor:&mut #facade<'_>,
                #state:&mut #target,
                #cx:&mut #rt::core::ReducerContext<#root>,
                __value:<#definition as #rt::core::AppEffect>::Ok,
            ) {
                let #ok_pattern = __value;
                #ok
            }
            #[allow(unused_variables)]
            fn #err_method(
                #editor:&mut #facade<'_>,
                #state:&mut #target,
                #cx:&mut #rt::core::ReducerContext<#root>,
                __error:<#definition as #rt::core::AppEffect>::Err,
            ) {
                let #err_pattern = __error;
                #err
            }
        });

        let props = captures.iter().map(|field| {
            let id = &field.name;
            let value = captured_field_value(field, editor);
            quote!(#id: #value)
        });
        let keys = locals.iter().map(|field| {
            let id = &field.name;
            let handle = handle_name(id);
            let key = format_ident!("__key_{}", id);
            quote!(let #key=#handle.key().clone();)
        });
        let local_reads = locals.iter().map(|field| {
            let id = &field.name;
            let ty = &field.ty;
            let key = format_ident!("__key_{}", id);
            quote!(let Some(#id)=__live.snapshot::<#ty>(&#key)else{return;};)
        });
        let local_values = locals.iter().map(|field| &field.name);
        let local_writes = locals.iter().map(|field| {
            let id = &field.name;
            let key = format_ident!("__key_{}", id);
            quote!(if !__live.replace(&#key,__editor.#id){return;})
        });
        let ok_local_reads = locals.iter().map(|field| {
            let id = &field.name;
            let ty = &field.ty;
            let key = format_ident!("__ok_key_{}", id);
            quote!(let Some(#id)=__ok_live.snapshot::<#ty>(&#key)else{return;};)
        });
        let err_local_reads = locals.iter().map(|field| {
            let id = &field.name;
            let ty = &field.ty;
            let key = format_ident!("__err_key_{}", id);
            quote!(let Some(#id)=__err_live.snapshot::<#ty>(&#key)else{return;};)
        });
        let ok_local_values = local_values.clone();
        let err_local_values = local_values.clone();
        let ok_local_writes = locals.iter().map(|field| {
            let id = &field.name;
            let key = format_ident!("__ok_key_{}", id);
            quote!(if !__ok_live.replace(&#key,__editor.#id){return;})
        });
        let err_local_writes = locals.iter().map(|field| {
            let id = &field.name;
            let key = format_ident!("__err_key_{}", id);
            quote!(if !__err_live.replace(&#key,__editor.#id){return;})
        });
        let callback_keys = locals.iter().map(|field| {
            let key = format_ident!("__key_{}", field.name);
            let ok_key = format_ident!("__ok_key_{}", field.name);
            let err_key = format_ident!("__err_key_{}", field.name);
            quote!(let #ok_key=#key.clone(); let #err_key=#key.clone();)
        });

        Ok(quote!({
            let __node=#rt::identity::node().id;
            let __control_mount=__mount.control(__node,#symbol);
            let __action=#request_action {
                key:__scope_key.clone(),
                incarnation:__control_mount.incarnation,
                props:#capture { #(#props,)* },
            };
            let __live=__control_mount.clone();
            let __expected_key=__scope_key.clone();
            #(#keys)*
            let __handler=#rt::events::HandlerFn(move|__root:&mut #root,__event:#request_action,__cx:&mut #rt::core::ReducerContext<#root>|{
                if __event.key!=__expected_key||!__live.alive(__event.incarnation){return;}
                let Some(__state)=<#scope as #rt::Scope>::get_mut(__root,&__event.key)else{return;};
                #(#local_reads)*
                let mut __editor=#facade { __props:&__event.props, #(#local_values,)* };
                let __request=#name::#start_method(&mut __editor,__state,__cx);
                #(#local_writes)*

                #(#callback_keys)*
                let __ok_live=::std::clone::Clone::clone(&__live);
                let __ok_expected=__event.key.clone();
                let __ok_handler=#rt::events::HandlerFn(move|__root:&mut #root,__event:#ok_action,__cx:&mut #rt::core::ReducerContext<#root>|{
                    if __event.key!=__ok_expected||!__ok_live.alive(__event.incarnation){return;}
                    let Some(__state)=<#scope as #rt::Scope>::get_mut(__root,&__event.key)else{return;};
                    #(#ok_local_reads)*
                    let mut __editor=#facade { __props:&__event.props, #(#ok_local_values,)* };
                    let __job=#rt::core::JobRef::<#rt::core::AppEffectJob<#definition>>::new(<#definition as #rt::core::AppEffect>::NAME);
                    let Some(__value)=__cx.input.job_ok(__job)else{return;};
                    #name::#ok_method(&mut __editor,__state,__cx,__value);
                    #(#ok_local_writes)*
                });
                let __err_live=::std::clone::Clone::clone(&__live);
                let __err_expected=__event.key.clone();
                let __err_handler=#rt::events::HandlerFn(move|__root:&mut #root,__event:#err_action,__cx:&mut #rt::core::ReducerContext<#root>|{
                    if __event.key!=__err_expected||!__err_live.alive(__event.incarnation){return;}
                    let Some(__state)=<#scope as #rt::Scope>::get_mut(__root,&__event.key)else{return;};
                    #(#err_local_reads)*
                    let mut __editor=#facade { __props:&__event.props, #(#err_local_values,)* };
                    let __job=#rt::core::JobRef::<#rt::core::AppEffectJob<#definition>>::new(<#definition as #rt::core::AppEffect>::NAME);
                    let Some(__error)=__cx.input.job_err(__job)else{return;};
                    #name::#err_method(&mut __editor,__state,__cx,__error);
                    #(#err_local_writes)*
                });
                let __ok=__cx.effects.bind(
                    #ok_action { key:__event.key.clone(), incarnation:__event.incarnation, props:__event.props.clone() },
                    __ok_handler,
                );
                let __err=__cx.effects.bind(
                    #err_action { key:__event.key, incarnation:__event.incarnation, props:__event.props },
                    __err_handler,
                );
                __cx.effects.app_effect::<#definition>(__request).on_ok(__ok).on_err(__err);
            });
            let __envelope=__build.bind_scoped(__node,#symbol,__action,__handler);
            #rt::identity::record(&__control_mount,__node,#symbol,&__envelope);
            #rt::Event::<#rt::events::Press>::new(__envelope)
        }))
    }
}

fn call_shape(call: &Call) -> String {
    let mut shape = call.path.to_token_stream().to_string();
    shape.push('(');
    for argument in &call.args {
        if let Some(binding) = &argument.binding {
            shape.push_str("bind(");
            shape.push_str(&binding.port.to_string());
            if let Some(event) = &binding.event {
                shape.push_str(" as ");
                shape.push_str(&event.to_token_stream().to_string());
            }
            shape.push_str("):");
        }
        if argument.name.as_ref().is_some_and(|name| name == "key") {
            continue;
        }
        if let Some(name) = &argument.name {
            shape.push_str(&name.to_string());
            shape.push(':');
        }
        shape.push_str(&value_shape(&argument.value));
        shape.push(',');
    }
    for modifier in &call.modifiers {
        shape.push('.');
        shape.push_str(&modifier.name.to_string());
        shape.push('(');
        shape.push_str(
            &modifier
                .args
                .iter()
                .map(ToTokens::to_token_stream)
                .map(|tokens| tokens.to_string())
                .collect::<Vec<_>>()
                .join(","),
        );
        shape.push(')');
    }
    shape.push(')');
    shape
}

fn value_shape(value: &Value) -> String {
    match value {
        Value::Effect(effect) => format!(
            "effect:{}({})",
            effect.operation.to_token_stream(),
            effect
                .args
                .iter()
                .map(ToTokens::to_token_stream)
                .map(|tokens| tokens.to_string())
                .collect::<Vec<_>>()
                .join(","),
        ),
        Value::Reduce(block) => format!("reduce:{}", block.to_token_stream()),
        Value::Call(call) => call_shape(call),
        Value::Rust(expr)
        | Value::ExplicitRust(expr)
        | Value::Read(expr)
        | Value::Source(expr)
        | Value::To(expr) => expr.to_token_stream().to_string(),
        Value::Optional(value) => format!("Some({})", value_shape(value)),
        Value::Array(values) => format!(
            "[{}]",
            values.iter().map(value_shape).collect::<Vec<_>>().join(",")
        ),
        Value::If { cond, yes, no } => format!(
            "if:{}:then:{}:else:{}",
            cond.to_token_stream(),
            value_shape(yes),
            no.as_deref().map(value_shape).unwrap_or_default(),
        ),
        Value::For { pat, iter, body } => format!(
            "for:{}:in:{}:{}",
            pat.to_token_stream(),
            iter.to_token_stream(),
            value_shape(body),
        ),
        Value::Match { expr, arms } => {
            let arms = arms
                .iter()
                .map(|(pattern, guard, body)| {
                    format!(
                        "{}:{}:{}",
                        pattern.to_token_stream(),
                        guard
                            .as_ref()
                            .map(ToTokens::to_token_stream)
                            .unwrap_or_default(),
                        value_shape(body),
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("match:{}:{{{arms}}}", expr.to_token_stream())
        }
    }
}

fn default_edit_event(port: &Ident) -> syn::Result<Tokens> {
    let rt = crate::support();
    match port.to_string().as_str() {
        "on_input" => Ok(quote!(#rt::events::TextChanged)),
        "on_toggle" => Ok(quote!(#rt::events::Toggle)),
        _ => Err(syn::Error::new_spanned(
            port,
            "custom event bindings must name their edit event type, for example bind(on_update as MyUpdate): to(state.value)",
        )),
    }
}

fn place(expr: &Expr, root: &Ident) -> Option<Vec<Ident>> {
    fn walk(expr: &Expr, root: &Ident, out: &mut Vec<Ident>) -> bool {
        match expr {
            Expr::Path(p) => p.qself.is_none() && p.path.is_ident(root),
            Expr::Field(f) => {
                if !walk(&f.base, root, out) {
                    return false;
                }
                if let Member::Named(n) = &f.member {
                    out.push(n.clone());
                    true
                } else {
                    false
                }
            }
            Expr::Paren(p) => walk(&p.expr, root, out),
            _ => false,
        }
    }
    let mut out = Vec::new();
    if walk(expr, root, &mut out) && !out.is_empty() {
        Some(out)
    } else {
        None
    }
}
fn borrowed_read(expr: &Expr, editor: &Ident, state: &Ident) -> Tokens {
    if place(expr, editor).is_some() || place(expr, state).is_some() {
        quote!(::std::clone::Clone::clone(&(#expr)))
    } else {
        quote!(#expr)
    }
}
fn string_literal(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Lit(l) => match &l.lit {
            Lit::Str(s) => Some(s.value()),
            _ => None,
        },
        _ => None,
    }
}

fn property_expression(expr: &Expr, editor: &Ident, state: &Ident) -> Tokens {
    if unsuffixed_float_expression(expr) {
        quote!((#expr) as f32)
    } else {
        borrowed_read(expr, editor, state)
    }
}

fn unsuffixed_float_expression(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(literal) => match &literal.lit {
            Lit::Float(float) => float.suffix().is_empty(),
            _ => false,
        },
        Expr::Binary(binary) => {
            numeric_literal_expression(&binary.left)
                && numeric_literal_expression(&binary.right)
                && (contains_float_literal(&binary.left) || contains_float_literal(&binary.right))
        }
        Expr::Group(group) => unsuffixed_float_expression(&group.expr),
        Expr::Paren(paren) => unsuffixed_float_expression(&paren.expr),
        Expr::Unary(unary) => unsuffixed_float_expression(&unary.expr),
        _ => false,
    }
}

fn numeric_literal_expression(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(literal) => matches!(literal.lit, Lit::Float(_) | Lit::Int(_)),
        Expr::Binary(binary) => {
            numeric_literal_expression(&binary.left) && numeric_literal_expression(&binary.right)
        }
        Expr::Group(group) => numeric_literal_expression(&group.expr),
        Expr::Paren(paren) => numeric_literal_expression(&paren.expr),
        Expr::Unary(unary) => numeric_literal_expression(&unary.expr),
        _ => false,
    }
}

fn contains_float_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(literal) => {
            matches!(&literal.lit, Lit::Float(float) if float.suffix().is_empty())
        }
        Expr::Binary(binary) => {
            contains_float_literal(&binary.left) || contains_float_literal(&binary.right)
        }
        Expr::Group(group) => contains_float_literal(&group.expr),
        Expr::Paren(paren) => contains_float_literal(&paren.expr),
        Expr::Unary(unary) => contains_float_literal(&unary.expr),
        _ => false,
    }
}
fn parameter(p: &Pat) -> syn::Result<Option<Ident>> {
    match p {
        Pat::Wild(_) => Ok(None),
        Pat::Ident(i) if i.by_ref.is_none() && i.subpat.is_none() && i.mutability.is_none() => {
            Ok(Some(i.ident.clone()))
        }
        _ => Err(syn::Error::new_spanned(
            p,
            "handler parameters must be identifiers or _",
        )),
    }
}
fn stable_hash(text: &str) -> u64 {
    // Symbol mangling only; runtime identity uses Fission ActionId.
    text.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    })
}

fn strip_expression_wrappers(mut expr: &Expr) -> &Expr {
    loop {
        expr = match expr {
            Expr::Group(group) => &group.expr,
            Expr::Paren(paren) => &paren.expr,
            _ => return expr,
        };
    }
}

fn expression_root(expr: &Expr) -> Option<&Ident> {
    match strip_expression_wrappers(expr) {
        Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            path.path.get_ident()
        }
        Expr::Field(field) => expression_root(&field.base),
        Expr::Index(index) => expression_root(&index.expr),
        Expr::MethodCall(call) => expression_root(&call.receiver),
        Expr::Reference(reference) => expression_root(&reference.expr),
        _ => None,
    }
}

fn validate_effect_argument(
    argument: &Expr,
    editor: &Ident,
    state: &Ident,
    cx: &Ident,
) -> syn::Result<()> {
    let argument = strip_expression_wrappers(argument);
    if matches!(argument, Expr::Path(path) if path.path.is_ident(editor)) {
        return Err(syn::Error::new_spanned(
            argument,
            format!(
                "an effect cannot receive the component binding `{editor}`; pass an owned snapshot, such as `{editor}.count` or `{editor}.name.clone()`"
            ),
        ));
    }
    if matches!(argument, Expr::Path(path) if path.path.is_ident(state)) {
        return Err(syn::Error::new_spanned(
            argument,
            format!(
                "an effect cannot receive the global-state binding `{state}`; select the owned values required by the operation, such as `{state}.session.account_id.clone()`"
            ),
        ));
    }

    let mut check = EffectArgumentCheck {
        editor,
        state,
        cx,
        error: None,
    };
    syn::visit::Visit::visit_expr(&mut check, argument);
    check.error.map_or(Ok(()), Err)
}

struct EffectArgumentCheck<'a> {
    editor: &'a Ident,
    state: &'a Ident,
    cx: &'a Ident,
    error: Option<syn::Error>,
}

impl<'ast> syn::visit::Visit<'ast> for EffectArgumentCheck<'_> {
    fn visit_expr(&mut self, expr: &'ast Expr) {
        if self.error.is_some() {
            return;
        }
        if matches!(expr, Expr::Path(path) if path.path.is_ident(self.cx)) {
            self.error = Some(syn::Error::new_spanned(
                expr,
                format!(
                    "an effect cannot receive the reducer context `{}`; use `{}` inside on_start, on_ok, or on_err",
                    self.cx, self.cx,
                ),
            ));
            return;
        }
        syn::visit::visit_expr(self, expr);
    }

    fn visit_expr_reference(&mut self, reference: &'ast syn::ExprReference) {
        if self.error.is_some() {
            return;
        }
        if expression_root(&reference.expr)
            .is_some_and(|root| root == self.editor || root == self.state)
        {
            self.error = Some(syn::Error::new_spanned(
                reference,
                "effect arguments must be owned; clone this value instead of passing a reference across the asynchronous effect boundary",
            ));
            return;
        }
        syn::visit::visit_expr_reference(self, reference);
    }
}

fn scan_fields(tokens: Tokens, editor: &Ident, out: &mut BTreeSet<String>) -> syn::Result<()> {
    let tokens: Vec<_> = tokens.into_iter().collect();
    for (i, t) in tokens.iter().enumerate() {
        if let TokenTree::Group(g) = t {
            scan_fields(g.stream(), editor, out)?;
        }
        if matches!(t,TokenTree::Ident(id) if id==editor) {
            if let (Some(TokenTree::Punct(dot)), Some(TokenTree::Ident(field))) =
                (tokens.get(i + 1), tokens.get(i + 2))
            {
                if dot.as_char() == '.' {
                    out.insert(field.to_string());
                    continue;
                }
            }
            return Err(syn::Error::new(
                t.span(),
                "event editor access must name a declared field, e.g. editor.session or editor.advanced; no implicit whole-widget capture",
            ));
        }
    }
    Ok(())
}
struct HandlerCheck {
    params: Vec<Ident>,
    error: Option<syn::Error>,
}
impl<'ast> syn::visit::Visit<'ast> for HandlerCheck {
    fn visit_expr_await(&mut self, e: &'ast syn::ExprAwait) {
        self.error = Some(syn::Error::new_spanned(
            e,
            "a component handler is synchronous; queue a Fission effect/job instead of awaiting",
        ));
    }
    fn visit_expr_async(&mut self, e: &'ast syn::ExprAsync) {
        self.error = Some(syn::Error::new_spanned(
            e,
            "an async block in an extracted handler is unsupported; use an ordinary effect/job helper",
        ));
    }
    fn visit_pat_ident(&mut self, p: &'ast syn::PatIdent) {
        if self.params.contains(&p.ident) {
            self.error = Some(syn::Error::new_spanned(
                p,
                "do not shadow an inline handler's editor/state/context parameters",
            ));
        }
        syn::visit::visit_pat_ident(self, p);
    }
}
