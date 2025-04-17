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

use std::net::{Ipv4Addr, TcpListener, UdpSocket};

//  crate exported  component:run -> run interface -> Guest
use crate::exports::component::run::run::Guest;

struct Component;

impl Guest for Component {
    fn run(args: String) -> String {
        let split: Vec<&str> = args.split(",").collect();
        let protocol = split[0];
        let ip_str = split[1];
        println!("IPV4 => {}", ip_str);

        if protocol == "TCP" {
            #[allow(unused)]
            let tpc_listener = TcpListener::bind(ip_str).unwrap();
            return "### TCP ###".to_string();
        } else if protocol == "UDP" {
            #[allow(unused)]
            let udp_listener = UdpSocket::bind(ip_str).unwrap();
            return "### UDP ###".to_string();
        }
        return "### ERROR ###".to_string();
    }
}

export!(Component);
