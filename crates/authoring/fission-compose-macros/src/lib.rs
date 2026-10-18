//! Declarative component and state-scope macros; application control flow
//! remains ordinary Rust.
use proc_macro::TokenStream;
use proc_macro2::TokenStream as Tokens;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use syn::parse_macro_input;
mod ast;
mod expand;
mod scope;

fn support() -> Tokens {
    match crate_name("fission-compose") {
        Ok(FoundCrate::Itself) => quote!(::fission_compose),
        Ok(FoundCrate::Name(name)) => {
            let id = format_ident!("{}", name);
            quote!(::#id)
        }
        Err(_) => match crate_name("fission") {
            Ok(FoundCrate::Itself) => quote!(::fission::compose),
            Ok(FoundCrate::Name(name)) => {
                let id = format_ident!("{}", name);
                quote!(::#id::compose)
            }
            Err(_) => quote!(::fission_compose),
        },
    }
}
#[proc_macro]
pub fn fission_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ast::Component);
    expand::component(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
#[proc_macro]
pub fn state_scope(input: TokenStream) -> TokenStream {
    scope::expand(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn expand(input: Tokens) -> syn::Result<Tokens> {
        expand::component(syn::parse2(input)?)
    }
    fn rejects(input: Tokens, message: &str) {
        let error = match expand(input) {
            Ok(_) => panic!("expected rejection"),
            Err(e) => e,
        };
        assert!(error.to_string().contains(message), "{error}");
    }
    #[test]
    fn generated_items_are_valid_rust_syntax() {
        let output = expand(quote! {
            pub Editor<State> { session: u64, #[local_state(default = false)] open: bool }
            |editor, state, cx| => Column(children: [
                TextInput(label: Some("Name".into()), bind: state.name),
                Switch(checked: editor.open, bind(on_toggle): to(editor.open)),
                Button(child: Text("Save"), key: "save", on_press: |editor, state, cx| {
                    state.name = format!("{}", editor.session);
                    editor.open = !editor.open;
                }),
            ])
        })
        .unwrap();
        syn::parse2::<syn::File>(output).unwrap();
    }
    #[test]
    fn named_arguments_lower_to_struct_fields_without_generated_builders() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| =>
                ThirdPartyWidget(title: "Imported", enabled: true)
        })
        .unwrap()
        .to_string();

        assert!(output.contains("ThirdPartyWidget {"), "{output}");
        assert!(output.contains("title :"), "{output}");
        assert!(output.contains("enabled :"), "{output}");
        assert!(output.contains("Default :: default"), "{output}");
        assert!(!output.contains(". title ("), "{output}");
        assert!(!output.contains(". enabled ("), "{output}");
    }
    #[test]
    fn setup_statements_and_explicit_widget_modifiers_remain_ordinary_rust() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| {
                let padding = cx.view.env().theme.tokens.spacing.l;
            } => Container(
                Column(children: [Text("Ready")]),
            )
            .padding_all(padding)
            .width_length(Length::percent(100.0))
        })
        .unwrap();
        syn::parse2::<syn::File>(output.clone()).unwrap();
        let output = output.to_string();

        assert!(output.contains("let padding ="), "{output}");
        assert!(output.contains(". padding_all (padding)"), "{output}");
        assert!(output.contains(". width_length"), "{output}");
    }

    #[test]
    fn components_default_to_their_root_state_without_scope_boilerplate() {
        let expanded = expand(quote! {
            pub App<AppState> |app, state, cx| => Text(state.title.clone())
        })
        .unwrap()
        .to_string();

        assert!(expanded.contains("struct __AppRootScope"));
        assert!(expanded.contains("type Root = AppState"));
        assert!(expanded.contains("type Target = AppState"));
        assert!(!expanded.contains("DefaultScope"));
    }
    #[test]
    fn rust_prelude_tuple_constructors_remain_rust_expressions() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| => ThirdPartyWidget(
                optional: Some("value".to_owned()),
                success: Ok(3),
                failure: Err("no"),
            )
        })
        .unwrap()
        .to_string();

        assert!(
            output.contains("Some (\"value\" . to_owned ())"),
            "{output}"
        );
        assert!(output.contains("Ok (3)"), "{output}");
        assert!(output.contains("Err (\"no\")"), "{output}");
        assert!(!output.contains("Some {"), "{output}");
        assert!(!output.contains("Ok {"), "{output}");
        assert!(!output.contains("Err {"), "{output}");
    }

    #[test]
    fn qualified_enum_constructors_remain_rust_expressions() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| =>
                Text(TextContent::Key("screen.title".into()))
        })
        .unwrap()
        .to_string();

        assert!(output.contains("TextContent :: Key"), "{output}");
        assert!(!output.contains("TextContent :: Key {"), "{output}");
    }

    #[test]
    fn positional_imported_widgets_receive_a_distinct_stable_scope() {
        let first = expand(quote! {
            pub Screen<State> |screen, state, cx| =>
                Container(ThirdPartyWidget { title: "Imported".into() })
        })
        .unwrap()
        .to_string();
        let second = expand(quote! {
            pub Screen<State> |screen, state, cx| =>
                Container(ThirdPartyWidget { title: "Imported".into() })
        })
        .unwrap()
        .to_string();

        assert!(first.contains("in_slot (\"argument:"), "{first}");
        assert_eq!(first, second, "positional scopes must be deterministic");
    }

    #[test]
    fn conditional_component_branches_can_have_different_concrete_types() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| => if state.expanded {
                ExpandedView
            } else {
                CompactView
            }
        })
        .unwrap();
        let parsed = syn::parse2::<syn::File>(output.clone());
        assert!(parsed.is_ok(), "{}", output);
        assert!(output.to_string().contains("let __branch"));
    }
    #[test]
    fn conditional_property_values_remain_ordinary_rust() {
        let output = expand(quote! {
            pub Screen<State> |screen, state, cx| => Button(
                child: Text(if state.ready { "Ready".to_owned() } else { "Waiting".to_owned() }),
                variant: if state.ready {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Ghost
                },
            )
        })
        .unwrap();
        syn::parse2::<syn::File>(output.clone()).unwrap();
        assert!(!output.to_string().contains("let __branch"));
    }
    #[test]
    fn duplicate_fields_are_an_error() {
        rejects(
            quote! { X<S> { x: bool, x: bool } |e,s,c| => Text("x") },
            "duplicate component field",
        );
    }
    #[test]
    fn duplicate_props_are_an_error() {
        rejects(
            quote! { X<S> |e,s,c| => Text("x", size: 12.0, size: 14.0) },
            "duplicate",
        );
    }
    #[test]
    fn repeated_event_bindings_target_distinct_widget_properties() {
        let output = expand(quote! {
            pub RangeEditor<State> |editor,state,cx| => RangeControl(
                lower: state.lower,
                bind(on_lower_change as LowerChanged): to(state.lower),
                upper: state.upper,
                bind(on_upper_change as UpperChanged): to(state.upper),
            )
        })
        .unwrap();
        syn::parse2::<syn::File>(output.clone()).unwrap();
        let output = output.to_string();

        assert!(output.contains("on_lower_change :"), "{output}");
        assert!(output.contains("on_upper_change :"), "{output}");
        assert!(output.contains("LowerChanged"), "{output}");
        assert!(output.contains("UpperChanged"), "{output}");
    }
    #[test]
    fn duplicate_event_binding_is_an_error() {
        rejects(
            quote! {
                X<S> |e,s,c| => TextInput(
                    bind(on_input): to(s.first),
                    bind(on_input): to(s.second),
                )
            },
            "duplicate binding for event property",
        );
    }
    #[test]
    fn unkeyed_dynamic_children_are_an_error() {
        rejects(
            quote! { X<S> |e,s,c| => Column(children:[ for item in &s.items { Text(item.name.clone()) } ]) },
            "explicit key",
        );
    }
    #[test]
    fn anonymous_inline_handlers_need_a_semantic_name() {
        rejects(
            quote! { X<S> |e,s,c| => Button(child: Text("Save"), on_press: |_,s,_| s.save()) },
            "literal key",
        );
    }
    #[test]
    fn handler_capture_and_async_are_rejected() {
        rejects(
            quote! { X<S> |e,s,c| => Button(child: Text("Save"), key:"save", on_press: move |_,s,_| s.save()) },
            "no move",
        );
        rejects(
            quote! { X<S> |e,s,c| => Button(child: Text("Save"), key:"save", on_press: |_,s,_| s.save().await) },
            "synchronous",
        );
    }
    #[test]
    fn local_declaration_reorder_keeps_handler_symbols() {
        fn events(tokens: Tokens) -> Vec<String> {
            let f: syn::File = syn::parse2(expand(tokens).unwrap()).unwrap();
            f.items
                .into_iter()
                .filter_map(|item| match item {
                    syn::Item::Struct(s) if s.ident.to_string().contains("_Event_") => {
                        Some(s.ident.to_string())
                    }
                    _ => None,
                })
                .collect()
        }
        let a = events(
            quote! { X<S> { #[local_state(default=false)] a:bool, #[local_state(default=false)] b:bool }
            |e,s,c| => Button(child:Text("Save"),key:"save",on_press:|_,s,_|s.save()) },
        );
        let b = events(
            quote! { X<S> { #[local_state(default=false)] b:bool, #[local_state(default=false)] a:bool }
            |e,s,c| => Button(child:Text("Save"),key:"save",on_press:|_,s,_|s.save()) },
        );
        assert_eq!(a, b);
        assert_eq!(a.len(), 1);
    }
    #[test]
    fn scope_and_props_expansions_are_rust_items() {
        for input in [
            quote!(pub Root => Root),
            quote!(pub Root.products => Products),
            quote!(pub Editors(Root,u64=>Product){get:|s,k|s.rows.get(k),get_mut:|s,k|s.rows.get_mut(k)}),
        ] {
            syn::parse2::<syn::File>(scope::expand(input).unwrap()).unwrap();
        }
    }
}
