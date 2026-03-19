use limes_macro::limes_run;

#[limes_run]
fn run(args: String) -> String {
    let mut lm_args = args.clone();
    lm_args.push_str("### TEST ###");
    lm_args
}
