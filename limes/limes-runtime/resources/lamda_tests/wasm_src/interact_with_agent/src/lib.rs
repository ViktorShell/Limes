use limes_macro::limes_run;

#[limes_run]
fn run(_input: String) -> String {
    invoke_agent(&_input)
}
