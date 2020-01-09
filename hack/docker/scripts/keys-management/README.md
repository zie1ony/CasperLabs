# Keys Management Example

This directory contains three shell scripts, that shows how to grant access to smart contract execution to another account, execute and remove this premission. Scripts are set to play well with `hack/docker` setup. 

The below example shows how a simple keys management could look like in the asset management company ABC. 

## Actors

Actors and their keys:
- System Administrator - `faucet-account`
- Employee - `account-0`

## Scenario

Organisation ABC hires a new Employee. Here's what should happen:
1. Employee generates new set of CasperLabs keys.
2. Employee sends a public key to the System Administrator.
3. System Administrator executes `add_key.sh`. Now Employee can deploy as System Administrator.
4. Employee can make a deploy in the name of System Administrator. For example Employee could transfer CLX tokens from the System Adminstrator's account using `transfer_from_system.sh`.
5. System Administrator can revoke granted permissions from Employee by calling `remove_key.sh`.
