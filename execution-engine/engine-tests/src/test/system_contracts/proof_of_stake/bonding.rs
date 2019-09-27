use contract_ffi::base16;
use contract_ffi::key::Key;
use contract_ffi::value::account::PublicKey;
use contract_ffi::value::account::PurseId;
use contract_ffi::value::{Value, U512};

use engine_core::engine_state::genesis::{GenesisAccount, POS_BONDING_PURSE};
use engine_core::engine_state::CONV_RATE;
use engine_core::engine_state::MAX_PAYMENT;
use engine_shared::motes::Motes;
use engine_shared::transform::Transform;

use crate::support::test_support::{
    self, DeployBuilder, ExecRequestBuilder, InMemoryWasmTestBuilder, STANDARD_PAYMENT_CONTRACT,
};
use crate::test::DEFAULT_PAYMENT;
use crate::test::{DEFAULT_ACCOUNTS, DEFAULT_ACCOUNT_ADDR};

const ACCOUNT_1_ADDR: [u8; 32] = [1u8; 32];
const ACCOUNT_1_SEED_AMOUNT: u64 = 100_000_000 * 2;
const ACCOUNT_1_STAKE: u64 = 42_000;
const ACCOUNT_1_UNBOND_1: u64 = 22_000;
const ACCOUNT_1_UNBOND_2: u64 = 20_000;

const GENESIS_VALIDATOR_STAKE: u64 = 50_000;
const GENESIS_ACCOUNT_STAKE: u64 = 100_000;
const GENESIS_ACCOUNT_UNBOND_1: u64 = 45_000;
const GENESIS_ACCOUNT_UNBOND_2: u64 = 55_000;

const TEST_BOND: &str = "bond";
const TEST_BOND_FROM_MAIN_PURSE: &str = "bond-from-main-purse";
const TEST_SEED_NEW_ACCOUNT: &str = "seed_new_account";
const TEST_UNBOND: &str = "unbond";

fn get_pos_purse_id_by_name(
    builder: &InMemoryWasmTestBuilder,
    purse_name: &str,
) -> Option<PurseId> {
    let pos_contract = builder.get_pos_contract();

    pos_contract
        .urefs_lookup()
        .get(purse_name)
        .and_then(Key::as_uref)
        .map(|u| PurseId::new(*u))
}

fn get_pos_bonding_purse_balance(builder: &InMemoryWasmTestBuilder) -> U512 {
    let purse_id = get_pos_purse_id_by_name(builder, POS_BONDING_PURSE)
        .expect("should find PoS payment purse");
    builder.get_purse_balance(purse_id)
}

#[ignore]
#[test]
fn should_run_successful_bond_and_unbond() {
    let genesis_account_key = Key::Account(DEFAULT_ACCOUNT_ADDR);

    let accounts = {
        let mut tmp: Vec<GenesisAccount> = DEFAULT_ACCOUNTS.clone();
        let account = GenesisAccount::new(
            PublicKey::new([42; 32]),
            Motes::new(GENESIS_VALIDATOR_STAKE.into()) * Motes::new(2.into()),
            Motes::new(GENESIS_VALIDATOR_STAKE.into()),
        );
        tmp.push(account);
        tmp
    };

    let genesis_config = test_support::create_genesis_config(accounts);

    let result = InMemoryWasmTestBuilder::default()
        .run_genesis(&genesis_config)
        .finish();

    let genesis_account = result
        .builder()
        .get_account(genesis_account_key)
        .expect("should get account 1");

    let pos = result.builder().get_pos_contract_uref();

    let exec_request_1 = {
        let deploy = DeployBuilder::new()
            .with_address(DEFAULT_ACCOUNT_ADDR)
            .with_payment_code(STANDARD_PAYMENT_CONTRACT, (*DEFAULT_PAYMENT,))
            .with_session_code(
                "pos_bonding.wasm",
                (String::from(TEST_BOND), U512::from(GENESIS_ACCOUNT_STAKE)),
            )
            .with_deploy_hash([1u8; 32])
            .with_authorization_keys(&[PublicKey::new(DEFAULT_ACCOUNT_ADDR)])
            .build();
        ExecRequestBuilder::from_deploy(deploy).build()
    };

    let result = InMemoryWasmTestBuilder::from_result(result)
        .exec_with_exec_request(exec_request_1)
        .expect_success()
        .commit()
        .finish();

    let exec_response = result
        .builder()
        .get_exec_response(0)
        .expect("should have exec response");
    let mut genesis_gas_cost = test_support::get_exec_costs(&exec_response)[0];

    let transforms = &result.builder().get_transforms()[0];

    let pos_transform = &transforms[&Key::from(pos).normalize()];

    // Verify that genesis account is in validator queue
    let contract = if let Transform::Write(Value::Contract(contract)) = pos_transform {
        contract
    } else {
        panic!(
            "pos transform is expected to be of AddKeys variant but received {:?}",
            pos_transform
        );
    };

    let lookup_key = format!(
        "v_{}_{}",
        base16::encode_lower(&DEFAULT_ACCOUNT_ADDR),
        GENESIS_ACCOUNT_STAKE
    );
    assert!(contract.urefs_lookup().contains_key(&lookup_key));

    // Gensis validator [42; 32] bonded 50k, and genesis account bonded 100k inside
    // the test contract
    assert_eq!(
        get_pos_bonding_purse_balance(result.builder()),
        U512::from(GENESIS_VALIDATOR_STAKE + GENESIS_ACCOUNT_STAKE)
    );

    let exec_request_2 = {
        let deploy = DeployBuilder::new()
            .with_address(DEFAULT_ACCOUNT_ADDR)
            .with_payment_code(STANDARD_PAYMENT_CONTRACT, (*DEFAULT_PAYMENT,))
            .with_session_code(
                "pos_bonding.wasm",
                (
                    String::from(TEST_SEED_NEW_ACCOUNT),
                    PublicKey::new(ACCOUNT_1_ADDR),
                    U512::from(ACCOUNT_1_SEED_AMOUNT),
                ),
            )
            .with_deploy_hash([2u8; 32])
            .with_authorization_keys(&[PublicKey::new(DEFAULT_ACCOUNT_ADDR)])
            .build();
        ExecRequestBuilder::from_deploy(deploy).build()
    };

    let exec_request_3 = {
        let deploy = DeployBuilder::new()
            .with_address(ACCOUNT_1_ADDR)
            .with_payment_code(STANDARD_PAYMENT_CONTRACT, (*DEFAULT_PAYMENT,))
            .with_session_code(
                "pos_bonding.wasm",
                (
                    String::from(TEST_BOND_FROM_MAIN_PURSE),
                    U512::from(ACCOUNT_1_STAKE),
                ),
            )
            .with_deploy_hash([1; 32])
            .with_authorization_keys(&[PublicKey::new(ACCOUNT_1_ADDR)])
            .build();
        ExecRequestBuilder::from_deploy(deploy).build()
    };

    // Create new account (from genesis funds) and bond with it
    let result = InMemoryWasmTestBuilder::from_result(result)
        .exec_with_exec_request(exec_request_2)
        .expect_success()
        .commit()
        .exec_with_exec_request(exec_request_3)
        .expect_success()
        .commit()
        .finish();

    let account_1_key = Key::Account(ACCOUNT_1_ADDR);

    let exec_response = result
        .builder()
        .get_exec_response(0)
        .expect("should have exec response");
    genesis_gas_cost = genesis_gas_cost + test_support::get_exec_costs(&exec_response)[0];

    let account_1 = result
        .builder()
        .get_account(account_1_key)
        .expect("should get account 1");

    let pos = result.builder().get_pos_contract_uref();

    let transforms = &result.builder().get_transforms()[1];

    let pos_transform = &transforms[&Key::from(pos).normalize()];

    // Verify that genesis account is in validator queue
    let contract = if let Transform::Write(Value::Contract(contract)) = pos_transform {
        contract
    } else {
        panic!(
            "pos transform is expected to be of AddKeys variant but received {:?}",
            pos_transform
        );
    };

    let lookup_key = format!(
        "v_{}_{}",
        base16::encode_lower(&ACCOUNT_1_ADDR),
        ACCOUNT_1_STAKE
    );
    assert!(contract.urefs_lookup().contains_key(&lookup_key));

    // Gensis validator [42; 32] bonded 50k, and genesis account bonded 100k inside
    // the test contract
    let pos_bonding_purse_balance = get_pos_bonding_purse_balance(result.builder());
    assert_eq!(
        pos_bonding_purse_balance,
        U512::from(GENESIS_VALIDATOR_STAKE + GENESIS_ACCOUNT_STAKE + ACCOUNT_1_STAKE)
    );

    //
    // Stage 2a - Account 1 unbonds by decreasing less than 50% (and is still in the
    // queue)
    //
    let exec_request_4 = {
        let deploy = DeployBuilder::new()
            .with_address(ACCOUNT_1_ADDR)
            .with_payment_code(STANDARD_PAYMENT_CONTRACT, (*DEFAULT_PAYMENT,))
            .with_session_code(
                "pos_bonding.wasm",
                (
                    String::from(TEST_UNBOND),
                    Some(U512::from(ACCOUNT_1_UNBOND_1)),
                ),
            )
            .with_deploy_hash([2; 32])
            .with_authorization_keys(&[PublicKey::new(ACCOUNT_1_ADDR)])
            .build();
        ExecRequestBuilder::from_deploy(deploy).build()
    };
    let account_1_bal_before = result.builder().get_purse_balance(account_1.purse_id());
    let result = InMemoryWasmTestBuilder::from_result(result)
        .exec_with_exec_request(exec_request_4)
        .expect_success()
        .commit()
        .finish();

    let account_1_bal_after = result.builder().get_purse_balance(account_1.purse_id());
    let exec_response = result
        .builder()
        .get_exec_response(0)
        .expect("should have exec response");
    let gas_cost_b = Motes::from_gas(test_support::get_exec_costs(&exec_response)[0], CONV_RATE)
        .expect("should convert");

    assert_eq!(
        account_1_bal_after,
        account_1_bal_before - gas_cost_b.value() + ACCOUNT_1_UNBOND_1,
    );

    // POS bonding purse is decreased
    assert_eq!(
        get_pos_bonding_purse_balance(result.builder()),
        U512::from(GENESIS_VALIDATOR_STAKE + GENESIS_ACCOUNT_STAKE + ACCOUNT_1_UNBOND_2)
    );

    let pos_contract = result.builder().get_pos_contract();

    let lookup_key = format!(
        "v_{}_{}",
        base16::encode_lower(&ACCOUNT_1_ADDR),
        ACCOUNT_1_STAKE
    );
    assert!(!pos_contract.urefs_lookup().contains_key(&lookup_key));

    let lookup_key = format!(
        "v_{}_{}",
        base16::encode_lower(&ACCOUNT_1_ADDR),
        ACCOUNT_1_UNBOND_2
    );
    // Account 1 is still tracked anymore in the bonding queue with different uref
    // name
    assert!(pos_contract.urefs_lookup().contains_key(&lookup_key));

    //
    // Stage 2b - Genesis unbonds by decreasing less than 50% (and is still in the
    // queue)
    //
    // Genesis account unbonds less than 50% of his stake
    let exec_request_5 = {
        let deploy = DeployBuilder::new()
            .with_address(DEFAULT_ACCOUNT_ADDR)
            .with_payment_code(STANDARD_PAYMENT_CONTRACT, (*DEFAULT_PAYMENT,))
            .with_session_code(
                "pos_bonding.wasm",
                (
                    String::from(TEST_UNBOND),
                    Some(U512::from(GENESIS_ACCOUNT_UNBOND_1)),
                ),
            )
            .with_deploy_hash([3; 32])
            .with_authorization_keys(&[PublicKey::new(DEFAULT_ACCOUNT_ADDR)])
            .build();
        ExecRequestBuilder::from_deploy(deploy).build()
    };
    let result = InMemoryWasmTestBuilder::from_result(result)
        .exec_with_exec_request(exec_request_5)
        .expect_success()
        .commit()
        .finish();

    let exec_response = result
        .builder()
        .get_exec_response(0)
        .expect("should have exec response");
    genesis_gas_cost = genesis_gas_cost + test_support::get_exec_costs(&exec_response)[0];

    assert_eq!(
        result
            .builder()
            .get_purse_balance(genesis_account.purse_id()),
        U512::from(
            test_support::GENESIS_INITIAL_BALANCE
                - Motes::from_gas(genesis_gas_cost, CONV_RATE)
                    .expect("should convert")
                    .value()
                    .as_u64()
                - ACCOUNT_1_SEED_AMOUNT
                - GENESIS_ACCOUNT_UNBOND_2
        ),
    );

    // POS bonding purse is further decreased
    assert_eq!(
        get_pos_bonding_purse_balance(result.builder()),
        U512::from(GENESIS_VALIDATOR_STAKE + GENESIS_ACCOUNT_UNBOND_2 + ACCOUNT_1_UNBOND_2)
    );

    //
    // Stage 3a - Fully unbond account1 with Some(TOTAL_AMOUNT)
    //
    let account_1_bal_before = result.builder().get_purse_balance(account_1.purse_id());

    let exec_request_6 = {
        let deploy = DeployBuilder::new()
            .with_address(ACCOUNT_1_ADDR)
            .with_payment_code(STANDARD_PAYMENT_CONTRACT, (*DEFAULT_PAYMENT,))
            .with_session_code(
                "pos_bonding.wasm",
                (
                    String::from(TEST_UNBOND),
                    Some(U512::from(ACCOUNT_1_UNBOND_2)),
                ),
            )
            .with_deploy_hash([3; 32])
            .with_authorization_keys(&[PublicKey::new(ACCOUNT_1_ADDR)])
            .build();
        ExecRequestBuilder::from_deploy(deploy).build()
    };

    let result = InMemoryWasmTestBuilder::from_result(result)
        .exec_with_exec_request(exec_request_6)
        .expect_success()
        .commit()
        .finish();

    let account_1_bal_after = result.builder().get_purse_balance(account_1.purse_id());
    let exec_response = result
        .builder()
        .get_exec_response(0)
        .expect("should have exec response");
    let gas_cost_b = Motes::from_gas(test_support::get_exec_costs(&exec_response)[0], CONV_RATE)
        .expect("should convert");

    assert_eq!(
        account_1_bal_after,
        account_1_bal_before - gas_cost_b.value() + ACCOUNT_1_UNBOND_2,
    );

    // POS bonding purse contains now genesis validator (50k) + genesis account
    // (55k)
    assert_eq!(
        get_pos_bonding_purse_balance(result.builder()),
        U512::from(GENESIS_VALIDATOR_STAKE + GENESIS_ACCOUNT_UNBOND_2)
    );

    let pos_contract = result.builder().get_pos_contract();

    let lookup_key = format!(
        "v_{}_{}",
        base16::encode_lower(&ACCOUNT_1_ADDR),
        ACCOUNT_1_UNBOND_2
    );
    // Account 1 isn't tracked anymore in the bonding queue
    assert!(!pos_contract.urefs_lookup().contains_key(&lookup_key));

    //
    // Stage 3b - Fully unbond account1 with Some(TOTAL_AMOUNT)
    //

    // Genesis account unbonds less than 50% of his stake
    let exec_request_7 = {
        let deploy = DeployBuilder::new()
            .with_address(DEFAULT_ACCOUNT_ADDR)
            .with_payment_code(STANDARD_PAYMENT_CONTRACT, (*DEFAULT_PAYMENT,))
            .with_session_code(
                "pos_bonding.wasm",
                (String::from(TEST_UNBOND), None as Option<U512>),
            )
            .with_deploy_hash([4; 32])
            .with_authorization_keys(&[PublicKey::new(DEFAULT_ACCOUNT_ADDR)])
            .build();
        ExecRequestBuilder::from_deploy(deploy).build()
    };

    let result = InMemoryWasmTestBuilder::from_result(result)
        .exec_with_exec_request(exec_request_7)
        .expect_success()
        .commit()
        .finish();

    let exec_response = result
        .builder()
        .get_exec_response(0)
        .expect("should have exec response");
    genesis_gas_cost = genesis_gas_cost + test_support::get_exec_costs(&exec_response)[0];

    // Back to original after funding account1's pursee
    assert_eq!(
        result
            .builder()
            .get_purse_balance(genesis_account.purse_id()),
        U512::from(
            test_support::GENESIS_INITIAL_BALANCE
                - Motes::from_gas(genesis_gas_cost, CONV_RATE)
                    .expect("should convert")
                    .value()
                    .as_u64()
                - ACCOUNT_1_SEED_AMOUNT
        )
    );

    // Final balance after two full unbonds is the initial bond valuee
    assert_eq!(
        get_pos_bonding_purse_balance(result.builder()),
        U512::from(GENESIS_VALIDATOR_STAKE)
    );

    let pos_contract = result.builder().get_pos_contract();
    let lookup_key = format!(
        "v_{}_{}",
        base16::encode_lower(&DEFAULT_ACCOUNT_ADDR),
        GENESIS_ACCOUNT_UNBOND_2
    );
    // Genesis is still tracked anymore in the bonding queue with different uref
    // name
    assert!(!pos_contract.urefs_lookup().contains_key(&lookup_key));

    //
    // Final checks on validator queue
    //

    // Account 1 is still tracked anymore in the bonding queue with any amount
    // suffix
    assert_eq!(
        pos_contract
            .urefs_lookup()
            .iter()
            .filter(|(key, _)| key.starts_with(&format!(
                "v_{}",
                base16::encode_lower(&DEFAULT_ACCOUNT_ADDR)
            )))
            .count(),
        0
    );
    assert_eq!(
        pos_contract
            .urefs_lookup()
            .iter()
            .filter(
                |(key, _)| key.starts_with(&format!("v_{}", base16::encode_lower(&ACCOUNT_1_ADDR)))
            )
            .count(),
        0
    );
    // only genesis validator is still in the queue
    assert_eq!(
        pos_contract
            .urefs_lookup()
            .iter()
            .filter(|(key, _)| key.starts_with("v_"))
            .count(),
        1
    );
}

#[ignore]
#[test]
fn should_fail_bonding_with_insufficient_funds() {
    let accounts = {
        let mut tmp: Vec<GenesisAccount> = DEFAULT_ACCOUNTS.clone();
        let account = GenesisAccount::new(
            PublicKey::new([42; 32]),
            Motes::new(GENESIS_VALIDATOR_STAKE.into()) * Motes::new(2.into()),
            Motes::new(GENESIS_VALIDATOR_STAKE.into()),
        );
        tmp.push(account);
        tmp
    };

    let genesis_config = test_support::create_genesis_config(accounts);

    let exec_request_1 = {
        let deploy = DeployBuilder::new()
            .with_address(DEFAULT_ACCOUNT_ADDR)
            .with_payment_code(STANDARD_PAYMENT_CONTRACT, (U512::from(MAX_PAYMENT),))
            .with_session_code(
                "pos_bonding.wasm",
                (
                    String::from(TEST_SEED_NEW_ACCOUNT),
                    PublicKey::new(ACCOUNT_1_ADDR),
                    U512::from(MAX_PAYMENT + GENESIS_ACCOUNT_STAKE),
                ),
            )
            .with_deploy_hash([1; 32])
            .with_authorization_keys(&[PublicKey::new(DEFAULT_ACCOUNT_ADDR)])
            .build();
        ExecRequestBuilder::from_deploy(deploy).build()
    };
    let exec_request_2 = {
        let deploy = DeployBuilder::new()
            .with_address(ACCOUNT_1_ADDR)
            .with_payment_code(STANDARD_PAYMENT_CONTRACT, (U512::from(MAX_PAYMENT),)) // That's already too much assuming non-zero costs of wasm execution
            .with_session_code(
                "pos_bonding.wasm",
                (
                    String::from(TEST_BOND_FROM_MAIN_PURSE),
                    U512::from(MAX_PAYMENT + GENESIS_ACCOUNT_STAKE),
                ),
            )
            .with_deploy_hash([2; 32])
            .with_authorization_keys(&[PublicKey::new(ACCOUNT_1_ADDR)])
            .build();
        ExecRequestBuilder::from_deploy(deploy).build()
    };

    let result = InMemoryWasmTestBuilder::default()
        .run_genesis(&genesis_config)
        .exec_with_exec_request(exec_request_1)
        .commit()
        .exec_with_exec_request(exec_request_2)
        .commit()
        .finish();

    let response = result
        .builder()
        .get_exec_response(1)
        .expect("should have a response")
        .to_owned();

    let error_message = {
        let execution_result = crate::support::test_support::get_success_result(&response);
        test_support::get_error_message(execution_result)
    };
    // Error::BondTransferFailed => 7
    assert_eq!(error_message, "Exit code: 7");
}

#[ignore]
#[test]
fn should_fail_unbonding_validator_without_bonding_first() {
    let accounts = {
        let mut tmp: Vec<GenesisAccount> = DEFAULT_ACCOUNTS.clone();
        let account = GenesisAccount::new(
            PublicKey::new([42; 32]),
            Motes::new(GENESIS_VALIDATOR_STAKE.into()) * Motes::new(2.into()),
            Motes::new(GENESIS_VALIDATOR_STAKE.into()),
        );
        tmp.push(account);
        tmp
    };

    let genesis_config = test_support::create_genesis_config(accounts);

    let exec_request = {
        let deploy = DeployBuilder::new()
            .with_address(DEFAULT_ACCOUNT_ADDR)
            .with_payment_code(STANDARD_PAYMENT_CONTRACT, (U512::from(MAX_PAYMENT),))
            .with_session_code(
                "pos_bonding.wasm",
                (String::from(TEST_UNBOND), Some(U512::from(42))),
            )
            .with_deploy_hash([1; 32])
            .with_authorization_keys(&[PublicKey::new(DEFAULT_ACCOUNT_ADDR)])
            .build();
        ExecRequestBuilder::from_deploy(deploy).build()
    };

    let result = InMemoryWasmTestBuilder::default()
        .run_genesis(&genesis_config)
        .exec_with_exec_request(exec_request)
        .commit()
        .finish();

    let response = result
        .builder()
        .get_exec_response(0)
        .expect("should have a response")
        .to_owned();

    let error_message = {
        let execution_result = crate::support::test_support::get_success_result(&response);
        test_support::get_error_message(execution_result)
    };
    // Error::NotBonded => 0
    assert_eq!(error_message, "Exit code: 0");
}
