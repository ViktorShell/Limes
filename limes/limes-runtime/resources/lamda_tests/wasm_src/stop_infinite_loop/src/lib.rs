use limes_macro::limes_run;
use std::thread;
use std::time::Duration;

#[limes_run]
fn run(args: String) -> String {
    let mut local_args = args;
    local_args.push_str("### INFINITE LOOP ###");
    loop {
        thread::sleep(Duration::from_secs(1));
    }
    #[allow(unreachable_code)]
    local_args
}
