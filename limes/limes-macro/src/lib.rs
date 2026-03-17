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
        // La tua funzione originale
        #fn_vis #fn_sig #fn_body

        pub async fn invoke_agent(args: String) -> String {
            _limes_wit::component::run::limes_api::invoke_agent(&args).await
        }

        mod _limes_wit {
            wit_bindgen::generate!({
                inline: r#"
                    package component:run;
                    interface limes-api {
                        invoke-agent: func(args: string) -> string;
                    }
                    // Definiamo l'interfaccia fuori dal world
                    interface runner {
                        run: func(args: string) -> string;
                    }
                    world runnable {
                        import limes-api;
                        export runner; // Esportiamo l'interfaccia nominata
                    }
                "#,
                async: true,
            });

            struct GuestImpl;

            // Ora il percorso è più chiaro: exports::component::run::runner::Guest
            impl exports::component::run::runner::Guest for GuestImpl {
                async fn run(args: String) -> String {
                    super::#fn_name(args).await
                }
            }

            export!(GuestImpl);
        }
    };

    TokenStream::from(expanded)
}
