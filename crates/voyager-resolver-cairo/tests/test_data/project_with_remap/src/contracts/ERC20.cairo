#[starknet::contract]
mod ERC20 {
    use zeroable::Zeroable;
    use starknet::get_caller_address;
    use starknet::contract_address_const;
    use starknet::ContractAddress;
    use dependency::main::foo;
    use project_with_remap::contracts::bar;

    #[storage]
    struct Storage {
        name: felt252,
        symbol: felt252,
        decimals: u8,
        total_supply: u256,
        balances: LegacyMap::<ContractAddress, u256>,
        allowances: LegacyMap::<(ContractAddress, ContractAddress), u256>,
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        Approval: Approval,
        Transfer: Transfer,
    }

    #[derive(Drop, starknet::Event)]
    struct Transfer {
        from: ContractAddress,
        to: ContractAddress,
        value: u256
    }

    #[derive(Drop, starknet::Event)]
    struct Approval {
        owner: ContractAddress,
        spender: ContractAddress,
        value: u256
    }

    #[constructor]
    fn constructor(
        ref self: ContractState,
        name_: felt252,
        symbol_: felt252,
        decimals_: u8,
        initial_supply: u256,
        recipient: ContractAddress
    ) {
        self.name.write(name_);
        self.symbol.write(symbol_);
        self.decimals.write(decimals_);
        assert(!recipient.is_zero(), 'ERC20: mint to the 0 address');
        self.total_supply.write(initial_supply);
        self.balances.write(recipient, initial_supply);
        self
            .emit(
                Transfer {
                    from: contract_address_const::<0>(), to: recipient, value: initial_supply
                }
            );
    }

    #[external(v0)]
    fn get_name(self: @ContractState) -> felt252 {
        self.name.read()
    }

    #[external(v0)]
    fn get_symbol(self: @ContractState) -> felt252 {
        self.symbol.read()
    }

    #[external(v0)]
    fn get_decimals(self: @ContractState) -> u8 {
        self.decimals.read()
    }

    #[external(v0)]
    fn get_total_supply(self: @ContractState) -> u256 {
        self.total_supply.read()
    }

    #[external(v0)]
    fn balance_of(self: @ContractState, account: ContractAddress) -> u256 {
        self.balances.read(account)
    }

    #[external(v0)]
    fn allowance(self: @ContractState, owner: ContractAddress, spender: ContractAddress) -> u256 {
        self.allowances.read((owner, spender))
    }

    #[external(v0)]
    fn transfer(ref self: ContractState, recipient: ContractAddress, amount: u256) {
        let sender = get_caller_address();
        transfer_helper(ref self, sender, recipient, amount);
    }

    #[external(v0)]
    fn transfer_from(
        ref self: ContractState, sender: ContractAddress, recipient: ContractAddress, amount: u256
    ) {
        let caller = get_caller_address();
        spend_allowance(ref self, sender, caller, amount);
        transfer_helper(ref self, sender, recipient, amount);
    }

    #[external(v0)]
    fn approve(ref self: ContractState, spender: ContractAddress, amount: u256) {
        let caller = get_caller_address();
        approve_helper(ref self, caller, spender, amount);
    }

    #[external(v0)]
    fn increase_allowance(ref self: ContractState, spender: ContractAddress, added_value: u256) {
        let caller = get_caller_address();
        approve_helper(
            ref self, caller, spender, self.allowances.read((caller, spender)) + added_value
        );
    }

    #[external(v0)]
    fn decrease_allowance(
        ref self: ContractState, spender: ContractAddress, subtracted_value: u256
    ) {
        let caller = get_caller_address();
        approve_helper(
            ref self, caller, spender, self.allowances.read((caller, spender)) - subtracted_value
        );
    }

    fn transfer_helper(
        ref self: ContractState, sender: ContractAddress, recipient: ContractAddress, amount: u256
    ) {
        assert(!sender.is_zero(), 'ERC20: transfer from 0');
        assert(!recipient.is_zero(), 'ERC20: transfer to 0');
        self.balances.write(sender, self.balances.read(sender) - amount);
        self.balances.write(recipient, self.balances.read(recipient) + amount);
        self.emit(Transfer { from: sender, to: recipient, value: amount })
    }

    fn spend_allowance(
        ref self: ContractState, owner: ContractAddress, spender: ContractAddress, amount: u256
    ) {
        let current_allowance = self.allowances.read((owner, spender));
        let ONES_MASK = 0xffffffffffffffffffffffffffffffff_u128;
        let is_unlimited_allowance = current_allowance.low == ONES_MASK
            && current_allowance.high == ONES_MASK;
        if !is_unlimited_allowance {
            approve_helper(ref self, owner, spender, current_allowance - amount);
        }
    }

    fn approve_helper(
        ref self: ContractState, owner: ContractAddress, spender: ContractAddress, amount: u256
    ) {
        assert(!spender.is_zero(), 'ERC20: approve from 0');
        self.allowances.write((owner, spender), amount);
        self.emit(Approval { owner, spender, value: amount });
    }
}
