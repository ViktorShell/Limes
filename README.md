# Limes
Distributed WebAssembly system for urgent edge cloud computing

## Running the Sync tests
``` bash
cargo run --bin benchmark --release
```
This will take a while, 40min on a 2Gh processor and depends by your disk speed.
It will generate times.csv which includes the data relative cold start and exec time of the Limes Lambda Executor 
Analysis of the data can be found inside the analysis folder:

```bash
cd analysis
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
jupyter notebook
```
Open data_analysis file

## User side function implementation
How to create a Limes compatible serverless function
```cargo new --lib```

add to Cargo.toml
```toml
[lib]
crate-type = ["cdylib"]
```
```bash
cargo add wit-bindgen
```

Modifie the lib.rs with:
```rust
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
```

Install target
```bash
    rustup target add wasm32-wasip2
```
and compile
```bash
    cargo build --target wasm32-wasip2 --release
```
the source wasm file can be found in the target folder.
