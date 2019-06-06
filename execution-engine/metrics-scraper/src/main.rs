use std::io;
use std::time::Duration;

use crate::input::Sink;
use crate::output::open_drain;
use metrics_scraper::accumulator::Accumulator;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub mod input;
pub mod output;

const EXPECTED_POLL_INTERVAL_SECONDS: u64 = 5;
const RUNNABLE_CHECK_INTERVAL_SECONDS: u64 = 3;
const SIGINT_HANDLE_EXPECT: &str = "Error setting Ctrl-C handler";
const SINK_START_EXPECT: &str = "Sink should start";

/// Gets SIGINT handle to allow clean exit
fn get_sigint_handle() -> Arc<AtomicBool> {
    let handle = Arc::new(AtomicBool::new(true));
    let h = handle.clone();
    ctrlc::set_handler(move || {
        h.store(false, Ordering::SeqCst);
    })
    .expect(SIGINT_HANDLE_EXPECT);
    handle
}

fn main() -> io::Result<()> {
    // TODO: args
    let expected_poll_length = Duration::new(EXPECTED_POLL_INTERVAL_SECONDS, 0);
    let addr = ([127, 0, 0, 1], 3000).into();

    let accumulator: Accumulator<String> = Accumulator::new(expected_poll_length);

    {
        let _ = open_drain(accumulator.clone(), &addr);
    }

    {
        let sink = Sink::new(accumulator.clone());
        let _ = sink.start().expect(SINK_START_EXPECT);

        let interval = Duration::from_secs(RUNNABLE_CHECK_INTERVAL_SECONDS);
        let runnable = get_sigint_handle();
        while runnable.load(Ordering::SeqCst) {
            std::thread::park_timeout(interval);
        }

        sink.stop();
    }

    Ok(())
}
