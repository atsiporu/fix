use fix::*;
use util::*;
use std::marker::PhantomData;

//////////////////////////////////////////////////////////////////
// Temporary
pub struct NullFixMessage;
impl FixTagHandler for NullFixMessage {
	fn tag_value(&mut self, t: u32, v: &[u8]) { }
}
impl FixStream for NullFixMessage
{
	type MSG_TYPES = ();
	fn fix_message_done(&mut self, res: Result<(), FixStreamException>) {}
	fn fix_message_start(&mut self, msg_type: FixMsgType<Self::MSG_TYPES>, is_replayable: bool)
	{
	}
}
//////////////////////////////////////////////////////////////////

impl FixAppMsgType for () {
	fn lookup(btype: &[u8]) -> Option<Self> where Self: Sized { None }
}

pub struct FixClient<'a, T, E> 
where T: 'a + FixTransport, E: 'a + FixTimerFactory
{
	conn: &'a mut FixConnection<T, E>,
}

pub struct FixServer<'a, T, E>
where T: 'a + FixTransport, E: 'a + FixTimerFactory
{
	conn: &'a mut FixConnection<T, E>,
}

pub struct FixConnection<T, E> 
where T: FixTransport, E: FixTimerFactory
{
	timers: E,
	transport: T,
	in_state: FixInState,
	out_state: FixOutState,
	out_stream: NullFixMessage,
}

impl<'a, T, E> FixService for FixServer<'a, T, E>
where T: 'a + FixTransport, E: 'a + FixTimerFactory
{
	fn connect(&mut self)
	{
		// TODO: Start listening for logon
		self.conn.in_state = FixInState::Logon;
		self.conn.out_state = FixOutState::Disconnected;
	}
}

impl<'a, T, E> FixService for FixClient<'a, T, E>
where T: 'a + FixTransport, E: 'a + FixTimerFactory
{
	fn connect(&mut self)
	{
		// TODO: Open output stream, send logon
		self.conn.in_state = FixInState::Logon;
		self.conn.out_state = FixOutState::Logon;
	}
}

pub struct FixSessionHandler<'a, S, T, E> 
where S: 'a + FixStream, T: 'a + FixTransport, E: 'a + FixTimerFactory
{
	fc: &'a FixConnection<T, E>,
	app_handler: Option<&'a mut S>,
}


impl<T, E> FixConnection<T, E>
where T: FixTransport, E: FixTimerFactory
{
	pub fn new(transport: T, timers: E) -> FixConnection<T, E>
	{
		FixConnection {
			timers: timers,
			transport: transport,
			in_state: FixInState::Disconnected,
			out_state: FixOutState::Disconnected,
			out_stream: NullFixMessage,
		}
	}

	pub fn read_message<S>(&mut self, fm: &mut S) 
	where S: FixStream
	{
		let parse_result = {
			let mut fsh = FixSessionHandler { fc: self, app_handler: Some(fm) };
			super::util::parse_fix_message(self.transport.view(), &mut fsh)
		};
		
		match parse_result {
			Ok(Some(size)) => { self.transport.consume(size); }
			Ok(None) => { /* not full message */ }
			Err(_) => {/* shutdown */}
		}
	}
}

impl<T, E> FixInChannel for FixConnection<T, E> 
where T: FixTransport, E: FixTimerFactory
{
	fn read_fix_message<S>(&mut self, fm: &mut S) 
	where S: FixStream
	{
		self.read_message(fm);
	}
}

impl<T, E>  FixOutChannel for FixConnection<T, E>
where T: FixTransport, E: FixTimerFactory
{
	type FMS = NullFixMessage;

	fn get_out_stream(&mut self) -> &mut Self::FMS
	{
		&mut self.out_stream
	}
}

impl<T, E>  FixErrorChannel for FixConnection<T, E>
where T: FixTransport, E: FixTimerFactory
{
	fn error(&mut self, e: FixStreamException)
	{
	}
}

impl<'a, S, T, E> FixStream for FixSessionHandler<'a, S, T, E>
where S: 'a + FixStream, T: 'a + FixTransport, E: 'a + FixTimerFactory
{
	type MSG_TYPES = S::MSG_TYPES;
	fn fix_message_start(&mut self, msg_type: FixMsgType<Self::MSG_TYPES>, is_replayable: bool)
	{
		match msg_type {
			FixMsgType::Logon => {
				self.app_handler = None;
			},
			_ => {
				match self.app_handler {
					Some(ref mut fm) => {fm.fix_message_start(msg_type, is_replayable)},
					None => {}
				}
			},
		}
	}
	fn fix_message_done(&mut self, res: Result<(), FixStreamException>) {
		match self.app_handler {
			Some(ref mut fm) => {fm.fix_message_done(res)},
			None => {}
		}
	}
}

impl<'a, S, T, E> FixTagHandler for FixSessionHandler<'a, S, T, E>
where S: 'a + FixStream, T: 'a + FixTransport, E: 'a + FixTimerFactory
{
	fn tag_value(&mut self, t: u32, v: &[u8]) {
		match self.app_handler {
			Some(ref mut fm) => {fm.tag_value(t, v)},
			None => {}
		}
	}
}

