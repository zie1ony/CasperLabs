set -u 

SYSTEM="../../keys/faucet-account"
REMOVE_KEY="../../keys/account-0"

BASE_PATH="../../../../"
CLIENT="${BASE_PATH}/client/target/universal/stage/bin/casperlabs-client"
HOST="localhost"
ACCOUNT_PRIVATE_FILE="account-private.pem"
ACCOUNT_ID_FILE="account-id"
ACCOUNT_ID_HEX_FILE="account-id-hex"
FROM="${SYSTEM}/${ACCOUNT_PRIVATE_FILE}"
REMOVE_KEY_HEX=$(cat "${REMOVE_KEY}/${ACCOUNT_ID_HEX_FILE}")
WASM_FILE="${BASE_PATH}/execution-engine/target/wasm32-unknown-unknown/release/remove_associated_key.wasm"

ARGS="[\
    {\"name\": \"target-account\", \"value\": {\"bytes_value\": \"${REMOVE_KEY_HEX}\"}} \
]"

RESPONSE=$($CLIENT --host $HOST deploy \
    --session ${WASM_FILE} \
    --session-args "${ARGS}" \
    --payment-amount 10000000 \
    --private-key ${FROM} \
)

DEPLOY_HASH=$(echo $RESPONSE | awk '{print $3}')

$CLIENT --host $HOST propose

$CLIENT --host $HOST show-deploy $DEPLOY_HASH
