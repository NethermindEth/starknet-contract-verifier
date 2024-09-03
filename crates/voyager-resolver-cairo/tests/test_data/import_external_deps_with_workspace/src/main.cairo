#[starknet::contract]
mod contract {
    use openzeppelin::{
        introspection::{src5::SRC5Component}, access::ownable::OwnableComponent,
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
