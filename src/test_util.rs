use fix::FixMsgType;
use fix::FixAppMsgType;
use fix::FixStream;
use fix::FixTagHandler;
use fix::FixStreamException;
use fix::FixParseIdLenSum;
use fix::FixTransport;
use fix::FixTimerFactory;
use std::result::Result;
use std::fmt::format;
use connection::FixConnection;
use super::util;
use std::cell::RefCell;
use std::collections::HashMap;
use std::cell::UnsafeCell;
use std::str;
use std::string::String;
use std::time::{Duration, SystemTime};
use fix::FixTimerHandler;
use std::thread::{self, Thread};
use std::sync::mpsc::{self, TryRecvError, Sender, Receiver};


extern crate scopefi;

const ASCII_ZERO: i32 = ('0' as i32);

pub struct TestFixEnvironment {}
pub struct TestFixTimerHandler
{
	cancel_signal: Sender<()>,
}

pub struct TestFixTransport<'b> {
	remote: &'b RefCell<TestFixRemote>,
}

pub struct TestFixRemote {
	sum: u32,
	len: u32,
	lenOff: usize,
	data: UnsafeCell<Vec<u8>>,
}

pub struct TestFixMessage {
	pub msg_type: Option<String>,
	pub is_replayable: bool,
	pub done: bool,
	pub tag_ids: Vec<u32>,
	pub tag_values: HashMap<u32, String>,
}

impl TestFixMessage {
	pub fn new() -> TestFixMessage
	{
		TestFixMessage {
			msg_type: None,
			is_replayable: false,
			done: false,
			tag_ids: Vec::new(),
			tag_values: HashMap::new(),
		}
	}
}

impl FixTagHandler for TestFixMessage {
	fn tag_value(&mut self, t: u32, v: &[u8])
	{
		self.tag_ids.push(t);
		self.tag_values.insert(t, String::from_utf8(Vec::from(v)).unwrap());
	}
}

impl FixStream for TestFixMessage
{
	type MSG_TYPES = ();
	fn fix_message_done(&mut self, res: Result<(), FixStreamException>)
	{
		self.done = true;
	}

	fn fix_message_start(&mut self, msg_type: FixMsgType<Self::MSG_TYPES>, is_replayable: bool)
	{
		self.is_replayable = is_replayable;
		self.done = false;
		let mut tv = vec![0;0];
		let t = String::from_utf8(Vec::from(msg_type.as_ref())).unwrap();
		self.msg_type = Some(t);
	}
}

impl TestFixRemote {
	pub fn new() -> TestFixRemote {
		TestFixRemote {
			sum: 0, 
			len: 0,
			lenOff: 0,
			data: UnsafeCell::new(vec![0u8;0]),
		}
	}
}

impl<'b> TestFixTransport<'b> {
	pub fn new<'a:'b>(remote: &'a RefCell<TestFixRemote>) -> FixConnection<TestFixTransport<'b>, TestFixEnvironment> {
		let tft = TestFixTransport {
			remote: remote,
		};

		let env = TestFixEnvironment {};

		FixConnection::new(tft, env)
	}
}

impl FixTimerHandler for TestFixTimerHandler {

	fn cancel(self) {
		self.cancel_signal.send(());
	}
}

impl FixTimerFactory for TestFixEnvironment
{
	type TH = TestFixTimerHandler;
	fn set_timeout<F>(&mut self, on_timeout: F, duration: Duration) -> Self::TH
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

		TestFixTimerHandler {
			cancel_signal: tx
		}
	}
}

impl<'b> FixTransport for TestFixTransport<'b>
{
	fn connect<F>(&self, on_success: F) where F: Fn() -> ()
	{
		on_success();
	}

	fn view(&self) -> &[u8] {
		unsafe {& *self.remote.borrow().data.get()}
	}

	fn consume(&self, len: usize) {
		unsafe {let _: Vec<u8> = (&mut *self.remote.borrow().data.get()).drain(0..len).collect();}
	}

	fn write(&self, buf: &[u8]) -> usize {
		0usize
	}
}

impl FixStream for TestFixRemote
{
	type MSG_TYPES = ();
	fn fix_message_start(&mut self, msg_type: FixMsgType<Self::MSG_TYPES>, is_replayable: bool)
	{
		self.sum = 0; // start accumulation of checksum
		
		// version first
		self.tag_value(8, &"FIX.4.2".to_string().into_bytes());
		
		// message length
		self.tag_value(9, &"".to_string().into_bytes()); // place holder
		let buf = unsafe {&mut *self.data.get()};
		self.lenOff = buf.len() - 1 /*SOH*/;
		self.len = 0; // start accumulation of length
	   
		// message type
		let v = msg_type.as_ref();
		self.tag_value(35, &v);
	}

	fn fix_message_done(&mut self, res: Result<(), FixStreamException>)
	{
		let buf = unsafe {&mut *self.data.get()};
		
		let len = format!("{:03}",  self.len).into_bytes();
		self.sum += len.iter()
			.fold((0u32, self.lenOff), |acc, &x| { buf.insert(acc.1, x as u8); (acc.0 + x as u32, acc.1 + 1) }).0;


		let chksum = format!("{:03}",  self.sum % 256u32);
		self.tag_value(10, &chksum.into_bytes()); 
	}
}

impl FixTagHandler for TestFixRemote {
	fn tag_value(&mut self, tag: u32, value: &[u8])
	{
		let buf = unsafe {&mut *self.data.get()};

		let (sum, len) = util::put_tag_id_eq(tag, buf);
		self.sum += sum;
		self.len += len;
		
		let (sum, len) = util::put_tag_val_soh(value, buf);
		self.sum += sum;
		self.len += len;
	}
}
