use limes_macro::limes_run;

#[limes_run]
fn run(prompt: String) -> String {
    invoke_agent(&prompt)
}
