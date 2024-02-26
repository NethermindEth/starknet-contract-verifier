# Running the verifier locally

In order to run the verifier with locally ran services, you'll have to make some modification to the code, particularly in the `cli` crate.

## Running partial stack locally
1. Setup your database. For convenience sake you can connect to a database with data like those in integration or dev env for the network db, and you can run a local centralized db for easier monitoring,
2. Run the cairo compiler service
3. Run the backend api service
4. Change relevant code for the verification api endpoints in [`crates/cli/src/cli.rs`](./crates/cli/src/cli.rs)

With this you should be able to run the whole stack for testing verification locally.

### Running it against the dev environment

Generally, running against the integration environments is more troublesome. Once changes are propagated to dev env, you can make the necessary changes in [`crates/cli/src/cli.rs`](./crates/cli/src/cli.rs) to point to the dev env.

If the api endpoint is unknown to you, please ask the Voyager team to provide you with the information.

### Running full stack locally
WIP