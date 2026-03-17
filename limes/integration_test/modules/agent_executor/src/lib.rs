use limes_macro::limes_run;

#[limes_run]
async fn run(args: String) -> String {
    invoke_agent(&args).await
}
