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
        let args: Vec<&str> = args.split(' ').collect();
        let a = args[0].parse::<i32>().unwrap();
        let b = args[2].parse::<i32>().unwrap();
        let op = args[1];
        let result = match op {
            "+" => a + b,
            "-" => a - b,
            "/" => a / b,
            "*" => a * b,
            _ => 0,
        };

        result.to_string()
    }
}

export!(Component);
