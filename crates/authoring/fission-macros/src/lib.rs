use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Action)] 
pub fn derive_action(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Generate a unique identifier for the lazy_static variable
    let action_id_static_name = format_ident!("{}_ACTION_ID", name.to_string().to_uppercase());

    // Generate the full path string using `module_path!()` at the call site
    // This ensures a globally unique identifier for the ActionId.
    let full_path_str = quote! { concat!(module_path!(), "::", stringify!(#name)) };

    let expanded = quote! {
        #[automatically_derived]
        #[allow(non_upper_case_globals)] // Allow static variable name
        lazy_static::lazy_static! {
            static ref #action_id_static_name: fission_core::ActionId = fission_core::ActionId::from_name(#full_path_str);
        }

        #[automatically_derived]
        impl #impl_generics fission_core::Action for #name #ty_generics #where_clause {
            fn id(&self) -> fission_core::ActionId {
                *#action_id_static_name // Dereference the lazy_static value
            }
        }
    };

    expanded.into()
}

#[proc_macro_derive(Widget, attributes(widget))]
pub fn derive_widget(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    // Placeholder for Widget macro
    quote!().into()
}