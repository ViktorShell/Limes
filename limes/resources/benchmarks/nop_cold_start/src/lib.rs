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

struct Component;
impl Guest for Component {
    #[allow(unused)]
    fn run(args: String) -> String {
        args
    }
}

export!(Component);
