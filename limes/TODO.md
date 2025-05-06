- The lambda.run should only have the func
- Add a logger to the project +limes
- Add axum interface for register, unregister, start and stop +limes
- Add documentation +limes
- Create a local DB for the modules +limes
- Force yield thread +limes

Any function registered and initialized have a second id to identify the runtime

IMPORTANT:
Any compiled component have and engine and this engine have to be the same for the function to execute

THE STORE AND WASICTX are the motherfucker that make a hell of a mess when working with async due to the fact that they do not implement Sync explicitly
