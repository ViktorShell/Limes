# Limes

A runtime for WebAssembly lambda function's with support for LLM's interaction.

## How to use the server

Download the project folder and move inside the `limes-runtime` directory.
Build the project using `cargo build --release` to obtain the executable server.
Run the server using the follow example:

``` bash
cargo run --bin limes-server -- -v --ip {IP_ADDRESS} -p {PORT}
```

Or obtain more information by executing

``` bash
cargo run --bin limes-server -- --help
```

## Define you own Lambda Functions in WebAssembly

Any type of WebAssembly WasiP2 is compatible but it **must** define a *run* function as follows:

```rust
fn run(args: String) -> String {
  //...
}
```

For our example we define the functions using **Rust** as follow:
Create a Rust project by using the command `cargo new --lib my_lambda`.
Modify the `Cargo.toml` file by adding the following lines:

```toml
[package]
name = "my_lambda"
version = "0.1.0"
edition = "2024"

[dependencies]
atoi = "2.0.0"
limes-macro = { path = "{PATH_TO}/limes-macro" }
wit-bindgen = "0.53.1"

[lib]
crate-type = ["cdylib"]
```

You must insert the valid *path* to the limes-macro folder
Modify the `lib.rs` as follow and add the WebAssembly compatible module as you need

``` rust
use limes_macro::limes_run;

#[limes_run]
fn run(args: String) -> String {
  // ...
}
```

Than compile the project by using.

``` bash
cargo build --release --target wasm32-wasip2
```

## Interact with the Server

The server offer a **REST API** with the following paths:

| Entry | Http method | Path | Action |
| --- | --- | --- | --- |
| **Create User** | *Post* | `/users` | Gives and unique user id |
| **Remove User** | *Delete* | `/users/user_id` | Delete the user by the id |
| **Register Module** | *Post* | `/users/{user_id}/modules` | Insert the bytes in the body to register the module |
| **Load Function** | *Post* | `/users/{user_id}/modules/{module_id}/functions` | Load the functions, is important the input structure definition and give an unique function id |
| **Execute Function** | *Post* | `/users/{user_id}/functions/{function_id}/exec` | Execute the function |

In the `integration_test/` folder there is a simple implementation in python of a client able to interact with the server.

### LLM's interaction and Input Structure

Every function you load can interact with a local or remote LLM, in our case we are using **llama3.2:3b** with the tool **ollama**.
You can download the model and execute the service by executing `ollama pull llama3.2:3b && ollama serve` than the server will automatically connect to the ollama server.
The interaction with the model is done by using the function `invoke_agent(prompt: &args) -> String` inside your function, the implementation is hidden behind the **limes-macro**.
> [!NOTE]
> If you want that the local **agents** interact with your functions you must specify a **JSON** format of your function input.
> This is a must because of how the LLM's interact with local tools and how they are able to generate the right input for your functions.
> An example of a well defined input structure for your function can be as follows:
>
> ```rust
> fn add(a: i32, b: i32) -> i32 {
>   a + b
> }
>```
>
> When registering the function_input_structure we define: `{a: number, b: number}`

For any question open an issue or write me on my email ❤️.
