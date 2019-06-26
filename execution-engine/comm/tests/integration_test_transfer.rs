extern crate casperlabs_engine_grpc_server;
extern crate common;
extern crate execution_engine;
extern crate grpc;
extern crate parking_lot;
extern crate shared;
extern crate storage;

use std::collections::HashMap;
use std::sync::Arc;

use grpc::RequestOptions;
use parking_lot::Mutex;

use casperlabs_engine_grpc_server::engine_server::ipc_grpc::ExecutionEngineService;
use common::bytesrepr::ToBytes;
use common::key::Key;
use common::uref::URef;
use common::value::{Account, U512, Value};
use execution_engine::engine_state::EngineState;
use shared::transform::Transform;
use storage::global_state::in_memory::InMemoryGlobalState;

#[allow(unused)]
mod test_support;

const INITIAL_GENESIS_AMOUNT: u32 = 1_000_000;

const TRANSFER_1_AMOUNT: u32 = 1000;
const TRANSFER_2_AMOUNT: u32 = 750;

const GENESIS_ADDR: [u8; 32] = [6u8; 32];
const ACCOUNT_1_ADDR: [u8; 32] = [1u8; 32];
const ACCOUNT_2_ADDR: [u8; 32] = [2u8; 32];

struct TestContext {
    mint_contract_uref: URef,
    locals: HashMap<[u8; 32], Key>,
}

impl TestContext {
    pub fn new(mint_contract_uref: URef) -> Self {
        TestContext {
            mint_contract_uref,
            locals: Default::default(),
        }
    }

    pub fn track(
        &mut self,
        transforms: &HashMap<Key, Transform>,
        account_key: Key,
        account: Account,
    ) {
        let account_key_bytes = if let Key::Account(bytes) = account_key {
            bytes
        } else {
            panic!("account_key must by Key::Account variant");
        };
        let local = {
            let purse_id_bytes = account
                .purse_id()
                .value()
                .addr()
                .to_bytes()
                .expect("should serialize");
            Key::local(self.mint_contract_uref.addr(), &purse_id_bytes)
        };
        if let Some(Transform::Write(Value::Key(key @ Key::URef(_)))) = transforms.get(&local) {
            self.locals.insert(account_key_bytes, key.normalize());
        }
    }

    pub fn lookup(
        &self,
        transforms: &HashMap<Key, Transform>,
        addr: [u8; 32],
    ) -> Option<Transform> {
        self.locals
            .get(&addr)
            .and_then(|local: &Key| transforms.get(local))
            .map(ToOwned::to_owned)
    }
}

#[ignore]
#[test]
fn should_transfer_to_account() {
    let initial_genesis_amount: U512 = U512::from(INITIAL_GENESIS_AMOUNT);
    let transfer_amount: U512 = U512::from(TRANSFER_1_AMOUNT);
    let genesis_account_key = Key::Account(GENESIS_ADDR);
    let account_key = Key::Account(ACCOUNT_1_ADDR);

    let global_state = InMemoryGlobalState::empty().unwrap();
    let engine_state = EngineState::new(Arc::new(Mutex::new(global_state)), false);

    // Run genesis

    let (genesis_request, contracts) = test_support::create_genesis_request(GENESIS_ADDR);

    let genesis_response = engine_state
        .run_genesis(RequestOptions::new(), genesis_request)
        .wait_drop_metadata()
        .unwrap();

    let genesis_hash = genesis_response.get_success().get_poststate_hash();

    let genesis_transforms = test_support::get_genesis_transforms(&genesis_response);

    let mint_contract_uref = test_support::get_mint_contract_uref(&genesis_transforms, &contracts)
        .expect("should get uref");

    let mut test_context = TestContext::new(mint_contract_uref);

    let genesis_account = test_support::get_account(&genesis_transforms, &genesis_account_key)
        .expect("should get account");

    test_context.track(&genesis_transforms, genesis_account_key, genesis_account);

    // Check genesis account balance

    let genesis_balance_transform = test_context
        .lookup(&genesis_transforms, GENESIS_ADDR)
        .expect("should lookup");

    assert_eq!(
        genesis_balance_transform,
        Transform::Write(Value::UInt512(initial_genesis_amount))
    );

    // Exec transfer contract

    let exec_request = test_support::create_exec_request(
        GENESIS_ADDR,
        "transfer_to_account_01.wasm",
        genesis_hash,
    );

    let exec_response = engine_state
        .exec(RequestOptions::new(), exec_request)
        .wait_drop_metadata()
        .unwrap();

    let exec_transforms = &test_support::get_exec_transforms(&exec_response)[0];

    let account =
        test_support::get_account(&exec_transforms, &account_key).expect("should get account");

    test_context.track(&exec_transforms, account_key, account);

    // Check genesis account balance

    let genesis_balance_transform = test_context
        .lookup(&exec_transforms, GENESIS_ADDR)
        .expect("should lookup");

    assert_eq!(
        genesis_balance_transform,
        Transform::Write(Value::UInt512(initial_genesis_amount - transfer_amount))
    );

    // Check account 1 balance

    let account_1_balance_transform = test_context
        .lookup(&exec_transforms, ACCOUNT_1_ADDR)
        .expect("should lookup");

    assert_eq!(
        account_1_balance_transform,
        Transform::Write(Value::UInt512(transfer_amount))
    );
}

#[ignore]
#[test]
fn should_transfer_from_account_to_account() {
    let initial_genesis_amount: U512 = U512::from(INITIAL_GENESIS_AMOUNT);
    let transfer_1_amount: U512 = U512::from(TRANSFER_1_AMOUNT);
    let transfer_2_amount: U512 = U512::from(TRANSFER_2_AMOUNT);
    let genesis_account_key = Key::Account(GENESIS_ADDR);
    let account_1_key = Key::Account(ACCOUNT_1_ADDR);
    let account_2_key = Key::Account(ACCOUNT_2_ADDR);

    let global_state = InMemoryGlobalState::empty().unwrap();
    let engine_state = EngineState::new(Arc::new(Mutex::new(global_state)), false);

    // Run genesis

    let (genesis_request, contracts) = test_support::create_genesis_request(GENESIS_ADDR);

    let genesis_response = engine_state
        .run_genesis(RequestOptions::new(), genesis_request)
        .wait_drop_metadata()
        .unwrap();

    let genesis_hash = genesis_response.get_success().get_poststate_hash();

    let genesis_transforms = test_support::get_genesis_transforms(&genesis_response);

    let mint_contract_uref = test_support::get_mint_contract_uref(&genesis_transforms, &contracts)
        .expect("should get uref");

    let mut test_context = TestContext::new(mint_contract_uref);

    let genesis_account = test_support::get_account(&genesis_transforms, &genesis_account_key)
        .expect("should get account");

    test_context.track(&genesis_transforms, genesis_account_key, genesis_account);

    // Exec transfer 1 contract

    let exec_request = test_support::create_exec_request(
        GENESIS_ADDR,
        "transfer_to_account_01.wasm",
        genesis_hash,
    );

    let exec_1_response = engine_state
        .exec(RequestOptions::new(), exec_request)
        .wait_drop_metadata()
        .unwrap();

    let exec_1_transforms = &test_support::get_exec_transforms(&exec_1_response)[0];

    let account_1 =
        test_support::get_account(&exec_1_transforms, &account_1_key).expect("should get account");

    test_context.track(&exec_1_transforms, account_1_key, account_1);

    // Check genesis account balance

    let genesis_balance_transform = test_context
        .lookup(&exec_1_transforms, GENESIS_ADDR)
        .expect("should lookup");

    assert_eq!(
        genesis_balance_transform,
        Transform::Write(Value::UInt512(initial_genesis_amount - transfer_1_amount))
    );

    // Check account 1 balance

    let account_1_balance_transform = test_context
        .lookup(&exec_1_transforms, ACCOUNT_1_ADDR)
        .expect("should lookup");

    assert_eq!(
        account_1_balance_transform,
        Transform::Write(Value::UInt512(transfer_1_amount))
    );

    // Commit transfer contract

    let commit_request = test_support::create_commit_request(genesis_hash, &exec_1_transforms);

    let commit_response = engine_state
        .commit(RequestOptions::new(), commit_request)
        .wait_drop_metadata()
        .unwrap();

    let commit_hash = commit_response.get_success().get_poststate_hash();

    // Exec transfer 2 contract

    let exec_request = test_support::create_exec_request(
        ACCOUNT_1_ADDR,
        "transfer_to_account_02.wasm",
        commit_hash,
    );

    let exec_2_response = engine_state
        .exec(RequestOptions::new(), exec_request)
        .wait_drop_metadata()
        .unwrap();

    let exec_2_transforms = &test_support::get_exec_transforms(&exec_2_response)[0];

    let account_2 =
        test_support::get_account(&exec_2_transforms, &account_2_key).expect("should get account");

    test_context.track(&exec_2_transforms, account_2_key, account_2);

    // Check account 1 balance

    let account_1_balance_transform = test_context
        .lookup(&exec_2_transforms, ACCOUNT_1_ADDR)
        .expect("should lookup");

    assert_eq!(
        account_1_balance_transform,
        Transform::Write(Value::UInt512(transfer_1_amount - transfer_2_amount))
    );

    let account_2_balance_transform = test_context
        .lookup(&exec_2_transforms, ACCOUNT_2_ADDR)
        .expect("should lookup");

    assert_eq!(
        account_2_balance_transform,
        Transform::Write(Value::UInt512(transfer_2_amount))
    );
}

#[ignore]
#[test]
fn should_transfer_to_existing_account() {
    let initial_genesis_amount: U512 = U512::from(INITIAL_GENESIS_AMOUNT);
    let transfer_1_amount: U512 = U512::from(TRANSFER_1_AMOUNT);
    let transfer_2_amount: U512 = U512::from(TRANSFER_2_AMOUNT);
    let genesis_account_key = Key::Account(GENESIS_ADDR);
    let account_1_key = Key::Account(ACCOUNT_1_ADDR);
    let account_2_key = Key::Account(ACCOUNT_2_ADDR);

    let global_state = InMemoryGlobalState::empty().unwrap();
    let engine_state = EngineState::new(Arc::new(Mutex::new(global_state)), false);

    // Run genesis

    let (genesis_request, contracts) = test_support::create_genesis_request(GENESIS_ADDR);

    let genesis_response = engine_state
        .run_genesis(RequestOptions::new(), genesis_request)
        .wait_drop_metadata()
        .unwrap();

    let genesis_hash = genesis_response.get_success().get_poststate_hash();

    let genesis_transforms = test_support::get_genesis_transforms(&genesis_response);

    let mint_contract_uref = test_support::get_mint_contract_uref(&genesis_transforms, &contracts)
        .expect("should get uref");

    let mut test_context = TestContext::new(mint_contract_uref);

    let genesis_account = test_support::get_account(&genesis_transforms, &genesis_account_key)
        .expect("should get account");

    test_context.track(&genesis_transforms, genesis_account_key, genesis_account);

    // Check genesis account balance

    let genesis_balance_transform = test_context
        .lookup(&genesis_transforms, GENESIS_ADDR)
        .expect("should lookup");

    assert_eq!(
        genesis_balance_transform,
        Transform::Write(Value::UInt512(initial_genesis_amount))
    );

    // Exec transfer contract

    let exec_request = test_support::create_exec_request(
        GENESIS_ADDR,
        "transfer_to_account_01.wasm",
        genesis_hash,
    );

    let exec_response = engine_state
        .exec(RequestOptions::new(), exec_request)
        .wait_drop_metadata()
        .unwrap();

    let exec_1_transforms = &test_support::get_exec_transforms(&exec_response)[0];

    let account_1 =
        test_support::get_account(&exec_1_transforms, &account_1_key).expect("should get account");

    test_context.track(&exec_1_transforms, account_1_key, account_1);

    // Check genesis account balance

    let genesis_balance_transform = test_context
        .lookup(&exec_1_transforms, GENESIS_ADDR)
        .expect("should lookup");

    assert_eq!(
        genesis_balance_transform,
        Transform::Write(Value::UInt512(initial_genesis_amount - transfer_1_amount))
    );

    // Check account 1 balance

    let account_1_balance_transform = test_context
        .lookup(&exec_1_transforms, ACCOUNT_1_ADDR)
        .expect("should lookup");

    assert_eq!(
        account_1_balance_transform,
        Transform::Write(Value::UInt512(transfer_1_amount))
    );

    // Commit transfer contract

    let commit_request = test_support::create_commit_request(genesis_hash, &exec_1_transforms);

    let commit_response = engine_state
        .commit(RequestOptions::new(), commit_request)
        .wait_drop_metadata()
        .unwrap();

    let commit_hash = commit_response.get_success().get_poststate_hash();

    // Exec transfer contract

    let exec_request = test_support::create_exec_request(
        ACCOUNT_1_ADDR,
        "transfer_to_account_02.wasm",
        commit_hash,
    );

    let exec_response = engine_state
        .exec(RequestOptions::new(), exec_request)
        .wait_drop_metadata()
        .unwrap();

    let exec_2_transforms = &test_support::get_exec_transforms(&exec_response)[0];

    let account_2 =
        test_support::get_account(&exec_2_transforms, &account_2_key).expect("should get account");

    test_context.track(&exec_2_transforms, account_2_key, account_2);

    // Check account 1 balance

    let account_1_balance_transform = test_context
        .lookup(&exec_2_transforms, ACCOUNT_1_ADDR)
        .expect("should lookup");

    assert_eq!(
        account_1_balance_transform,
        Transform::Write(Value::UInt512(transfer_1_amount - transfer_2_amount))
    );

    // Check account 2 balance

    let account_2_balance_transform = test_context
        .lookup(&exec_2_transforms, ACCOUNT_2_ADDR)
        .expect("should lookup");

    assert_eq!(
        account_2_balance_transform,
        Transform::Write(Value::UInt512(transfer_2_amount))
    );
}

#[ignore]
#[test]
fn should_fail_when_insufficient_funds() {
    let global_state = InMemoryGlobalState::empty().unwrap();
    let engine_state = EngineState::new(Arc::new(Mutex::new(global_state)), false);

    // Run genesis

    let (genesis_request, _) = test_support::create_genesis_request(GENESIS_ADDR);

    let genesis_response = engine_state
        .run_genesis(RequestOptions::new(), genesis_request)
        .wait_drop_metadata()
        .unwrap();

    let genesis_hash = genesis_response.get_success().get_poststate_hash();

    // Exec transfer contract

    let exec_request = test_support::create_exec_request(
        GENESIS_ADDR,
        "transfer_to_account_01.wasm",
        genesis_hash,
    );

    let exec_response = engine_state
        .exec(RequestOptions::new(), exec_request)
        .wait_drop_metadata()
        .unwrap();

    let exec_1_transforms = &test_support::get_exec_transforms(&exec_response)[0];

    // Commit transfer contract

    let commit_request = test_support::create_commit_request(genesis_hash, &exec_1_transforms);

    let commit_response = engine_state
        .commit(RequestOptions::new(), commit_request)
        .wait_drop_metadata()
        .unwrap();

    let commit_hash = commit_response.get_success().get_poststate_hash();

    // Exec transfer contract

    let exec_request = test_support::create_exec_request(
        ACCOUNT_1_ADDR,
        "transfer_to_account_02.wasm",
        commit_hash,
    );

    let exec_response = engine_state
        .exec(RequestOptions::new(), exec_request)
        .wait_drop_metadata()
        .unwrap();

    let exec_2_transforms = &test_support::get_exec_transforms(&exec_response)[0];

    // Commit transfer contract

    let commit_request = test_support::create_commit_request(commit_hash, &exec_2_transforms);

    let commit_response = engine_state
        .commit(RequestOptions::new(), commit_request)
        .wait_drop_metadata()
        .unwrap();

    let commit_hash = commit_response.get_success().get_poststate_hash();

    // Exec transfer contract

    let exec_request = test_support::create_exec_request(
        ACCOUNT_1_ADDR,
        "transfer_to_account_02.wasm",
        commit_hash,
    );

    let exec_response = engine_state
        .exec(RequestOptions::new(), exec_request)
        .wait_drop_metadata()
        .unwrap();

    assert_eq!(
        "Trap(Trap { kind: Unreachable })",
        exec_response
            .get_success()
            .get_deploy_results()
            .get(0)
            .unwrap()
            .get_execution_result()
            .get_error()
            .get_exec_error()
            .get_message()
    )
}
