use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

#[proc_macro_attribute]
pub fn limes_run(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;
    let fn_sig = &input_fn.sig;
    let fn_body = &input_fn.block;
    let fn_vis = &input_fn.vis;

    let expanded = quote! {
        // Expose invoke_agent as a free function so user code can call it directly.
        fn invoke_agent(args: &str) -> String {
            _limes_wit::component::run::limes_api::invoke_agent(args)
        }

        #fn_vis #fn_sig #fn_body

        mod _limes_wit {
            wit_bindgen::generate!({
                inline: r#"
                    package component:run;
                    interface limes-api {
                        invoke-agent: func(args: string) -> string;
                    }
                    world runnable {
                        import limes-api;
                        export run: interface {
                            run: func(args: string) -> string;
                        }
                    }
                "#,
            });

            struct GuestImpl;

            // wit-bindgen generates `exports::run::Guest` for an inline anonymous
            // interface — NOT `exports::component::run::run::Guest`.
            impl exports::run::Guest for GuestImpl {
                fn run(args: String) -> String {
                    super::#fn_name(args)
                }
            }

            export!(GuestImpl);
        }
    };

    TokenStream::from(expanded)
}
