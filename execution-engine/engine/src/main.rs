// internal dependencies
extern crate binascii;
// third-party dependencies
extern crate clap;
extern crate common;
extern crate execution_engine;
#[macro_use]
extern crate lazy_static;
extern crate parking_lot;
extern crate shared;
extern crate storage;
extern crate wasm_prep;

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::fs::File;
use std::io::prelude::*;
use std::iter::Iterator;
use std::str;
use std::sync::Arc;

use clap::{App, Arg, ArgMatches};
use parking_lot::Mutex;

use common::key::Key;
use common::value::account::BlockTime;
use execution_engine::engine_state::EngineState;
use execution_engine::engine_state::error::RootNotFound;
use execution_engine::engine_state::execution_effect::ExecutionEffect;
use execution_engine::engine_state::execution_result::ExecutionResult;
use execution_engine::execution::WasmiExecutor;
use shared::init::mocked_account;
use shared::logging;
use shared::logging::log_level::LogLevel;
use shared::logging::log_settings;
use shared::logging::log_settings::{LogLevelFilter, LogSettings};
use shared::newtypes::{Blake2bHash, CorrelationId};
use storage::global_state::CommitResult;
use storage::global_state::History;
use storage::global_state::in_memory::InMemoryGlobalState;
use wasm_prep::{wasm_costs::WasmCosts, WasmiPreprocessor};

// exe / proc
const PROC_NAME: &str = "execution-engine";
const APP_NAME: &str = "Execution Engine Standalone";
const SERVER_START_MESSAGE: &str = "starting Execution Engine Standalone";
const SERVER_STOP_MESSAGE: &str = "stopping Execution Engine Standalone";
const SERVER_NO_WASM_MESSAGE: &str = "no wasm files to process";
const SERVER_NO_GAS_LIMIT_MESSAGE: &str = "gas limit is 0";
const VALIDATE_NONCE: &str = "validate-nonce";

// loglevel
const ARG_LOG_LEVEL: &str = "loglevel";
const ARG_LOG_LEVEL_VALUE: &str = "LOGLEVEL";
const ARG_LOG_LEVEL_HELP: &str = "[ fatal | error | warning | info | debug ]";

// defaults
const DEFAULT_ADDRESS: &str = "3030303030303030303030303030303030303030303030303030303030303030";
const DEFAULT_GAS_LIMIT: &str = "18446744073709551615";

// Command line arguments instance
lazy_static! {
    static ref ARG_MATCHES: clap::ArgMatches<'static> = get_args();
}

// LogSettings instance to be used within this application
lazy_static! {
    static ref LOG_SETTINGS: log_settings::LogSettings = get_log_settings();
}

#[derive(Debug)]
struct Task {
    path: String,
    bytes: Vec<u8>,
}

fn apply_effects<H>(
    correlation_id: CorrelationId,
    engine_state: &EngineState<H>,
    pre_state_hash: &Blake2bHash,
    effects: ExecutionEffect,
) -> (
    LogLevel,
    String,
    BTreeMap<String, String>,
    Option<Blake2bHash>,
)
where
    H: History,
    H::Error: Into<execution_engine::execution::Error> + Debug,
{
    match engine_state.apply_effect(correlation_id, *pre_state_hash, effects.transforms) {
        Ok(CommitResult::RootNotFound) => {
            let mut properties: BTreeMap<String, String> = BTreeMap::new();
            let error_message = format!("root {:?} not found", pre_state_hash);
            properties.insert(String::from("root-hash"), format!("{:?}", pre_state_hash));
            (LogLevel::Warning, error_message, properties, None)
        }
        Ok(CommitResult::KeyNotFound(key)) => {
            let mut properties: BTreeMap<String, String> = BTreeMap::new();
            let error_message = format!("key {:?} not found", key);
            (LogLevel::Warning, error_message, properties, None)
        }
        Ok(CommitResult::TypeMismatch(type_mismatch)) => {
            let mut properties: BTreeMap<String, String> = BTreeMap::new();
            let error_message = format!("type mismatch: {:?} ", type_mismatch);
            (LogLevel::Warning, error_message, properties, None)
        }
        Ok(CommitResult::Success(new_root_hash)) => {
            let mut properties: BTreeMap<String, String> = BTreeMap::new();
            properties.insert(
                String::from("post-state-hash"),
                format!("{:?}", new_root_hash),
            );
            (
                LogLevel::Info,
                String::new(),
                properties,
                Some(new_root_hash),
            )
        }
        Err(storage_err) => {
            let mut properties: BTreeMap<String, String> = BTreeMap::new();
            let error_message = format!("{:?}", storage_err);
            (LogLevel::Error, error_message, properties, None)
        }
    }
}

fn log_message(
    log_level: LogLevel,
    error_message: String,
    mut properties: BTreeMap<String, String>,
) {
    let success = error_message.is_empty();
    properties.insert(String::from("success"), format!("{:?}", success));

    if !success {
        properties.insert(String::from("error"), error_message);
    }

    let message_format: String = if success {
        String::from("{wasm-path} success: {success} gas_cost: {gas-cost}")
    } else {
        String::from("{wasm-path} error: {error} gas_cost: {gas-cost}")
    };

    logging::log_details(log_level, message_format, properties);
}

#[allow(unreachable_code)]
fn main() {
    set_panic_hook();

    log_settings::set_log_settings_provider(&*LOG_SETTINGS);

    logging::log_info(SERVER_START_MESSAGE);

    let matches: &clap::ArgMatches = &*ARG_MATCHES;

    let wasm_files: Vec<Task> = {
        let file_str_iter = matches.values_of("wasm").expect("Wasm file not defined.");
        file_str_iter
            .map(|file_str| {
                let mut file = File::open(file_str).expect("Cannot open Wasm file");
                let mut content: Vec<u8> = Vec::new();
                let _ = file
                    .read_to_end(&mut content)
                    .expect("Error when reading a file:");
                Task {
                    path: String::from(file_str),
                    bytes: content,
                }
            })
            .collect()
    };

    if wasm_files.is_empty() {
        logging::log_info(SERVER_NO_WASM_MESSAGE);
    }

    let account_addr = {
        let address_hex = matches.value_of("address").expect("Unable to get address");
        if address_hex.len() != 64 {
            panic!("Provided address should be exactly 64 bytes long");
        }
        // Into fixed size array of 32 bytes
        let mut dest = [0; 32];
        binascii::hex2bin(address_hex.as_bytes(), &mut dest)
            .ok()
            .expect("Unable to parse address");
        Key::Account(dest)
    };

    let gas_limit: u64 = matches
        .value_of("gas-limit")
        .and_then(|v| v.parse::<u64>().ok())
        .expect("Provided gas limit value is not u64.");

    if gas_limit == 0 {
        logging::log_info(SERVER_NO_GAS_LIMIT_MESSAGE);
    }

    let validate_nonce = matches.is_present(VALIDATE_NONCE);

    // TODO: move to arg parser
    let timestamp: u64 = 100_000;
    let protocol_version: u64 = 1;

    let init_state = mocked_account(account_addr.as_account().unwrap());
    let global_state = InMemoryGlobalState::from_pairs(CorrelationId::new(), &init_state)
        .expect("Could not create global state");
    let mut state_hash: Blake2bHash = global_state.root_hash;
    let global_state_arc = Arc::new(Mutex::new(global_state));
    let engine_state = EngineState::new(global_state_arc, validate_nonce);

    let wasmi_executor = WasmiExecutor;
    let wasm_costs = WasmCosts::from_version(protocol_version).unwrap_or_else(|| {
        panic!(
            "Wasm cost table wasn't defined for protocol version: {}",
            protocol_version
        )
    });
    let wasmi_preprocessor: WasmiPreprocessor = WasmiPreprocessor::new(wasm_costs);

    for (i, wasm_bytes) in wasm_files.iter().enumerate() {
        let correlation_id = CorrelationId::new();
        let nonce = i as u64 + 1;
        let result = engine_state.run_deploy(
            &wasm_bytes.bytes,
            &[], // TODO: consume args from CLI
            account_addr,
            BlockTime(timestamp),
            nonce,
            state_hash,
            gas_limit,
            protocol_version,
            correlation_id,
            &wasmi_executor,
            &wasmi_preprocessor,
        );

        let mut properties = BTreeMap::new();

        properties.insert(
            String::from("validate-nonce"),
            format!("{:?}", validate_nonce),
        );
        properties.insert(String::from("pre-state-hash"), format!("{:?}", state_hash));
        properties.insert(String::from("wasm-path"), wasm_bytes.path.to_owned());
        properties.insert(String::from("nonce"), format!("{}", nonce));

        match result {
            Err(RootNotFound(hash)) => {
                let log_level = LogLevel::Error;
                let error_message = format!("root {:?} not found", hash);
                properties.insert(String::from("root-hash"), format!("{:?}", hash));
                log_message(log_level, error_message, properties);
            }
            Ok(ExecutionResult::Success {
                effect: effects,
                cost,
            }) => {
                properties.insert("gas-cost".to_string(), format!("{:?}", cost));
                properties.insert(
                    "effects".to_string(),
                    format!("{:?}", effects.transforms.clone()),
                );
                let (log_level, error_message, mut new_properties, new_state_hash) =
                    apply_effects(correlation_id, &engine_state, &state_hash, effects);

                if let Some(hash) = new_state_hash {
                    state_hash = hash;
                }

                properties.append(&mut new_properties);
                log_message(log_level, error_message, properties);
            }
            Ok(ExecutionResult::Failure {
                error,
                effect: effects,
                cost,
            }) => {
                let log_level = LogLevel::Error;
                properties.insert("gas-cost".to_string(), format!("{:?}", cost));

                let (new_log_level, new_error_message, mut new_properties, new_state_hash) =
                    apply_effects(correlation_id, &engine_state, &state_hash, effects);

                if let Some(hash) = new_state_hash {
                    state_hash = hash;
                }

                new_properties.append(&mut properties.clone());
                log_message(new_log_level, new_error_message, new_properties);

                let error_message = format!("{:?}", error);
                log_message(log_level, error_message, properties);
            }
        }
    }

    logging::log_info(SERVER_STOP_MESSAGE);
}

/// Sets panic hook for logging panic info
fn set_panic_hook() {
    let hook: Box<dyn Fn(&std::panic::PanicInfo) + 'static + Sync + Send> =
        Box::new(move |panic_info| {
            match panic_info.payload().downcast_ref::<&str>() {
                Some(s) => {
                    let panic_message = format!("{:?}", s);
                    logging::log_fatal(&panic_message);
                }
                None => {
                    let panic_message = format!("{:?}", panic_info);
                    logging::log_fatal(&panic_message);
                }
            }

            logging::log_info(SERVER_STOP_MESSAGE);
        });
    std::panic::set_hook(hook);
}

/// Gets command line arguments
fn get_args() -> ArgMatches<'static> {
    App::new(APP_NAME)
        .arg(
            Arg::with_name("address")
                .short("a")
                .long("address")
                .default_value(DEFAULT_ADDRESS)
                .value_name("BYTES")
                .required(false)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("gas-limit")
                .short("l")
                .long("gas-limit")
                .default_value(DEFAULT_GAS_LIMIT)
                .required(false)
                .takes_value(true),
        )
        .arg(
            Arg::with_name(ARG_LOG_LEVEL)
                .required(false)
                .long(ARG_LOG_LEVEL)
                .takes_value(true)
                .value_name(ARG_LOG_LEVEL_VALUE)
                .help(ARG_LOG_LEVEL_HELP),
        )
        .arg(Arg::with_name(VALIDATE_NONCE).long(VALIDATE_NONCE))
        .arg(
            Arg::with_name("wasm")
                .long("wasm")
                .multiple(true)
                .required(true)
                .index(1),
        )
        .get_matches()
}

/// Builds and returns log_settings
fn get_log_settings() -> log_settings::LogSettings {
    let matches: &clap::ArgMatches = &*ARG_MATCHES;

    let log_level_filter = LogLevelFilter::from_input(matches.value_of(ARG_LOG_LEVEL));

    LogSettings::new(PROC_NAME, log_level_filter)
}
