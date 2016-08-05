pub type FixStreamException = String;
pub type FixParseIdLenSum = (u32, usize, u32);
pub enum PhantomType {}

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

use std::fmt::Error;
use std::fmt::Debug;
use std::fmt::Formatter;
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

pub trait FixStream<T>: FixTagHandler 
where T: FixAppMsgType
{
    fn fix_message_done(&mut self, res: Result<(), FixStreamException>);
    fn fix_message_start(&mut self, msg_type: FixMsgType<T>, is_replayable: bool);
}

pub trait FixOutChannel<T>
where T: FixAppMsgType 
{
    type FMS;
    fn get_out_stream(&mut self) -> &mut Self::FMS
        where Self::FMS: FixStream<T>;
}

pub trait FixInChannel {
    fn read_fix_message<M, T>(&mut self, &mut T) where T: FixStream<M>, M: FixAppMsgType;
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

/*
impl<'a, T> FixAppMsgType<'a> for FixMsgType<'a, T>
where T: FixAppMsgType
{
    fn lookup<'b: 'a>(btype: &'b[u8]) -> Option<Self>
    {
        match btype.len() {
            1 => {
                match btype[0] as char {
                    'A' => return Some(FixMsgType::Logon),
                    // TODO: add rest of session level messages
                    _ => { /* fall thru */ },
                }
            },
            _ => { /* fall thru */ },
        }
        
        match T::lookup(btype)  {
            Some(app_type) => return Some(FixMsgType::Custom(app_type)),
            _ => {},
        }

        Some(FixMsgType::Unknown(&btype))
    }
}
*/

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
    pub fn lookup<'a: 'b>(btype: &'a [u8]) -> FixMsgType<T> {
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
    pub fn get_value(self) -> &'static str {
        match self {
            FixMsgType::Logon => {
                return "A"
            },
            _ => {
                return "Unknown"
            },
        }
    }
}
