use std::convert::From;
use std::convert::Into;
use std::fmt::Error;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::time::Duration;

pub type FixStreamException = String;
pub type FixParseIdLenSum = (u32, usize, u32);

pub trait FixAppMsgType {
	fn lookup(btype: &[u8]) -> Option<Self> where Self: Sized;
}

pub enum FixMsgType<'a, T>
where T:FixAppMsgType {
	Logon,
	Logout,
	SeqReset,
	Heartbeat,
	TestRequest,
	ResendRequest,
	Custom(T),
	Unknown(&'a[u8]),
}

impl<'a, T: FixAppMsgType> Debug for FixMsgType<'a, T> {
	fn fmt(&self, fmtr: &mut Formatter) -> Result<(), Error>
	{
		Ok(())
	}
}


#[derive(Debug)]
pub enum FixOutState {
	Disconnected,
	Logon,
	Logout,
	Connected,
	Resending,
	Lagging,
}

#[derive(Debug)]
pub enum FixInState {
	Disconnected,
	Logon,
	Logout,
	Connected,
}

#[derive(Debug)]
pub enum ParseControl {
	Error(FixStreamException),
	Stop,
	Continue,
	Skip,
}

pub trait FixTagHandler {
	fn tag_value(&mut self, t: u32, v: &[u8]);
}

pub trait FixStream: FixTagHandler 
{
	type MSG_TYPES: FixAppMsgType;
	fn fix_message_done(&mut self, res: Result<(), FixStreamException>);
	fn fix_message_start(&mut self, msg_type: FixMsgType<Self::MSG_TYPES>, is_replayable: bool);
}

pub trait FixOutChannel
{
	type FMS;
	fn get_out_stream(&mut self) -> &mut Self::FMS
		where Self::FMS: FixStream;
}

pub trait FixInChannel {
	fn read_fix_message<T>(&mut self, &mut T) where T: FixStream;
}

pub trait FixErrorChannel {
	fn error(&mut self, FixStreamException); // TODO: Think
}

pub trait FixSessionControl {
	fn start_session(&mut self);
	fn end_session(&mut self);
	fn force_expected_incoming_seq(&mut self, seq: u32);
	fn get_expected_incoming_seq(&mut self) -> u32;
}

pub trait CustomHeaderInjector : FixTagHandler {} 

pub trait TimerHandler {
    fn cancel(self);
}

pub trait FixEnvironment {
    type TH: TimerHandler;
	fn set_timeout<F>(&mut self, on_timeout: F, duration: Duration) -> Self::TH
	   where F: FnMut() -> ();
}

pub trait FixTransport {
	fn connect<F>(&self, on_success: F) where F: Fn() -> ();
	fn view(&self) -> &[u8];
	fn consume(&self, len: usize);
	fn write(&self, buf: &[u8]) -> usize;
}

pub trait FixConnection {
	fn connect(&mut self);
}

pub trait FixApplication {
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

impl<'b, T> AsRef<[u8]> for FixMsgType<'b, T>
where T: FixAppMsgType
{
	fn as_ref(&self) -> &[u8]
	{
		match *self {
			FixMsgType::Logon => {
				return "A".as_bytes()
			},
			FixMsgType::Unknown(t) => {
				return t;
			},
			_ => {
				return "Unknown".as_bytes()
			},
		}
	}
}

impl<'b, T> FixMsgType<'b,T>
where T: FixAppMsgType {
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
	
	pub fn _get_value<'a>(self) -> &'a[u8]
	where 'b:'a 
	{
		match self {
			FixMsgType::Logon => {
				return "A".as_bytes()
			},
			FixMsgType::Unknown(t) => {
				return t;
			},
			_ => {
				return "Unknown".as_bytes()
			},
		}
	}
}
