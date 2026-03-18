/// # limes-macro
///
/// Provides the `#[limes_run]` attribute macro for writing Limes Wasm guest modules.
///
/// ## Usage
///
/// ```rust
/// use limes_macro::limes_run;
///
/// #[limes_run]
/// async fn run(args: String) -> String {
///     // Use invoke_agent to call the host LLM agent.
///     let answer = invoke_agent(&args).await;
///     answer
/// }
/// ```
///
/// The macro:
/// 1. Keeps your function intact.
/// 2. Generates a `mod _limes_wit` that sets up `wit_bindgen` and exports the
///    correct Wasm component interface so the Limes runtime can call your `run`.
/// 3. Exposes an `invoke_agent(args: &str) -> String` free function at the
///    crate root that guest modules can call to reach the host LLM.
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

/// Attribute macro that turns an `async fn run(args: String) -> String` into a
/// fully-wired Wasm component guest.
///
/// The annotated function becomes the entry point called by the Limes runtime.
/// Inside it, you may call `invoke_agent(&args).await` to query the host LLM.
#[proc_macro_attribute]
pub fn limes_run(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    let vis = &input_fn.vis;
    let sig = &input_fn.sig;
    let body = &input_fn.block;
    let fn_name = &input_fn.sig.ident;

    let expanded: TokenStream2 = quote! {
        // ── User's original function ──────────────────────────────────────────
        #vis #sig #body

        // ── invoke_agent free function (Guest → Host via WIT import) ─────────
        /// Call the Limes host LLM agent with the given prompt.
        /// Returns the agent's response as a `String`.
        pub async fn invoke_agent(args: &str) -> String {
            _limes_wit::component::run::limes_api::invoke_agent(args).await
        }

        // ── WIT bindgen glue (hidden module) ─────────────────────────────────
        mod _limes_wit {
            // Generate all Wasm component bindings from the inline WIT definition.
            // This MUST match the world declared in `limes-runtime/src/runtime/lambda.rs`.
            wit_bindgen::generate!({
                inline: r#"
                    package component:run;

                    /// Host-provided LLM agent interface.
                    interface limes-api {
                        invoke-agent: func(args: string) -> string;
                    }

                    world runnable {
                        import limes-api;
                        export run: func(args: string) -> string;
                    }
                "#,
                imports: {
                    "component:run/limes-api": async,
                },
                exports: {
                    "run": async,
                },
            });

            /// Concrete implementation of the exported `run` WIT interface.
            struct LimesGuest;

            impl Guest for LimesGuest {
                async fn run(args: String) -> String {
                    super::#fn_name(args).await
                }
            }

            export!(LimesGuest);
        }
    };

    TokenStream::from(expanded)
}
