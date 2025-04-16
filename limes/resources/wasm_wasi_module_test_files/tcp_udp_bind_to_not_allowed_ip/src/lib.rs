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

struct Component;

impl Guest for Component {
    fn run(args: String) -> String {
        let mut local_str = String::from(args);
    }
}

export!(Component);
