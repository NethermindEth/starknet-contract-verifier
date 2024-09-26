# Running the verifier locally

You might want to run the verifier locally in order to test some features of the verification stack. We cover some cases here which might be helpful to devs.

### Running it against a dev environment

In order to run the cli against the dev environment, you can utilize the custom api endpoint env vars to pass your desired api endpoints and thus allow the cli to interact with the dev environment.

For example, if your dev environment is located locally at your machine, you can do the following:

```bash
CUSTOM_INTERNAL_API_ENDPOINT_URL="http://localhost:3030" CUSTOM_PUBLIC_API_ENDPOINT_URL="http://localhost:3034" cargo run --bin starknet-contract-verifier
```

## Running the verification stack locally
This details the steps in order to run the stack for our verification flow locally. The first 3 steps includes components not in this repository. This is usually done by developer working on these components and want to perform manual testing of the verification stack.

1. Setup your database. For convenience sake you can connect to a database with data like those in integration or dev env for the network db, and you can run a local centralized db for easier monitoring,
2. Run the Cairo compiler service
3. Run the backend verification api service
4. Pass the custom endpoints env vars in order to set your endpoint urls.

With this you should be able to run the whole stack for testing verification locally.

Make sure the changes you want to test is already propagated to the dev env. If the api endpoint is unknown to you, please ask the Voyager team to provide you with the information.
