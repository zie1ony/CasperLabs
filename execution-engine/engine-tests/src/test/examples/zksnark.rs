use engine_test_support::low_level::{
    ExecuteRequestBuilder,
    DeployItemBuilder,
    InMemoryWasmTestBuilder as TestBuilder, 
    DEFAULT_GENESIS_CONFIG,
    DEFAULT_ACCOUNT_ADDR,
    DEFAULT_ACCOUNT_INITIAL_BALANCE,
    STANDARD_PAYMENT_CONTRACT
};
use types::{U512, account::PublicKey};
use super::erc20::erc20_test::get_cost;

const WASM: &str = "zksnark.wasm";

#[ignore]
#[test]
fn test_zksnark() {
    let mut builder = TestBuilder::default();
    builder.run_genesis(&DEFAULT_GENESIS_CONFIG).commit();
    let deploy = DeployItemBuilder::new()
        .with_address(DEFAULT_ACCOUNT_ADDR)
        .with_session_code(WASM, ())
        .with_payment_code(STANDARD_PAYMENT_CONTRACT, (U512::from(100_000_000_000u64),))
        .with_authorization_keys(&[PublicKey::new(DEFAULT_ACCOUNT_ADDR)])
        .with_deploy_hash([1u8; 32])
        .build();
    let request = ExecuteRequestBuilder::new().push_deploy(deploy).build();
    builder.exec(request).expect_success().commit();

    let response = builder.get_exec_response(0).unwrap();
    let cost = get_cost(response);
    assert_eq!(cost, U512::from(100_000_000));
}