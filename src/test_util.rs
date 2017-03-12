use fix::*;
use std::result::Result;
use std::fmt::format;
use connection::FixConnection;
use connection::ConnectionType;
use super::util;
use std::collections::HashMap;
use std::cell::{RefCell, UnsafeCell};
use std::str;
use std::string::String;
use std::time::{Duration, SystemTime};
use std::rc::Rc;
use std::ptr;

extern crate sentinel_list;
use self::sentinel_list::{List, ListHandle};

const ASCII_ZERO: i32 = ('0' as i32);

pub struct TestFixApplication
{
    pub message: TestFixMessage,
    pub requests: Vec<SessionRequest>,
}

pub struct TestFixEnvironment
{
	now: SystemTime,
	timers: List<TestFixTimerDesc>,
}

pub struct TestFixTimerDesc
{
	d: Duration,
	last: SystemTime,
	on_timeout: Box<Fn()->()>,
}

pub struct TestFixTimerHandler<T>
where T: ListHandle<TestFixTimerDesc>
{
	timerh: T,
}

pub struct TestFixTransport {
	remote: Rc<RefCell<TestFixRemote>>,
}

pub struct TestFixRemote {
	sum: u32,
	len: usize,
	lenOff: usize,
	data: UnsafeCell<Vec<u8>>,
    pend_accept: Vec<Box<FnOnce() -> ()>>,
    pend_read: Vec<Box<FnOnce() -> ()>>,
}

pub struct TestFixMessage {
	pub msg_type: Option<String>,
	pub is_replayable: bool,
	pub done: bool,
	pub tag_ids: Vec<u32>,
	pub tag_values: HashMap<u32, String>,
}

impl FixAppMsgType for () {
    fn lookup(btype: &[u8]) -> Option<Self>
        where Self: Sized
    {
        None
    }
}

impl TestFixEnvironment
{
	pub fn new() -> TestFixEnvironment {
		TestFixEnvironment {
			now: SystemTime::now(),
			timers: List::new(),
		}
	}

	pub fn run_for(&mut self, d: Duration)
	{
		for th in self.timers.iter_mut() {
			self.now += d;
			let num = (d.as_secs() * 1_000_000_000 + d.subsec_nanos() as u64) / (th.d.as_secs() * 1_000_000_000 + th.d.subsec_nanos() as u64);
			for _ in 0..num {
				(th.on_timeout)();
				th.last += th.d;
			}
		}
	}
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
		let t = String::from_utf8(Vec::from(msg_type.as_bytes())).unwrap();
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
            pend_read: vec![],
            pend_accept: vec![],
		}
	}

    pub fn accept(&mut self)
    {
        self.pend_accept.pop().unwrap()();
    }
}

pub fn fix_parts(remote: Rc<RefCell<TestFixRemote>>) -> (TestFixTransport, TestFixEnvironment, TestFixApplication)
{
    let tft = TestFixTransport { remote: remote, };
    let env = TestFixEnvironment::new();
    let fix_app = TestFixApplication { requests: vec![], message: TestFixMessage::new() };
    (tft, env, fix_app)
}

impl<T> FixTimerHandler for TestFixTimerHandler<T>
where T: ListHandle<TestFixTimerDesc>
{
	fn cancel(self) {
		self.timerh.unlink();
	}
}

impl FixTimerFactory for TestFixEnvironment
{
	fn set_timeout<F>(&mut self, on_timeout: F, duration: Duration) -> Box<FixTimerHandler>
		where F: Fn() -> () + Send + 'static
	{
		let mut desc = TestFixTimerDesc {
			d: duration,
			last: self.now,
			on_timeout: Box::new(on_timeout),
		};

		let h = self.timers.push_tail(desc);

		box TestFixTimerHandler {
			timerh: h,
		}
	}
}

impl FixTransport for TestFixTransport
{
	fn connect<F>(&mut self, mut on_success: F) where F: FnOnce(&mut Self) -> ()
	{
       //self.remote.borrow_mut().pend_accept.push(box move || {
       //    on_success(self);
       //});
	}

	fn view(&self) -> &[u8] {
		unsafe {& *self.remote.borrow().data.get()}
	}

	fn consume(&mut self, len: usize) {
		unsafe {let _: Vec<u8> = (&mut *self.remote.borrow().data.get()).drain(0..len).collect();}
	}

    fn write(&mut self, buf: &[u8]) -> usize {
        buf.len()
    }
	
    fn on_read<F>(&mut self, mut on_success: F) where F: FnOnce(&mut Self) -> ()
	{
		on_success(self);
	}
}

impl FixApplication for TestFixApplication
{
    type FIX_STREAM = TestFixMessage;

    fn on_request<S>(&mut self, r: SessionRequest, svs: &mut S)
    where S: FixService
    {
        self.requests.push(r);
    }

    fn on_message_pending<C>(&mut self, in_ch: &mut C)
        where C: FixInChannel
    {
        in_ch.read_fix_message(self);
    }

    fn in_stream(&mut self) -> &mut Self::FIX_STREAM
    {
        &mut self.message
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
		let v = msg_type.as_bytes();
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
