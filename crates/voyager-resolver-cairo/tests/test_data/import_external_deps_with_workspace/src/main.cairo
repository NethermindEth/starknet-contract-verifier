#[starknet::contract]
mod contract {
    use openzeppelin::token::erc1155::erc1155::ERC1155Component::InternalTrait;
    use openzeppelin::{
        introspection::{src5::SRC5Component}, access::ownable::OwnableComponent,
        token::erc1155::{ERC1155Component, ERC1155HooksEmptyImpl, interface::IERC1155MetadataURI},
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
