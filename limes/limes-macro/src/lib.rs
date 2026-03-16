use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

#[proc_macro_attribute]
pub fn limes_func(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;
    let fn_body = &input_fn.block;
    let fn_vis = &input_fn.vis;
    let fn_sig = &input_fn.sig;

    let expanded = quote! {
        // 1. Let's keep the original user function accessible in the module
        #fn_vis #fn_sig #fn_body

        // 2. WIT
        wit_bindgen::generate!({
            inline: r#"
                package component:run;
                interface limes-api { invoke-agent: func(args: string) -> string; }
                interface run { run: func(args: string) -> string; }
                world runnable {
                    import limes-api;
                    export run;
                }
            "#
        });

        struct Component;

        // 3. Guest Component for wasm impl
        impl exports::component::run::run::Guest for Component {
            fn run(args: String) -> String {
                // Call of the function on top with the visibility outside the module
                #fn_name(args)
            }
        }

        export!(Component);
    };

    TokenStream::from(expanded)
}
