use std::net::TcpListener;

#[allow(warnings)]
mod bindings;

use bindings::exports::component::run::run::Guest;

struct Component;

impl Guest for Component {
    /// Say hello!
    fn run(args: String) -> String {
        let mut local = args.clone();
        #[allow(unused)]
        let tpc_listener = TcpListener::bind("192.168.1.2");
        local.push_str("### TEST WORKING ###");
        return local;
    }
}

bindings::export!(Component with_types_in bindings);
