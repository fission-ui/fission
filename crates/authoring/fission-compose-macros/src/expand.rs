use crate::ast::{Call, Component, Value};
use proc_macro2::{Span, TokenStream as Tokens, TokenTree};
use quote::{format_ident, quote, ToTokens};
use std::collections::{BTreeSet, HashSet};
use syn::{Expr, Ident, Lit, LitStr, Member, Pat};

pub fn component(input: Component) -> syn::Result<Tokens> {
    let rt = crate::support();
    let Component {
        vis,
        name,
        target,
        scope,
        fields,
        editor,
        state,
        cx,
        body,
    } = input;
    let scope_ty = match &scope {
        Some((p, _)) => quote!(#p),
        None => quote!(<#target as #rt::DefaultScope>::Scope),
    };
    let key = scope
        .as_ref()
        .map(|(_, e)| quote!(::std::clone::Clone::clone(&(#e))))
        .unwrap_or(quote!(()));
    let root = quote!(<#scope_ty as #rt::Scope>::Root);
    let props: Vec<_> = fields.iter().filter(|f| f.local.is_none()).collect();
    let locals: Vec<_> = fields.iter().filter(|f| f.local.is_some()).collect();
    let view_ty = format_ident!("__{}View", name);
    let name_string = quote!(concat!(module_path!(), "::", stringify!(#name)));
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
    let default_impl = props.is_empty().then(|| {
        quote!(
            impl ::std::default::Default for #name{fn default()->Self{Self{}}}
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
        fields: &fields,
        items: Vec::new(),
        methods: Vec::new(),
        symbols: HashSet::new(),
    };
    let tree = emit.bare(&body, "view")?;
    let extra_items = emit.items;
    let methods = emit.methods;
    Ok(quote! {
        #vis struct #name {
            #(#prop_fields,)*
        }
        #[doc(hidden)]
        struct #view_ty<'__a>{__props:&'__a #name,#(#local_defs,)*}
        impl ::std::ops::Deref for #view_ty<'_>{
            type Target=#name;fn deref(&self)->&Self::Target{self.__props}
        }
        impl #name {
            pub fn new(#(#prop_args),*)->Self{Self{#(#prop_init,)*}}
            #(#methods)*
        }
        #default_impl
        #(#extra_items)*
        impl From<#name> for #rt::core::Widget {
            #[allow(unused_variables)]
            fn from(__component:#name)->Self{
                let #editor=&__component;
                let __scope_key:<#scope_ty as #rt::Scope>::Key=#key;
                let (__build,__view)=#rt::core::build::current::<#root>();
                let #state:&#target=match <#scope_ty as #rt::Scope>::get(__view.state(),&__scope_key){
                    Some(state)=>state,
                    None=>panic!(
                        "Fission Compose component `{}` could not resolve its state scope",
                        #name_string,
                    ),
                };
                #rt::identity::BuildSession::run(||{
                    let __base=#rt::identity::current_id();
                    let __owner=#rt::identity::child_id(__base,#name_string,&__scope_key);
                    #rt::identity::BuildSession::claim(__owner,#name_string);
                    #rt::core::build::with_widget_id(__owner,||{
                        let __mount=#rt::Mount::new(#name_string,__view);
                        #(#local_handles)*
                        let #editor=#view_ty{__props:&__component,#(#local_values,)*};
                        let #cx=#rt::view_cx::ViewCx{build:__build,view:__view};
                        let _=(&#editor,&#state,&#cx);
                        let __tree:#rt::core::Widget=(#tree).into();
                        __tree
                    })
                })
            }
        }
    })
}
fn handle_name(id: &Ident) -> Ident {
    format_ident!("__local_{}", id)
}

struct Emitter<'a> {
    rt: Tokens,
    component: Ident,
    scope: Tokens,
    target: Tokens,
    root: Tokens,
    editor: Ident,
    state: Ident,
    fields: &'a [crate::ast::Field],
    items: Vec<Tokens>,
    methods: Vec<Tokens>,
    symbols: HashSet<String>,
}
impl Emitter<'_> {
    fn bare(&mut self, v: &Value, path: &str) -> syn::Result<Tokens> {
        let rt = self.rt.clone();
        match v {
            Value::Call(c) => self.call(c, path),
            Value::Rust(e) | Value::ExplicitRust(e) | Value::Read(e) | Value::Source(e) => {
                Ok(quote!(#e))
            }
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
            Value::If { cond, yes, no } => {
                // Conditional branches share the same semantic namespace. This lets
                // a keyed component intentionally survive switching between branches.
                let yes = self.bare(yes, path)?;
                if let Some(no) = no {
                    let no = self.bare(no, path)?;
                    Ok(quote!(if #cond{#yes}else{#no}))
                } else {
                    Ok(quote!(if #cond{Some(#yes)}else{None}))
                }
            }
            Value::For { .. } => {
                let p = self.append(v, path)?;
                Ok(quote!({let mut __children:Vec<#rt::core::Widget>=Vec::new();#p __children}))
            }
            Value::Match { expr, arms } => {
                let mut out = Vec::new();
                for (pat, guard, value) in arms {
                    let guard = guard.as_ref().map(|g| quote!(if #g));
                    let body = self.bare(value, path)?;
                    out.push(quote!(#pat #guard=>{#body}));
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
                    return Err(syn::Error::new_spanned(iter,"a dynamic child must be a component/control with an explicit key: for item in items { Row(key: item.id, ...) }"));
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
        Err(syn::Error::new_spanned(expr,"bind requires a directly writable state or local field; use source(existing_binding) for a custom binding"))
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
            borrowed_read(expr, &self.editor, &self.state)
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
                fields.push(quote!(
                    #port:#rt::identity::in_slot(#property,||
                        #rt::binding::update_for::<_,#event,_>(#property,#source)
                    )
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
                positional.push(self.value(&arg.value, &argument_path)?);
                continue;
            };
            let argument_path = format!("{path}/{property}");
            let value = if property == "bind" {
                self.binding(&arg.value)?
            } else if property.starts_with("on_") {
                match arg.value.rust() {
                    Some(Expr::Closure(closure)) => {
                        let handler_name=c.named("handler").and_then(Value::rust).and_then(string_literal).or_else(||static_key.clone())
                            .ok_or_else(||syn::Error::new_spanned(closure,"an inline handler needs a literal key: \"save\"; with a dynamic control key, add handler: \"save\""))?;
                        self.handler(
                            closure,
                            &property,
                            &format!("{parent_path}/{widget_name}/{handler_name}/{property}"),
                        )?
                    }
                    Some(e) => quote!(#e),
                    None => {
                        return Err(syn::Error::new_spanned(
                            widget,
                            "an event takes an inline handler or an already-typed Event value",
                        ))
                    }
                }
            } else {
                self.value(&arg.value, &argument_path)?
            };
            // Argument values are built in a named slot, never a declaration ordinal.
            if property == "bind" {
                bindings.push(quote!(let __widget=#rt::ComposeBind::compose_bind(__widget,#rt::identity::in_slot(#property,||#value));));
            } else if property.starts_with("on_") {
                fields.push(
                    quote!(#method:Some((#rt::identity::in_slot(#property,||#value)).into())),
                );
            } else {
                fields.push(quote!(#method:(#rt::identity::in_slot(#property,||#value)).into()));
            }
        }
        let base = if positional.is_empty() {
            quote!(::std::default::Default::default())
        } else {
            quote!(#widget::new(#(#positional),*))
        };
        Ok(quote!({
            let __key=#key;
            #rt::identity::with_node(stringify!(#widget),&__key,||{
                let __widget=#widget{#(#fields,)*..#base};
                #(#bindings)*
                let __widget:#rt::core::Widget=__widget.into();
                #rt::core::ui::WidgetIdExt::id(__widget,#rt::identity::node().id)
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
            return Err(syn::Error::new_spanned(closure,"inline handlers use |editor, state, cx| { ... }; no move, async, return-type annotation, or captured render locals"));
        }
        let args: Vec<_> = closure
            .inputs
            .iter()
            .map(parameter)
            .collect::<syn::Result<_>>()?;
        let event=match port{
            "on_press"|"on_submit"=>quote!(#rt::events::Press),
            "on_input"=>quote!(#rt::events::TextChanged),
            "on_toggle"=>quote!(#rt::events::Toggle),
            _=>return Err(syn::Error::new_spanned(closure,"this prototype supports inline on_press, on_submit, on_input and on_toggle; pass a typed Event for another event contract")),
        };
        if !self.symbols.insert(symbol.into()) {
            return Err(syn::Error::new_spanned(closure,"duplicate inline handler identity; give the control or handler a distinct semantic name"));
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
                return Err(syn::Error::new_spanned(closure,format!("{field} is not a declared prop or local field; event editor access is limited to declared fields")));
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
            let ty = &f.ty;
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
            quote!(#id: ::std::clone::Clone::clone(&#editor.#id))
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
    shape.push(')');
    shape
}

fn value_shape(value: &Value) -> String {
    match value {
        Value::Call(call) => call_shape(call),
        Value::Rust(expr)
        | Value::ExplicitRust(expr)
        | Value::Read(expr)
        | Value::Source(expr)
        | Value::To(expr) => expr.to_token_stream().to_string(),
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
            return Err(syn::Error::new(t.span(),"event editor access must name a declared field, e.g. editor.session or editor.advanced; no implicit whole-widget capture"));
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
        self.error=Some(syn::Error::new_spanned(e,"an async block in an extracted handler is unsupported; use an ordinary effect/job helper"));
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
