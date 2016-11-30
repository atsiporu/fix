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

enum Void {}
impl FixAppMsgType for Void {
	fn lookup(btype: &[u8]) -> Option<Self> where Self: Sized
    {
        None
    }
}

impl FixAppMsgType for () {
	fn lookup(btype: &[u8]) -> Option<Self> where Self: Sized { None }
}

#[derive(Debug)]
pub enum ConnectionType {
    Initiator,
    Acceptor,
}

pub struct FixConnection<T, E, L> 
where T: FixTransport, E: FixTimerFactory, L: FixSessionListener
{
	timers: E,
	transport: T,
	in_state: FixInState,
	out_state: FixOutState,
	out_stream: NullFixMessage,
    conn_type: ConnectionType,
    nextInSeq: u32,
    listener: L,
}

pub struct FixSessionHandler<'a, S, T, E, L> 
where S: 'a + FixStream, T: 'a + FixTransport, E: 'a + FixTimerFactory, L: 'a + FixSessionListener
{
	fc: &'a FixConnection<T, E, L>,
	app_handler: Option<&'a mut S>,
}


impl<T, E, L> FixConnection<T, E, L>
where T: FixTransport, E: FixTimerFactory, L: FixSessionListener
{
	pub fn new(transport: T, timers: E, conn_type: ConnectionType, listener: L) -> FixConnection<T, E, L>
	{
		FixConnection {
			timers: timers,
			transport: transport,
			in_state: FixInState::Disconnected,
			out_state: FixOutState::Disconnected,
			out_stream: NullFixMessage,
            conn_type: conn_type,
            nextInSeq: 0,
            listener: listener,
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

impl<T, E, L> FixService for FixConnection<T, E, L>
where T: FixTransport, E: FixTimerFactory, L: FixSessionListener
{
	fn connect(&mut self)
	{
        println!("Connecting {:?} in: {:?} out {:?}", self.conn_type, self.in_state, self.out_state);
        
        match self.conn_type {
            ConnectionType::Initiator => {
                // TODO: Open output stream, send logon
                self.in_state = FixInState::Disconnected;
                self.out_state = FixOutState::Logon;
                
				let l = &mut self.listener;
                self.transport.connect(|| {
                    l.on_request(FixMsgType::Logon as FixMsgType<Void>);
                });
            },
            ConnectionType::Acceptor => {
                // TODO: Start listening for logon
                self.in_state = FixInState::Logon;
                self.out_state = FixOutState::Disconnected;
                
                self.transport.connect(|| {
                    println!("Connection accepted!");
                });
            }
        }
        println!("Connected {:?} in: {:?} out {:?}", self.conn_type, self.in_state, self.out_state);
	}
}

impl<T, E, L> FixSessionControl for FixConnection<T, E, L>
where T: FixTransport, E: FixTimerFactory, L: FixSessionListener
{
	fn start_session(&mut self)
    {
    }
	
    fn end_session(&mut self)
    {
        match self.in_state {
            FixInState::Disconnected => {
                // nothing need to be done here
            },
            _ => {
                self.in_state = FixInState::Disconnected;
                self.out_state = FixOutState::Logout;
                self.listener.on_request(FixMsgType::Logout as FixMsgType<Void>);
            },
        }
    }

	fn force_expected_incoming_seq(&mut self, seq: u32)
    {
        self.nextInSeq = seq;
    }

	fn get_expected_incoming_seq(&mut self) -> u32
    {
        self.nextInSeq
    }
}

impl<T, E, L> FixInChannel for FixConnection<T, E, L> 
where T: FixTransport, E: FixTimerFactory, L: FixSessionListener
{
	fn read_fix_message<S>(&mut self, fm: &mut S) 
	where S: FixStream
	{
		self.read_message(fm);
	}
}

impl<T, E, L>  FixOutChannel for FixConnection<T, E, L>
where T: FixTransport, E: FixTimerFactory, L: FixSessionListener
{
	type FMS = NullFixMessage;

	fn get_out_stream(&mut self) -> &mut Self::FMS
	{
		&mut self.out_stream
	}
}

impl<T, E, L>  FixErrorChannel for FixConnection<T, E, L>
where T: FixTransport, E: FixTimerFactory, L: FixSessionListener
{
	fn error(&mut self, e: FixStreamException)
	{
	}
}

impl<'a, S, T, E, L> FixStream for FixSessionHandler<'a, S, T, E, L>
where S: 'a + FixStream, T: 'a + FixTransport, E: 'a + FixTimerFactory, L: 'a + FixSessionListener
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

impl<'a, S, T, E, L> FixTagHandler for FixSessionHandler<'a, S, T, E, L>
where S: 'a + FixStream, T: 'a + FixTransport, E: 'a + FixTimerFactory, L: 'a + FixSessionListener
{
	fn tag_value(&mut self, t: u32, v: &[u8]) {
		match self.app_handler {
			Some(ref mut fm) => {fm.tag_value(t, v)},
			None => {}
		}
	}
}


