#[contract]
mod Dummy {
    // Core Library Imports
    use starknet::ContractAddress;
    use starknet::get_caller_address;
    use array::ArrayTrait;

    #[storage]
    struct Storage {}

    #[contructor]
    fn constructor() -> () {
        return ();
    }

    #[view]
    fn get_value() -> u8 {
        return 42;
    }
}