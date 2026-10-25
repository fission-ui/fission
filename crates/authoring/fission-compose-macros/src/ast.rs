use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Attribute, Block, Expr, Ident, Pat, Path, Token, Type, Visibility,
};
mod kw {
    syn::custom_keyword!(effect);
    syn::custom_keyword!(on_err);
    syn::custom_keyword!(on_ok);
    syn::custom_keyword!(on_start);
    syn::custom_keyword!(props);
    syn::custom_keyword!(reduce);
    syn::custom_keyword!(start);
    syn::custom_keyword!(state);
    syn::custom_keyword!(using);
    syn::custom_keyword!(view);
    syn::custom_keyword!(rust);
}

pub struct Field {
    pub attrs: Vec<Attribute>,
    pub name: Ident,
    pub ty: Type,
    pub local: Option<Expr>,
}
pub struct Component {
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub name: Ident,
    pub target: Type,
    pub scope: Option<(Path, Expr)>,
    pub fields: Vec<Field>,
    pub editor: Ident,
    pub state: Ident,
    pub cx: Ident,
    pub setup: Option<Block>,
    pub body: Value,
}
impl Parse for Component {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let vis = input.parse()?;
        let name = input.parse()?;
        input.parse::<Token![<]>()?;
        let target = input.parse()?;
        input.parse::<Token![>]>()?;
        let scope = if input.peek(kw::using) {
            input.parse::<kw::using>()?;
            let p = input.parse()?;
            let args;
            syn::parenthesized!(args in input);
            let key = args.parse()?;
            if !args.is_empty() {
                return Err(args.error("using takes one state-scope key expression"));
            }
            Some((p, key))
        } else {
            None
        };
        let mut fields = Vec::new();
        if input.peek(syn::token::Brace) {
            let fs;
            syn::braced!(fs in input);
            if fs.peek(kw::props) || fs.peek(kw::state) {
                let mut saw_props = false;
                let mut saw_state = false;
                while !fs.is_empty() {
                    let local = if fs.peek(kw::props) {
                        fs.parse::<kw::props>()?;
                        if saw_props {
                            return Err(fs.error("duplicate props section"));
                        }
                        saw_props = true;
                        false
                    } else if fs.peek(kw::state) {
                        fs.parse::<kw::state>()?;
                        if saw_state {
                            return Err(fs.error("duplicate state section"));
                        }
                        saw_state = true;
                        true
                    } else {
                        return Err(fs.error("expected props { ... } or state { ... }"));
                    };
                    let section;
                    syn::braced!(section in fs);
                    parse_section_fields(&section, &mut fields, local)?;
                    if fs.peek(Token![,]) {
                        fs.parse::<Token![,]>()?;
                    }
                }
            } else {
                parse_legacy_fields(&fs, &mut fields)?;
            }
        }
        let (editor, state, cx) = if input.peek(kw::view) {
            input.parse::<kw::view>()?;
            let bindings;
            syn::parenthesized!(bindings in input);
            let editor = bindings.parse()?;
            bindings.parse::<Token![,]>()?;
            let state = bindings.parse()?;
            bindings.parse::<Token![,]>()?;
            let cx = bindings.parse()?;
            if !bindings.is_empty() {
                return Err(bindings.error("view takes component, state and context bindings"));
            }
            (editor, state, cx)
        } else {
            input.parse::<Token![|]>()?;
            let editor = input.parse()?;
            input.parse::<Token![,]>()?;
            let state = input.parse()?;
            input.parse::<Token![,]>()?;
            let cx = input.parse()?;
            input.parse::<Token![|]>()?;
            (editor, state, cx)
        };
        let setup = if input.peek(syn::token::Brace) {
            Some(input.parse()?)
        } else {
            None
        };
        input.parse::<Token![=>]>()?;
        let body = input.parse()?;
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
        if input.peek(Token![;]) {
            input.parse::<Token![;]>()?;
        }
        if !input.is_empty() {
            return Err(input.error("unexpected tokens after component view"));
        }
        if editor == state || editor == cx || state == cx {
            return Err(syn::Error::new_spanned(
                &editor,
                "editor, state and context bindings must have distinct names",
            ));
        }
        Ok(Self {
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
        })
    }
}

fn validate_field(fields: &[Field], name: &Ident) -> syn::Result<()> {
    if fields.iter().any(|field| field.name == *name) {
        return Err(syn::Error::new(name.span(), "duplicate component field"));
    }
    if name.to_string().starts_with("__") {
        return Err(syn::Error::new(
            name.span(),
            "names beginning __ are reserved for generated implementation details",
        ));
    }
    Ok(())
}

fn parse_section_fields(
    input: ParseStream<'_>,
    fields: &mut Vec<Field>,
    local: bool,
) -> syn::Result<()> {
    while !input.is_empty() {
        let attrs = input.call(Attribute::parse_outer)?;
        let name: Ident = input.parse()?;
        validate_field(fields, &name)?;
        input.parse::<Token![:]>()?;
        let ty = input.parse()?;
        let default = if local {
            input.parse::<Token![=]>()?;
            Some(input.parse()?)
        } else {
            None
        };
        fields.push(Field {
            attrs,
            name,
            ty,
            local: default,
        });
        if input.is_empty() {
            break;
        }
        input.parse::<Token![,]>()?;
    }
    Ok(())
}

fn parse_legacy_fields(input: ParseStream<'_>, fields: &mut Vec<Field>) -> syn::Result<()> {
    while !input.is_empty() {
        let mut attrs = input.call(Attribute::parse_outer)?;
        let name: Ident = input.parse()?;
        validate_field(fields, &name)?;
        input.parse::<Token![:]>()?;
        let ty = input.parse()?;
        let mut local = None;
        if let Some(index) = attrs
            .iter()
            .position(|attribute| attribute.path().is_ident("local_state"))
        {
            let attribute = attrs.remove(index);
            attribute.parse_nested_meta(|meta| {
                if !meta.path.is_ident("default") {
                    return Err(meta.error("expected default = expression"));
                }
                if local.is_some() {
                    return Err(meta.error("duplicate local default"));
                }
                local = Some(meta.value()?.parse()?);
                Ok(())
            })?;
            if local.is_none() {
                return Err(syn::Error::new_spanned(
                    attribute,
                    "local_state requires default = expression",
                ));
            }
        }
        fields.push(Field {
            attrs,
            name,
            ty,
            local,
        });
        if input.is_empty() {
            break;
        }
        input.parse::<Token![,]>()?;
    }
    Ok(())
}
#[derive(Clone)]
pub struct Arg {
    pub name: Option<Ident>,
    pub binding: Option<Binding>,
    pub value: Value,
}
#[derive(Clone)]
pub struct Binding {
    pub port: Ident,
    pub event: Option<Path>,
}
#[derive(Clone)]
pub struct Call {
    pub path: Path,
    pub args: Vec<Arg>,
    pub modifiers: Vec<Modifier>,
}
#[derive(Clone)]
pub struct Modifier {
    pub name: Ident,
    pub args: Punctuated<Expr, Token![,]>,
}

#[derive(Clone)]
pub struct Effect {
    pub operation: Path,
    pub args: Punctuated<Expr, Token![,]>,
    pub start: Option<Block>,
    pub ok_pattern: Pat,
    pub ok: Block,
    pub err_pattern: Pat,
    pub err: Block,
}
#[derive(Clone)]
pub struct ReduceCapture {
    pub name: Ident,
    pub ty: Type,
    pub value: Expr,
}
#[derive(Clone)]
pub struct Reduce {
    pub name: Option<Ident>,
    pub captures: Vec<ReduceCapture>,
    pub body: Block,
}
impl Call {
    pub fn named(&self, name: &str) -> Option<&Value> {
        self.args
            .iter()
            .find(|a| a.name.as_ref().is_some_and(|i| i == name))
            .map(|a| &a.value)
    }
}
#[derive(Clone)]
pub enum Value {
    Effect(Effect),
    Reduce(Reduce),
    Call(Call),
    Array(Vec<Value>),
    Optional(Box<Value>),
    If {
        cond: Expr,
        yes: Box<Value>,
        no: Option<Box<Value>>,
    },
    For {
        pat: Pat,
        iter: Expr,
        body: Box<Value>,
    },
    Match {
        expr: Expr,
        arms: Vec<(Pat, Option<Expr>, Value)>,
    },
    Rust(Expr),
    ExplicitRust(Expr),
    Read(Expr),
    Source(Expr),
    To(Expr),
}
impl Value {
    pub fn rust(&self) -> Option<&Expr> {
        match self {
            Self::Rust(e)
            | Self::ExplicitRust(e)
            | Self::Read(e)
            | Self::Source(e)
            | Self::To(e) => Some(e),
            _ => None,
        }
    }

    pub fn is_plain_rust(&self) -> bool {
        match self {
            Self::Rust(expr) => !is_bare_component(expr),
            Self::ExplicitRust(_) | Self::Read(_) | Self::Source(_) | Self::To(_) => true,
            Self::If { yes, no, .. } => {
                yes.is_plain_rust() && no.as_deref().is_none_or(Self::is_plain_rust)
            }
            Self::Optional(value) => value.is_plain_rust(),
            Self::Match { arms, .. } => arms.iter().all(|(_, _, body)| body.is_plain_rust()),
            Self::Effect(_)
            | Self::Reduce(_)
            | Self::Call(_)
            | Self::Array(_)
            | Self::For { .. } => false,
        }
    }
}

fn is_bare_component(expr: &Expr) -> bool {
    let Expr::Path(path) = expr else {
        return false;
    };
    if path.path.is_ident("None") {
        return false;
    }
    if path.qself.is_some() || path.path.segments.len() != 1 {
        return false;
    }
    path.path
        .segments
        .first()
        .and_then(|segment| segment.ident.to_string().chars().next())
        .is_some_and(char::is_uppercase)
}
fn branch(input: ParseStream<'_>) -> syn::Result<Value> {
    let body;
    syn::braced!(body in input);
    let result = body.parse()?;
    if body.peek(Token![,]) {
        body.parse::<Token![,]>()?;
    }
    if !body.is_empty() {
        return Err(
            body.error("one composition expression per branch; use a container or rust { ... }")
        );
    }
    Ok(result)
}
fn starts_constructor(input: ParseStream<'_>) -> bool {
    let fork = input.fork();
    let Ok(path) = fork.parse::<Path>() else {
        return false;
    };
    let Some(last) = path.segments.last() else {
        return false;
    };
    // Qualified constructors are ordinary Rust values. Compose widgets are
    // brought into scope with `use`, which also keeps third-party integration
    // independent of macro registration or cross-crate inspection.
    if path.segments.len() > 1 {
        return false;
    }
    let name = last.ident.to_string();
    if matches!(name.as_str(), "Some" | "Ok" | "Err") {
        return false;
    }
    name.chars().next().is_some_and(char::is_uppercase) && fork.peek(syn::token::Paren)
}
impl Parse for Value {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(kw::reduce) {
            input.parse::<kw::reduce>()?;
            let name = if input.peek(Ident) {
                let name: Ident = input.parse()?;
                if name.to_string().starts_with("__") {
                    return Err(syn::Error::new(
                        name.span(),
                        "reducer names beginning __ are reserved for generated implementation details",
                    ));
                }
                Some(name)
            } else {
                None
            };
            let mut captures = Vec::new();
            if input.peek(syn::token::Paren) {
                let arguments;
                syn::parenthesized!(arguments in input);
                while !arguments.is_empty() {
                    let name: Ident = arguments.parse()?;
                    if name.to_string().starts_with("__") {
                        return Err(syn::Error::new(
                            name.span(),
                            "capture names beginning __ are reserved for generated implementation details",
                        ));
                    }
                    if matches!(name.to_string().as_str(), "key" | "incarnation" | "props") {
                        return Err(syn::Error::new(
                            name.span(),
                            "key, incarnation, and props are reserved reducer capture names",
                        ));
                    }
                    if captures
                        .iter()
                        .any(|capture: &ReduceCapture| capture.name == name)
                    {
                        return Err(syn::Error::new(name.span(), "duplicate reducer capture"));
                    }
                    arguments.parse::<Token![:]>()?;
                    let ty = arguments.parse()?;
                    arguments.parse::<Token![=]>()?;
                    let value = arguments.parse()?;
                    captures.push(ReduceCapture { name, ty, value });
                    if arguments.is_empty() {
                        break;
                    }
                    arguments.parse::<Token![,]>()?;
                }
            }
            return Ok(Self::Reduce(Reduce {
                name,
                captures,
                body: input.parse()?,
            }));
        }
        if input.peek(kw::effect) {
            input.parse::<kw::effect>()?;
            let operation = input.parse()?;
            let arguments;
            syn::parenthesized!(arguments in input);
            let args = arguments.parse_terminated(Expr::parse, Token![,])?;
            let lifecycle;
            syn::braced!(lifecycle in input);

            let start = if lifecycle.peek(kw::on_start) {
                lifecycle.parse::<kw::on_start>()?;
                Some(lifecycle.parse()?)
            } else if lifecycle.peek(kw::start) {
                lifecycle.parse::<kw::start>()?;
                Some(lifecycle.parse()?)
            } else {
                None
            };

            let legacy_ok = lifecycle.peek(Ident)
                && lifecycle
                    .fork()
                    .parse::<Ident>()
                    .is_ok_and(|name| name == "Ok");
            if lifecycle.peek(kw::on_ok) {
                lifecycle.parse::<kw::on_ok>()?;
            } else if legacy_ok {
                lifecycle.parse::<Ident>()?;
            } else {
                return Err(lifecycle.error("effect requires on_ok(result) { ... }"));
            }
            let ok_binding;
            syn::parenthesized!(ok_binding in lifecycle);
            let ok_pattern = ok_binding.call(Pat::parse_single)?;
            if !ok_binding.is_empty() {
                return Err(ok_binding.error("on_ok takes one result pattern"));
            }
            let ok = lifecycle.parse()?;

            let legacy_err = lifecycle.peek(Ident)
                && lifecycle
                    .fork()
                    .parse::<Ident>()
                    .is_ok_and(|name| name == "Err");
            if lifecycle.peek(kw::on_err) {
                lifecycle.parse::<kw::on_err>()?;
            } else if legacy_err {
                lifecycle.parse::<Ident>()?;
            } else {
                return Err(lifecycle.error("effect requires on_err(error) { ... }"));
            }
            let err_binding;
            syn::parenthesized!(err_binding in lifecycle);
            let err_pattern = err_binding.call(Pat::parse_single)?;
            if !err_binding.is_empty() {
                return Err(err_binding.error("on_err takes one error pattern"));
            }
            let err = lifecycle.parse()?;

            if !lifecycle.is_empty() {
                return Err(lifecycle.error("unexpected tokens after on_err block"));
            }
            return Ok(Self::Effect(Effect {
                operation,
                args,
                start,
                ok_pattern,
                ok,
                err_pattern,
                err,
            }));
        }
        if input.peek(kw::rust) {
            input.parse::<kw::rust>()?;
            let block: syn::Block = input.parse()?;
            return Ok(Self::ExplicitRust(syn::parse_quote!(#block)));
        }
        if input.peek(Token![if]) {
            let fork = input.fork();
            if let Ok(expr) = fork.parse::<Expr>() {
                if !matches!(expr, Expr::If(_)) && (fork.is_empty() || fork.peek(Token![,])) {
                    let expr = input.parse::<Expr>()?;
                    return Ok(Self::Rust(expr));
                }
            }
            input.parse::<Token![if]>()?;
            let cond = input.call(Expr::parse_without_eager_brace)?;
            let yes = Box::new(branch(input)?);
            let no = if input.peek(Token![else]) {
                input.parse::<Token![else]>()?;
                Some(Box::new(if input.peek(Token![if]) {
                    input.parse()?
                } else {
                    branch(input)?
                }))
            } else {
                None
            };
            return Ok(Self::If { cond, yes, no });
        }
        if input.peek(Token![for]) {
            input.parse::<Token![for]>()?;
            let pat = input.call(Pat::parse_single)?;
            input.parse::<Token![in]>()?;
            let iter = input.call(Expr::parse_without_eager_brace)?;
            return Ok(Self::For {
                pat,
                iter,
                body: Box::new(branch(input)?),
            });
        }
        if input.peek(Token![match]) {
            input.parse::<Token![match]>()?;
            let expr = input.call(Expr::parse_without_eager_brace)?;
            let body;
            syn::braced!(body in input);
            let mut arms = Vec::new();
            while !body.is_empty() {
                let pat = body.call(Pat::parse_multi_with_leading_vert)?;
                let guard = if body.peek(Token![if]) {
                    body.parse::<Token![if]>()?;
                    Some(body.call(Expr::parse_without_eager_brace)?)
                } else {
                    None
                };
                body.parse::<Token![=>]>()?;
                let value = body.parse()?;
                let comma_optional = matches!(&value, Self::Rust(Expr::Block(_)));
                arms.push((pat, guard, value));
                if body.is_empty() {
                    break;
                }
                if body.peek(Token![,]) {
                    body.parse::<Token![,]>()?;
                } else if !comma_optional {
                    return Err(body.error("expected `,` after match arm"));
                }
            }
            return Ok(Self::Match { expr, arms });
        }
        if input.peek(syn::token::Bracket) {
            let list;
            syn::bracketed!(list in input);
            let mut values = Vec::new();
            while !list.is_empty() {
                let value: Value = list.parse()?;
                let comma_optional = matches!(
                    &value,
                    Self::If { .. }
                        | Self::For { .. }
                        | Self::Match { .. }
                        | Self::Rust(Expr::Block(_))
                );
                values.push(value);
                if list.is_empty() {
                    break;
                }
                if list.peek(Token![,]) {
                    list.parse::<Token![,]>()?;
                } else if !comma_optional {
                    return Err(list.error("expected `,` between child expressions"));
                }
            }
            return Ok(Self::Array(values));
        }
        if input.peek(Ident) {
            let fork = input.fork();
            let ident: Ident = fork.parse()?;
            if ident == "Some" && fork.peek(syn::token::Paren) {
                let fork_inner;
                syn::parenthesized!(fork_inner in fork);
                let _: Value = fork_inner.parse()?;
                if fork_inner.peek(Token![,]) {
                    fork_inner.parse::<Token![,]>()?;
                }
                // `Some(widget)` is Compose sugar, while an expression beginning
                // with `Some(...)` (for example, a comparison) remains ordinary Rust.
                if !fork_inner.is_empty() || !(fork.is_empty() || fork.peek(Token![,])) {
                    let expr: Expr = input.parse()?;
                    return Ok(Self::Rust(expr));
                }

                input.parse::<Ident>()?;
                let inner;
                syn::parenthesized!(inner in input);
                let value = inner.parse()?;
                if inner.peek(Token![,]) {
                    inner.parse::<Token![,]>()?;
                }
                if !inner.is_empty() {
                    return Err(inner.error("Some takes one value"));
                }
                return Ok(Self::Optional(Box::new(value)));
            }
        }
        if starts_constructor(input) {
            let path = input.parse()?;
            let args;
            syn::parenthesized!(args in input);
            let mut entries = Vec::new();
            let mut named = std::collections::HashSet::new();
            let mut bindings = std::collections::HashSet::new();
            let mut saw_named = false;
            while !args.is_empty() {
                let binding = {
                    let fork = args.fork();
                    let is_binding = fork.parse::<Ident>().is_ok_and(|ident| ident == "bind")
                        && fork.peek(syn::token::Paren);
                    if is_binding {
                        args.parse::<Ident>()?;
                        let binding;
                        syn::parenthesized!(binding in args);
                        let port: Ident = binding.parse()?;
                        let event = if binding.peek(Token![as]) {
                            binding.parse::<Token![as]>()?;
                            Some(binding.parse()?)
                        } else {
                            None
                        };
                        if !binding.is_empty() {
                            return Err(binding
                                .error("binding takes an event property and optional event type"));
                        }
                        args.parse::<Token![:]>()?;
                        if !bindings.insert(port.to_string()) {
                            return Err(syn::Error::new(
                                port.span(),
                                "duplicate binding for event property",
                            ));
                        }
                        saw_named = true;
                        Some(Binding { port, event })
                    } else {
                        None
                    }
                };
                let name = if binding.is_some() {
                    None
                } else if args.peek(Ident) && args.peek2(Token![:]) && !args.peek2(Token![::]) {
                    let id: Ident = args.parse()?;
                    args.parse::<Token![:]>()?;
                    if !named.insert(id.to_string()) {
                        return Err(syn::Error::new(id.span(), "duplicate component argument"));
                    }
                    saw_named = true;
                    Some(id)
                } else {
                    if saw_named {
                        return Err(args.error("positional arguments must precede named arguments"));
                    }
                    None
                };
                let value = args.parse()?;
                entries.push(Arg {
                    name,
                    binding,
                    value,
                });
                if args.is_empty() {
                    break;
                }
                args.parse::<Token![,]>()?;
            }
            let mut modifiers = Vec::new();
            while input.peek(Token![.]) {
                input.parse::<Token![.]>()?;
                let name = input.parse()?;
                let arguments;
                syn::parenthesized!(arguments in input);
                modifiers.push(Modifier {
                    name,
                    args: arguments.parse_terminated(Expr::parse, Token![,])?,
                });
            }
            return Ok(Self::Call(Call {
                path,
                args: entries,
                modifiers,
            }));
        }
        let expr: Expr = input.parse()?;
        if let Expr::Call(c) = &expr {
            if matches!(c.func.as_ref(),Expr::Path(p) if p.path.is_ident("source")) {
                if c.args.len() != 1 {
                    return Err(syn::Error::new_spanned(expr, "source takes one expression"));
                }
                return Ok(Self::Source(c.args[0].clone()));
            }
            if matches!(c.func.as_ref(),Expr::Path(p) if p.path.is_ident("read")) {
                if c.args.len() != 1 {
                    return Err(syn::Error::new_spanned(expr, "read takes one expression"));
                }
                return Ok(Self::Read(c.args[0].clone()));
            }
            if matches!(c.func.as_ref(),Expr::Path(p) if p.path.is_ident("to")) {
                if c.args.len() != 1 {
                    return Err(syn::Error::new_spanned(
                        expr,
                        "to takes one writable state or local field",
                    ));
                }
                return Ok(Self::To(c.args[0].clone()));
            }
        }
        Ok(Self::Rust(expr))
    }
}
