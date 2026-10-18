use syn::{
    parse::{Parse, ParseStream},
    Attribute, Expr, Ident, Pat, Path, Token, Type, Visibility,
};
mod kw {
    syn::custom_keyword!(using);
    syn::custom_keyword!(rust);
}

pub struct Field {
    pub attrs: Vec<Attribute>,
    pub name: Ident,
    pub ty: Type,
    pub local: Option<Expr>,
}
pub struct Component {
    pub vis: Visibility,
    pub name: Ident,
    pub target: Type,
    pub scope: Option<(Path, Expr)>,
    pub fields: Vec<Field>,
    pub editor: Ident,
    pub state: Ident,
    pub cx: Ident,
    pub body: Value,
}
impl Parse for Component {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
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
            while !fs.is_empty() {
                let mut attrs = fs.call(Attribute::parse_outer)?;
                let name: Ident = fs.parse()?;
                fs.parse::<Token![:]>()?;
                let ty = fs.parse()?;
                let mut local = None;
                if let Some(i) = attrs.iter().position(|a| a.path().is_ident("local_state")) {
                    let attr = attrs.remove(i);
                    attr.parse_nested_meta(|m| {
                        if !m.path.is_ident("default") {
                            return Err(m.error("expected default = expression"));
                        }
                        if local.is_some() {
                            return Err(m.error("duplicate local default"));
                        }
                        local = Some(m.value()?.parse()?);
                        Ok(())
                    })?;
                    if local.is_none() {
                        return Err(syn::Error::new_spanned(
                            attr,
                            "local_state requires default = expression",
                        ));
                    }
                }
                if fields.iter().any(|f: &Field| f.name == name) {
                    return Err(syn::Error::new(name.span(), "duplicate component field"));
                }
                if name.to_string().starts_with("__") {
                    return Err(syn::Error::new(
                        name.span(),
                        "names beginning __ are reserved for generated implementation details",
                    ));
                }
                fields.push(Field {
                    attrs,
                    name,
                    ty,
                    local,
                });
                if fs.is_empty() {
                    break;
                }
                fs.parse::<Token![,]>()?;
            }
        }
        input.parse::<Token![|]>()?;
        let editor = input.parse()?;
        input.parse::<Token![,]>()?;
        let state = input.parse()?;
        input.parse::<Token![,]>()?;
        let cx = input.parse()?;
        input.parse::<Token![|]>()?;
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
            vis,
            name,
            target,
            scope,
            fields,
            editor,
            state,
            cx,
            body,
        })
    }
}
#[derive(Clone)]
pub struct Arg {
    pub name: Option<Ident>,
    pub value: Value,
}
#[derive(Clone)]
pub struct Call {
    pub path: Path,
    pub args: Vec<Arg>,
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
    Call(Call),
    Array(Vec<Value>),
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
}
impl Value {
    pub fn rust(&self) -> Option<&Expr> {
        match self {
            Self::Rust(e) | Self::ExplicitRust(e) | Self::Read(e) | Self::Source(e) => Some(e),
            _ => None,
        }
    }
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
    last.ident
        .to_string()
        .chars()
        .next()
        .is_some_and(char::is_uppercase)
        && fork.peek(syn::token::Paren)
}
impl Parse for Value {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        if input.peek(kw::rust) {
            input.parse::<kw::rust>()?;
            let block: syn::Block = input.parse()?;
            return Ok(Self::ExplicitRust(syn::parse_quote!(#block)));
        }
        if input.peek(Token![if]) {
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
                    Some(body.parse()?)
                } else {
                    None
                };
                body.parse::<Token![=>]>()?;
                let value = body.parse()?;
                arms.push((pat, guard, value));
                if body.is_empty() {
                    break;
                }
                body.parse::<Token![,]>()?;
            }
            return Ok(Self::Match { expr, arms });
        }
        if input.peek(syn::token::Bracket) {
            let list;
            syn::bracketed!(list in input);
            let mut values = Vec::new();
            while !list.is_empty() {
                values.push(list.parse()?);
                if list.is_empty() {
                    break;
                }
                list.parse::<Token![,]>()?;
            }
            return Ok(Self::Array(values));
        }
        if starts_constructor(input) {
            let path = input.parse()?;
            let args;
            syn::parenthesized!(args in input);
            let mut entries = Vec::new();
            let mut named = std::collections::HashSet::new();
            let mut saw_named = false;
            while !args.is_empty() {
                let name = if args.peek(Ident) && args.peek2(Token![:]) && !args.peek2(Token![::]) {
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
                entries.push(Arg { name, value });
                if args.is_empty() {
                    break;
                }
                args.parse::<Token![,]>()?;
            }
            return Ok(Self::Call(Call {
                path,
                args: entries,
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
        }
        Ok(Self::Rust(expr))
    }
}
