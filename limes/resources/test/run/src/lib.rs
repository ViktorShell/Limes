#[allow(warnings)]
mod bindings;

use bindings::exports::component::run::run::Guest;

struct Component;

impl Guest for Component {
    /// Say hello!
    fn run(args: String) -> String {
        let mut local = args.clone();
        local.push_str("### TEST WORKING ###");
        return local;
    }
}

bindings::export!(Component with_types_in bindings);
