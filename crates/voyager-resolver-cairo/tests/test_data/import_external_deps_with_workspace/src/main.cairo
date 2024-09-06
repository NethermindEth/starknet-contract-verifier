#[starknet::contract]
mod contract {
    use snforge_std::{declare, ContractClassTrait, ContractClass};
    use starknet::{get_contract_address, get_caller_address};
    use starknet::{ContractAddress, ClassHash};

    #[storage]
    struct Storage {}

    fn value(self: @ContractState, ) -> felt252 {
        42
    }
}
