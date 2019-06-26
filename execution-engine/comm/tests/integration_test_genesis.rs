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
