use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

#[proc_macro_attribute]
pub fn limes_run(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;
    let fn_body = &input_fn.block;

    // Estraiamo il nome del parametro (assumendo sia uno solo di tipo String)
    // Per semplicità qui lo chiamiamo 'input'

    let expanded = quote! {
        // 1. Generiamo i bindings WIT internamente
        wit_bindgen::generate!({
            inline: r#"
                package limes:limes;
                world lambda {
                    import invoke-agent: func(input: string) -> string;
                    export run: func(input: string) -> string;
                }
            "#,
        });

        struct MyGuest;

        // 2. Implementiamo il trait Guest chiamando la funzione dell'utente
        impl Guest for MyGuest {
            fn run(input: String) -> String {
                // Definiamo la funzione dell'utente dentro lo scope
                #input_fn

                // Chiamiamo la funzione passata dall'utente
                #fn_name(input)
            }
        }

        // 3. Esportiamo il guest
        export!(MyGuest);
    };

    TokenStream::from(expanded)
}
