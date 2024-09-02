mod a {
    const A: felt252 = 42;
}

#[starknet::contract]
mod contract {
    use super::a;
    use super::super::contracts::constants;

    #[storage]
    struct Storage {}

    #[generate_trait]
    impl InternalImpl of InternalTrait {
        fn _value(
            self: @ContractState,
        ) -> felt252 {
            return constants::VALUE + a::A;
        }
    }
}