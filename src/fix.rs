/*!
  Defines main iterfaces used by FIX applications
 */
use std::convert::From;
use std::convert::Into;
use std::fmt::Error;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::time::Duration;

pub type FixStreamException = String;
pub type FixParseIdLenSum = (u32, usize, u32);

/// Application level message types
pub trait FixAppMsgType {
	fn lookup(btype: &[u8]) -> Option<Self> where Self: Sized;
}

/// Session level message types
#[derive(Debug)]
pub enum FixMsgType<'a, T>
where T: FixAppMsgType {
	Logon,
	Logout,
	SeqReset,
	Heartbeat,
	TestRequest,
	ResendRequest,
	Custom(T),
	Unknown(&'a[u8]),
}

/// "Outgoing" connection state
#[derive(Debug)]
pub enum FixOutState {
	Disconnected,
	Logon,
	Logout,
	Connected,
	Resending,
	Lagging,
}

/// "Incoming" connection state
#[derive(Debug)]
pub enum FixInState {
	Disconnected,
	Logon,
	Logout,
	Connected,
}

/// To control the parsing of FIX messages
/// so that we can stop or error out early in the process
/// Not using this for now but it might be a useful thing
#[derive(Debug)]
pub enum ParseControl {
	Error(FixStreamException),
	Stop,
	Continue,
	Skip,
}

/// Tag-value processor, required to parse tag-value stream
pub trait FixTagHandler {
	fn tag_value(&mut self, t: u32, v: &[u8]);
}

/// FIX stream defines a stream of tag-value pairs with additional message boundaries
/// User should expect to get fix_message_start, followed by a number of tag_value calls
/// followed by fix_message_done
pub trait FixStream: FixTagHandler 
{
	type MSG_TYPES: FixAppMsgType;
	fn fix_message_done(&mut self, res: Result<(), FixStreamException>);
	fn fix_message_start(&mut self, msg_type: FixMsgType<Self::MSG_TYPES>, is_replayable: bool);
}

/// Output channel provides the "outgoing" stream.
pub trait FixOutChannel
{
	type FMS;
	fn get_out_stream(&mut self) -> &mut Self::FMS
		where Self::FMS: FixStream;
}

/// Input channel provides means to read incoming fix message onto the provided by
/// user stream. Usual use case when user (fix application) is ready to read another
/// fix message, it woud call read_fix_message providing the FixStream
pub trait FixInChannel 
{
	fn read_fix_message<L>(&mut self, &mut L) where L: FixApplication;
}

/// If fix application determines that there is an unrecovable exception for the 
/// ongoing fix session it should communicate to underlying fix "transport" layer
/// via calling error on FixErrorChannel
pub trait FixErrorChannel {
	fn error(&mut self, FixStreamException); // TODO: Think
}

/// FixApplication is configured with session control object that allows
/// start/end fix session as well as query and force expected incoming sequnce
pub trait FixSessionControl
{
	fn start_session<L>(&mut self, l: &mut L) where L: FixApplication;
	fn end_session<L>(&mut self, l: &mut L) where L: FixApplication;
	fn force_expected_incoming_seq(&mut self, seq: u32);
	fn get_expected_incoming_seq(&mut self) -> u32;
}

/// Every FixApplication has to implement this trait
/// The idea is for FixService to drive the process. 
pub trait FixApplication {
	type FIX_STREAM: FixStream;
	/// FixService request to perform certain action.
	fn on_request<T, S>(&mut self, msg: FixMsgType<T>, svs: &mut S) 
    where T: FixAppMsgType, S: FixService, <S as FixOutChannel>::FMS: FixStream;
	/// FixService notifies app that there are potentially
	/// message pending to be processed.
	/// TODO: Think maybe we should pass the FixInChannel here
	/// this would give us more flexibility, but I am not sure it's needed
	fn on_message_pending<C>(&mut self, in_ch: &mut C) where C: FixInChannel;

    fn app_handler(&mut self) -> &mut Self::FIX_STREAM;
}

/// TimerHandler that allows to cancel previously scheduled timeout
pub trait FixTimerHandler {
	fn cancel(self);
}

/// Facility to provide fix "transport" with ability to set timeouts
/// It abstracts the actual implementation of timers. 
/// ASUMPTION!!! Once timer is set it keeps firing until canceled
pub trait FixTimerFactory
{
	fn set_timeout<F>(&mut self, on_timeout: F, duration: Duration) -> Box<FixTimerHandler>
	   where F: Fn() -> () + Send;
}

/// Transport abstraction. FIX is a stream and thus the most tipical transport is 
/// TCP however any other stream will do, and as such there is a reson to abstract this.
pub trait FixTransport {
	fn connect<F>(self, on_success: F) where F: FnOnce(Self) -> (), Self: Sized;
	fn view(&self) -> &[u8];
	fn consume(&mut self, len: usize);
	fn write(&mut self, buf: &[u8]) -> usize;
	fn on_read<F>(&mut self, on_read: F) where F: FnOnce(&mut Self) -> ();
}

/// Abstraction representing either FixClient or FixServer
/// The former initiates connection to the listening remote site.
/// The latter listens for incoming connections. You can be one or the other.
pub trait FixService: FixInChannel + FixOutChannel + FixErrorChannel + FixSessionControl 
{
	fn connect<L>(&mut self, l: &mut L) where L: FixApplication;
    fn request_done(&mut self);
}

impl<'b, 'a:'b, T> From<&'a[u8]> for FixMsgType<'b, T>
where T: FixAppMsgType
{
	fn from(btype: &'a[u8]) -> Self 
	{
		match btype.len() {
			1 => {
				match btype[0] as char {
					'A' => return FixMsgType::Logon,
					_ => return FixMsgType::Unknown(&btype),
				}
			},
			_ => return FixMsgType::Unknown(&btype),
		}

		FixMsgType::Unknown(&btype)
	}
}

impl<'b, T> FixMsgType<'b,T>
where T: FixAppMsgType
{
	pub fn as_bytes(&self) -> &[u8]
	{
		match *self {
		    FixMsgType::Logon => { "A".as_bytes() },
			FixMsgType::Logout => { "5".as_bytes() }
			FixMsgType::SeqReset => { "5".as_bytes() }
			FixMsgType::Heartbeat => { "5".as_bytes() }
			FixMsgType::TestRequest => { "5".as_bytes() }
			FixMsgType::ResendRequest => { "5".as_bytes() }
			FixMsgType::Unknown(t) => { t },
			FixMsgType::Custom(ref t) => { "Custom".as_bytes() },
		}
	}

	pub fn is_session_level(msg_type: FixMsgType<T>) -> bool {
		match msg_type {
			FixMsgType::Logon | 
			FixMsgType::Logout | 
			FixMsgType::SeqReset | 
			FixMsgType::Heartbeat | 
			FixMsgType::TestRequest | 
			FixMsgType::ResendRequest => true,
			_ => false,
		}
	}
}
