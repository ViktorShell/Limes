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

//  crate exported  component:run -> run interface -> Guest
use crate::exports::component::run::run::Guest;
use std::thread;
use std::time::Duration;

struct Component;

impl Guest for Component {
    fn run(args: String) -> String {
        let mut local_args = String::from(args);
        local_args.push_str("### INFINITE LOOP ###");
        loop {
            thread::sleep(Duration::from_secs(1));
        }
        #[allow(unreachable_code)]
        local_args
    }
}

export!(Component);
