#[contract]
mod ERC20 {
    struct Storage {
        name: felt252,
        symbol: felt252,
    }


    #[view]
    fn get_name() -> felt252 {
        name::read()
    }

    #[view]
    fn get_symbol() -> felt252 {
        symbol::read()
    }
}