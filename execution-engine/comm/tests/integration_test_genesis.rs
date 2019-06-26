extern crate casperlabs_engine_grpc_server;
extern crate execution_engine;
extern crate grpc;
extern crate parking_lot;
extern crate shared;
extern crate storage;

use std::sync::Arc;

use grpc::RequestOptions;
use parking_lot::Mutex;

use casperlabs_engine_grpc_server::engine_server::ipc::GenesisResponse;
use casperlabs_engine_grpc_server::engine_server::ipc_grpc::ExecutionEngineService;
use execution_engine::engine_state::EngineState;
use storage::global_state::in_memory::InMemoryGlobalState;

#[allow(unused)]
mod test_support;

const GENESIS_ADDR: [u8; 32] = [6u8; 32];

#[test]
fn should_run_genesis() {
    let global_state = InMemoryGlobalState::empty().expect("should create global state");
    let engine_state = EngineState::new(global_state, false);

    let genesis_request = {
        let genesis_account_addr = GENESIS_ADDR.to_vec();

        let initial_tokens = {
            let mut ret = BigInt::new();
            ret.set_bit_width(512);
            ret.set_value("1000000".to_string());
            ret
        };

        let mint_code = {
            let mut ret = DeployCode::new();
            let wasm_bytes = test_utils::create_empty_wasm_module_bytes();
            ret.set_code(wasm_bytes);
            ret
        };

        let proof_of_stake_code = {
            let mut ret = DeployCode::new();
            let wasm_bytes = test_utils::create_empty_wasm_module_bytes();
            ret.set_code(wasm_bytes);
            ret
        };

        let protocol_version = {
            let mut ret = ProtocolVersion::new();
            ret.set_value(1);
            ret
        };

        let mut ret = GenesisRequest::new();
        ret.set_address(genesis_account_addr.to_vec());
        ret.set_initial_tokens(initial_tokens);
        ret.set_mint_code(mint_code);
        ret.set_proof_of_stake_code(proof_of_stake_code);
        ret.set_protocol_version(protocol_version);
        ret
    };

    let request_options = RequestOptions::new();

    let genesis_response = engine_state
        .run_genesis(request_options, genesis_request)
        .wait_drop_metadata();

    let response = genesis_response.unwrap();

    let state_handle = engine_state.state();

    let state_handle_guard = state_handle.lock();

    let state_root_hash = state_handle_guard.root_hash;
    let response_root_hash = response.get_success().get_poststate_hash();

    assert_eq!(state_root_hash.to_vec(), response_root_hash.to_vec());
}

struct TestFixture {
    pub global_state: Arc<Mutex<InMemoryGlobalState>>,
    pub engine_state: EngineState<InMemoryGlobalState>,
}

impl TestFixture {
    pub fn new() -> TestFixture {
        let global_state = InMemoryGlobalState::empty().expect("should create global state");
        let global_state_arc = Arc::new(Mutex::new(global_state));
        let engine_state = EngineState::new(Arc::clone(&global_state_arc), true);
        TestFixture {
            global_state: Arc::clone(&global_state_arc),
            engine_state,
        }
    }

    //    pub fn run_genesis(&self, genesis_account: [u8; 32], init_tokens: U512, mint_code: &str, pos_code: &str) -> Self {
    pub fn run_genesis(&self, genesis_account: [u8; 32]) -> GenesisResponse {
        let (genesis_request, _) = test_support::create_genesis_request(genesis_account);
        let request_options = RequestOptions::new();

        self.engine_state
            .run_genesis(request_options, genesis_request)
            .wait_drop_metadata()
            .expect("No gRPC errors.")
    }
}

#[ignore]
#[test]
fn should_run_genesis_with_mint_bytes() {
    let test_fixture = TestFixture::new();
    let genesis_response = test_fixture.run_genesis(GENESIS_ADDR);

    let state_handle = test_fixture.engine_state.state();

    let state_handle_guard = state_handle.lock();

    let state_root_hash = state_handle_guard.root_hash;
    let response_root_hash = genesis_response.get_success().get_poststate_hash();

    assert_eq!(
        state_root_hash.to_vec(),
        response_root_hash.to_vec(),
        "Genesis response post state hash does not match current GlobalState hash."
    );
}
