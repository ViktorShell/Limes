- Add a logger to the project +limes
- Add axum interface for register, unregister, start and stop +limes
- Add documentation +limes
- Create a local DB for the modules +limes
- Force yield thread +limes

@ 16/05/24
- Finish axum interface
- Do the testing
- Force yield thread to clap
- Install & Run script

Any function registered and initialized have a second id to identify the runtime

IMPORTANT:
Any componentent that have a linked engine when using serde needs to have the same signature

THE STORE AND WASICTX are the motherfucker that make a hell of a mess when working with async due to the fact that they do not implement Sync explicitly
