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

pub struct FixClient<'a, T:'a + FixTransport> {
    conn: &'a mut FixConnectionImpl<T>,
}

pub struct FixServer<'a, T:'a + FixTransport> {
    conn: &'a mut FixConnectionImpl<T>,
}

pub struct FixConnectionImpl<T> 
where T: FixTransport
{
    in_state: FixInState,
    out_state: FixOutState,
    transport: T,
    out_stream: NullFixMessage,
}

impl<'a, T:'a + FixTransport> FixConnection for FixServer<'a, T>
{
    fn connect(&mut self)
    {
        // TODO: Start listening for logon
        self.conn.in_state = FixInState::Logon;
        self.conn.out_state = FixOutState::Disconnected;
    }
}

impl<'a, T:'a + FixTransport> FixConnection for FixClient<'a, T>
{
    fn connect(&mut self)
    {
        // TODO: Open output stream, send logon
        self.conn.in_state = FixInState::Logon;
        self.conn.out_state = FixOutState::Logon;
    }
}

pub struct FixSessionHandler<'a, S, T> 
where S: 'a + FixStream, T: 'a + FixTransport
{
    fc: &'a FixConnectionImpl<T>,
    app_handler: Option<&'a mut S>,
}


impl<T: FixTransport> FixConnectionImpl<T> {
    pub fn new(transport: T) -> FixConnectionImpl<T>
    {
        FixConnectionImpl {
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

        /*
        let bytes = match self.transport {
            Some(ref transport) => transport.view(),
            None => &vec![0;0], // if transport is None pretend that there is no bytes to read
        };
        */
    }
}

impl<P> FixInChannel for FixConnectionImpl<P> 
where P: FixTransport
{
    fn read_fix_message<T>(&mut self, fm: &mut T) 
    where T: FixStream
    {
        self.read_message(fm);
    }
}

impl<T: FixTransport>  FixOutChannel for FixConnectionImpl<T>
{
    type FMS = NullFixMessage;

    fn get_out_stream(&mut self) -> &mut Self::FMS
    {
        &mut self.out_stream
    }
}

impl<T: FixTransport>  FixErrorChannel for FixConnectionImpl<T>
{
    fn error(&mut self, e: FixStreamException)
    {
    }
}

impl<'a, S, T> FixStream for FixSessionHandler<'a, S, T>
where S: 'a + FixStream, T: 'a + FixTransport
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

impl<'a, S, T> FixTagHandler for FixSessionHandler<'a, S, T>
where S: 'a + FixStream, T: 'a + FixTransport
{
    fn tag_value(&mut self, t: u32, v: &[u8]) {
        match self.app_handler {
            Some(ref mut fm) => {fm.tag_value(t, v)},
            None => {}
        }
    }
}


