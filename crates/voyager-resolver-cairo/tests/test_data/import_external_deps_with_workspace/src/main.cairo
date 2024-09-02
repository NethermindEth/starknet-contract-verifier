#[starknet::contract]
mod contract {
    use openzeppelin::{
        upgrades::{UpgradeableComponent, interface::IUpgradeable},
    };
    use starknet::{get_contract_address, get_caller_address};
    use starknet::{ContractAddress, ClassHash};

    #[storage]
    struct Storage {}

    fn value(self: @ContractState, ) -> felt252 {
        42
    }
}
