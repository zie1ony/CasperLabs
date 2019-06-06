use std::{io, thread};

use metrics_scraper::accumulator::{AccumulationError, Pusher};
use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

fn process_line<P: Pusher<String>>(pusher: &P, line: String) -> Result<(), AccumulationError> {
    pusher.push(line)
}

pub(crate) struct Sink<P: Pusher<String> + 'static> {
    pusher: P,
    flag: Arc<AtomicBool>,
}

impl<P: Pusher<String>> Sink<P> {
    pub fn new(pusher: P) -> Sink<P> {
        let flag = Arc::new(AtomicBool::new(false));
        Sink { pusher, flag }
    }

    pub fn start(&self) -> Option<JoinHandle<()>> {
        if self.flag.load(Ordering::SeqCst) {
            return None;
        }

        self.flag.store(true, Ordering::SeqCst);

        let pusher = self.pusher.clone();
        let flag = self.flag.clone();

        Some(thread::spawn(move || {
            let stdin = io::stdin();
            let mut handle = stdin.lock();

            //let mut iter = handle.lines();
            while flag.load(Ordering::SeqCst) {
                let mut line = String::new();
                if handle.read_line(&mut line).is_ok() {
                    let result = process_line(&pusher, line);
                    if let Err(ae) = result {
                        panic!("{}", ae)
                    }
                }
            }
        }))
    }

    pub fn stop(&self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::input::{process_line, Sink};
    use metrics_scraper::accumulator::Accumulator;

    #[test]
    fn should_process_line() {
        let expected_poll_length = Duration::new(5, 0);
        let accumulator: Accumulator<String> = Accumulator::new(expected_poll_length);
        let pusher = accumulator.clone();

        let _ = process_line(&pusher, "abc".to_string());

        assert!(
            !accumulator.is_empty().expect("should is_empty"),
            "accumulator should not be empty"
        );
    }

    #[test]
    fn should_start_stop_sink() {
        let expected_poll_length = Duration::new(5, 0);
        let accumulator: Accumulator<String> = Accumulator::new(expected_poll_length);
        let sink = Sink::new(accumulator.clone());

        let handle = sink.start().expect("should start");
        sink.stop();

        let _ = handle.join();
    }

    #[test]
    fn should_start_stop_sink_multiple_times() {
        let expected_poll_length = Duration::new(5, 0);
        let accumulator: Accumulator<String> = Accumulator::new(expected_poll_length);
        let sink = Sink::new(accumulator.clone());

        let handle = sink.start().expect("should start");
        sink.stop();
        let _ = handle.join();

        let handle = sink.start().expect("should start");
        sink.stop();
        let _ = handle.join();
    }

    #[test]
    fn should_not_start_multiple_times() {
        let expected_poll_length = Duration::new(5, 0);
        let accumulator: Accumulator<String> = Accumulator::new(expected_poll_length);
        let sink = Sink::new(accumulator.clone());

        let handle1 = sink.start().expect("should start");
        let handle2 = sink.start();

        assert!(handle2.is_none(), "should not get second thread");
        sink.stop();
        let _ = handle1.join();
    }
}
