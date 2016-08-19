use fix::*;
use util::*;
use std::marker::PhantomData;

pub struct NullFixMessage;

impl FixTagHandler for NullFixMessage {
    fn tag_value(&mut self, t: u32, v: &[u8]) { }
}

impl FixAppMsgType for () {
    fn lookup(btype: &[u8]) -> Option<Self> where Self: Sized { None }
}

impl FixStream for NullFixMessage
{
    type MSG_TYPES = ();
    fn fix_message_done(&mut self, res: Result<(), FixStreamException>) {}
    fn fix_message_start(&mut self, msg_type: FixMsgType<Self::MSG_TYPES>, is_replayable: bool)
    {
    }
}

pub struct FixClient<'a, T, E> 
where T: 'a + FixTransport, E: 'a + FixEnvironment
{
    conn: &'a mut FixConnectionImpl<T, E>,
}

pub struct FixServer<'a, T, E>
where T: 'a + FixTransport, E: 'a + FixEnvironment
{
    conn: &'a mut FixConnectionImpl<T, E>,
}

pub struct FixConnectionImpl<T, E> 
where T: FixTransport, E: FixEnvironment
{
    environment: E,
    transport: T,
    in_state: FixInState,
    out_state: FixOutState,
    out_stream: NullFixMessage,
}

impl<'a, T, E> FixConnection for FixServer<'a, T, E>
where T: 'a + FixTransport, E: 'a + FixEnvironment
{
    fn connect(&mut self)
    {
        // TODO: Start listening for logon
        self.conn.in_state = FixInState::Logon;
        self.conn.out_state = FixOutState::Disconnected;
    }
}

impl<'a, T, E> FixConnection for FixClient<'a, T, E>
where T: 'a + FixTransport, E: 'a + FixEnvironment
{
    fn connect(&mut self)
    {
        // TODO: Open output stream, send logon
        self.conn.in_state = FixInState::Logon;
        self.conn.out_state = FixOutState::Logon;
    }
}

pub struct FixSessionHandler<'a, S, T, E> 
where S: 'a + FixStream, T: 'a + FixTransport, E: 'a + FixEnvironment
{
    fc: &'a FixConnectionImpl<T, E>,
    app_handler: Option<&'a mut S>,
}


impl<T, E> FixConnectionImpl<T, E>
where T: FixTransport, E: FixEnvironment
{
    pub fn new(transport: T, environment: E) -> FixConnectionImpl<T, E>
    {
        FixConnectionImpl {
            environment: environment,
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

impl<T, E> FixInChannel for FixConnectionImpl<T, E> 
where T: FixTransport, E: FixEnvironment
{
    fn read_fix_message<S>(&mut self, fm: &mut S) 
    where S: FixStream
    {
        self.read_message(fm);
    }
}

impl<T, E>  FixOutChannel for FixConnectionImpl<T, E>
where T: FixTransport, E: FixEnvironment
{
    type FMS = NullFixMessage;

    fn get_out_stream(&mut self) -> &mut Self::FMS
    {
        &mut self.out_stream
    }
}

impl<T, E>  FixErrorChannel for FixConnectionImpl<T, E>
where T: FixTransport, E: FixEnvironment
{
    fn error(&mut self, e: FixStreamException)
    {
    }
}

impl<'a, S, T, E> FixStream for FixSessionHandler<'a, S, T, E>
where S: 'a + FixStream, T: 'a + FixTransport, E: 'a + FixEnvironment
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
where S: 'a + FixStream, T: 'a + FixTransport, E: 'a + FixEnvironment
{
    fn tag_value(&mut self, t: u32, v: &[u8]) {
        match self.app_handler {
            Some(ref mut fm) => {fm.tag_value(t, v)},
            None => {}
        }
    }
}


