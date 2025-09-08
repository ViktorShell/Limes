// libreria della macro (in un crate separato `wit-macro`)
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Ident, ItemFn};

/// limes_func! {
///     fn run(args: String) -> String {
///         format!("Hello, {}", args);
///     }
/// }

#[proc_macro]
pub fn limes_func(input: TokenStream) -> TokenStream {
    // parsing: aspetta una funzione `fn run(...) -> ...`
    let func = parse_macro_input!(input as ItemFn);
    let name = func.sig.ident.clone();

    // generiamo codice con la struttura di wit_bindgen
    let expanded = quote! {
        wit_bindgen::generate!({
            inline: r#"
                package component:run;
                interface run {
                    run: func(args: string) -> string;
                }

                world runnable {
                    export run;
                }
            "#
        });

        use crate::exports::component::run::run::Guest;

        struct Component;

        impl Guest for Component {
            fn #name(args: String) -> String {
                #func
                #name(args)
            }
        }

        export!(Component);
    };

    expanded.into()
}
