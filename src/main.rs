#![feature(box_syntax)]

extern crate fixr;

use std::sync::mpsc::{self, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, SystemTime};
use fixr::fix::{FixTimerFactory, FixTimerHandler};

pub struct TestFixTimerHandler
{
	cancel_signal: Sender<()>,
}

impl FixTimerHandler for TestFixTimerHandler
{
	fn cancel(self) {
	}
}

pub struct TestFixEnvironment
{
}

impl FixTimerFactory for TestFixEnvironment
{
	fn set_timeout<F>(&mut self, on_timeout: F, duration: Duration) -> Box<FixTimerHandler>
		where F: 'static + Fn() -> () + Send
	{
		let (tx, rx) = mpsc::channel();
		let timer_thread = thread::spawn(move || {
			let mut now = SystemTime::now();
			loop {
				match rx.try_recv() {
					Ok(_) | Err(TryRecvError::Disconnected) => {
						// timeout canceled
						break;
					},
					Err(_) => {} // other error continue
				};
				match now.elapsed() {
					Ok(elapsed) => {
						if elapsed > duration {
							on_timeout();
							now = SystemTime::now();
						}
						thread::sleep(duration / 3);
					},
					Err(err) => {
						println!("{}", err);
					},
				};
			}
		});

		box TestFixTimerHandler {
			cancel_signal: tx
		}
	}
}

fn main()
{
    println!("Hello world!");

    let env = &mut TestFixEnvironment {};
    let h = env.set_timeout(|| { println!("On Timeout"); }, Duration::from_secs(5));

    loop {
        thread::sleep_ms(100);
    }
}
