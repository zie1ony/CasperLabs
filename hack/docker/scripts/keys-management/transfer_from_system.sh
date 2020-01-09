set -u 

SYSTEM="../../keys/faucet-account"
SENDER="../../keys/account-0"
RECIPIENT="../../keys/account-1"

BASE_PATH="../../../.."
CLIENT="${BASE_PATH}/client/target/universal/stage/bin/casperlabs-client"
HOST="localhost"
ACCOUNT_PRIVATE_FILE="account-private.pem"
ACCOUNT_ID_FILE="account-id"
ACCOUNT_ID_HEX_FILE="account-id-hex"
SENDER_PRIV="${SENDER}/${ACCOUNT_PRIVATE_FILE}"
TO=$(cat "${RECIPIENT}/${ACCOUNT_ID_FILE}")
TO_HEX=$(cat "${RECIPIENT}/${ACCOUNT_ID_HEX_FILE}")
SYSTEM_HEX=$(cat "${SYSTEM}/${ACCOUNT_ID_HEX_FILE}")
AMOUNT=1
WASM_FILE="${BASE_PATH}/execution-engine/target/wasm32-unknown-unknown/release/transfer_to_account.wasm"

ARGS="[\
    {\"name\": \"target-account\", \"value\": {\"bytes_value\": \"${TO_HEX}\"}}, \
    {\"name\": \"amount\", \"value\": {\"long_value\": \"${AMOUNT}\"}} \
]"

RESPONSE=$($CLIENT --host $HOST deploy \
    --session ${WASM_FILE} \
    --session-args "${ARGS}" \
    --payment-amount 10000000 \
    --private-key ${SENDER_PRIV} \
    --from ${SYSTEM_HEX}
)

DEPLOY_HASH=$(echo $RESPONSE | awk '{print $3}')

$CLIENT --host $HOST propose

$CLIENT --host $HOST show-deploy $DEPLOY_HASH

RESPONSE=$($CLIENT --host $HOST show-blocks)
BLOCK_HASH=$(echo $RESPONSE | awk -F "block_hash: \"" '{print $2}' | awk -F "\" header" '{print $1}')

$CLIENT --host $HOST balance \
    --block-hash $BLOCK_HASH \
    --address $TO_HEX
