wit_bindgen::generate!({
    inline: r"
        package component:run;
        interface run {
            run: func(args: string) -> string;
        }

        world runnable {
            export run;
        }
    "
});

use crate::exports::component::run::run::Guest;

use std::net::TcpListener;

struct Component;
impl Guest for Component {
    #[allow(unused)]
    fn run(args: String) -> String {
        #[allow(unused)]
        let listener = TcpListener::bind("127.0.0.1:50400").unwrap();
        println!("ECHO FROM WASM COMPONENT");
        String::from("### TEST WASIp2")
    }
}

export!(Component);
