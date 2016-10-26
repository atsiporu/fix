#![feature(box_syntax)]

extern crate fixr;

use std::sync::mpsc::{self, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, SystemTime};
use fixr::fix::{FixTransport, FixTimerFactory, FixTimerHandler, FixService};
use fixr::connection::{FixConnection, ConnectionType};

pub struct TestTransport
{
    view: Vec<u8>,
}

impl FixTransport for TestTransport
{
	fn connect<F>(&self, on_success: F) where F: Fn() -> ()
	{
		on_success();
	}

	fn view(&self) -> &[u8] {
	    &self.view
    }

	fn consume(&self, len: usize) {
	}

	fn write(&self, buf: &[u8]) -> usize {
		0usize
	}
}


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

use std::env;
fn main()
{
	let args = &mut env::args();

	args.next(); // consume program name

	if args.len() != 3 {
		println!("usage: <ip> <port> <-c|-l>");
		return;
	}

	let ip = args.next().unwrap();
	let port = args.next().unwrap();
	let mode = args.next().unwrap();
	
    let transport = TestTransport { view: vec![0]}; 
    let mut timers = TestFixEnvironment {};

    let (conn_type, _) = match mode.as_str() {
		"-c" => {
            (ConnectionType::Initiator, ())
		},
		"-l" => {
            (ConnectionType::Acceptor, ())
		},
		_ => {
			panic!("usage: <ip> <port> <-c|-l>");
		}
	};
    
    let mut conn = FixConnection::new(transport, timers, conn_type);
    conn.connect();

	loop {
		thread::sleep_ms(100);
	}
}
