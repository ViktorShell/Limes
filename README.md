# Limes

A runtime for WebAssembly lambda function's with support for LLM's interaction.

## How to use the server

Download the project folder and move inside the `limes-runtime` directory.
Build the project using `cargo build --release` to obtain the executable server.
Run the server using the follow example:

```bash
RUST_LOG=info cargo run --bin limes-server -- --ip {IP_ADDRESS} -p {PORT}
```

Or obtain more information by executing

```bash
cargo run --bin limes-server -- --help
```

## Define you own Lambda Functions in WebAssembly

Any type of WebAssembly WasiP2 is compatible but it **must** define a _run_ function as follows:

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
limes-macro = { path = "{PATH_TO}/limes-macro" }
wit-bindgen = "0.53.1"

[lib]
crate-type = ["cdylib"]
```

You must insert the valid _path_ to the limes-macro folder
Modify the `lib.rs` as follow and add the WebAssembly compatible module as you need

```rust
use limes_macro::limes_run;

#[limes_run]
fn run(args: String) -> String {
  // ...
}
```

Than compile the project by using.

```bash
cargo build --release --target wasm32-wasip2
```

## Interact with the Server

The server offer a **REST API** with the following paths:

| Entry                | Http method | Path                                             | Action                                                                                         |
| -------------------- | ----------- | ------------------------------------------------ | ---------------------------------------------------------------------------------------------- |
| **Create User**      | _Post_      | `/users`                                         | Gives and unique user id                                                                       |
| **Remove User**      | _Delete_    | `/users/user_id`                                 | Delete the user by the id                                                                      |
| **Register Module**  | _Post_      | `/users/{user_id}/modules`                       | Insert the bytes in the body to register the module                                            |
| **Load Function**    | _Post_      | `/users/{user_id}/modules/{module_id}/functions` | Load the functions, is important the input structure definition and give an unique function id |
| **Execute Function** | _Post_      | `/users/{user_id}/functions/{function_id}/exec`  | Execute the function                                                                           |

In the `integration_test/` folder there is a simple implementation in python of a client able to interact with the server.

### LLM's interaction and Input Structure

Every function you load can interact with a local or remote LLM, in our case we are using **qwen3:8b_4q_M_K** with the tool **ollama**.
You can download the model and execute the service by executing `ollama serve`, than run `ollama pull qwen3:8b_4q_M_K` than the server will automatically connect to the ollama server.
The interaction with the model is done by using the function `invoke_agent(prompt: &args) -> String` inside your function, the implementation is hidden behind the **limes-macro**.

> [!NOTE]
> If you want that the local **agents** interact with your functions you must specify a **JSON Schema** format of your function input.
> This is a must because of how the LLM's interact with local tools and how they are able to generate the right input for your functions.
> An example of a well defined input structure for your function can be as follows:
>
> ```rust
> fn add(a: i32, b: i32) -> i32 {
>   a + b
> }
> ```
>
> When registering the function_input_structure would be define as:
>
> ```json
> {
>   "required": ["a", "b"],
>   "properties": {
>     "a": {
>       "type": "number",
>       "description": "the first numerical value of the sum"
>     },
>     "b": {
>       "type": "number",
>       "description": "the second numerical value of the sum"
>     }
>   }
> }
> ```
>
> Than in your functions you will receive as a **json string**: `{"a": <value>, "b": <value>}` which can be parsed as you want.

For any question open an issue or write me on my email ❤️.
