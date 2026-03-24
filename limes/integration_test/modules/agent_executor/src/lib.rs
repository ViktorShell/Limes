use limes_macro::limes_run;

#[limes_run]
fn run(args: String) -> String {
    invoke_agent(&args)
}
