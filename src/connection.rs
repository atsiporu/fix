use fix::*;
use util::*;
use std::marker::PhantomData;

/// ///////////////////////////////////////////////////////////////
/// Temporary
/// ///////////////////////////////////////////////////////////////

enum Void {}
impl FixAppMsgType for Void {
    fn lookup(btype: &[u8]) -> Option<Self>
        where Self: Sized
    {
        None
    }
}

impl FixAppMsgType for () {
    fn lookup(btype: &[u8]) -> Option<Self>
        where Self: Sized
    {
        None
    }
}

#[derive(Debug)]
pub enum ConnectionType {
    Initiator,
    Acceptor,
}

pub struct FixConnection<T, E>
    where T: FixTransport,
          E: FixTimerFactory
{
    timers: E,
    transport: Option<T>,
    in_state: FixInState,
    out_state: FixOutState,
    conn_type: ConnectionType,
    nextInSeq: u32,
    fix_writer: FixMessageWriter<()>,
}

pub struct FixSessionHandler<'a, A>
    where A: 'a + FixApplication
{
    app_handler: Option<&'a mut A>,
}


impl<T, E> FixConnection<T, E>
    where T: FixTransport,
          E: FixTimerFactory
{
    pub fn new(transport: T, timers: E, conn_type: ConnectionType) -> FixConnection<T, E> {
        FixConnection {
            timers: timers,
            transport: Some(transport),
            in_state: FixInState::Disconnected,
            out_state: FixOutState::Disconnected,
            conn_type: conn_type,
            nextInSeq: 0,
            fix_writer: FixMessageWriter::new(),
        }
    }

    pub fn read_message<S>(&mut self, fm: &mut S, t: &mut T)
        where S: FixApplication
    {
        println!("Read message!");

        let parse_result = {
            let mut fsh = FixSessionHandler { app_handler: Some(fm) };
            super::util::parse_fix_message(t.view(), &mut fsh)
        };

        match parse_result {
            Ok(Some(size)) => {
                t.consume(size);
            }
            Ok(None) => {
                t.on_read(|transport| {
                    println!("On read!");
                });
            }
            Err(_) => {
                // shutdown
            }
        }
    }
}

impl<T, E> FixService for FixConnection<T, E>
    where T: FixTransport,
          E: FixTimerFactory
{
    fn connect<L>(&mut self, l: &mut L)
        where L: FixApplication
    {
        println!("Connecting {:?} in: {:?} out {:?}",
                 self.conn_type,
                 self.in_state,
                 self.out_state);

        match self.conn_type {
            ConnectionType::Initiator => {
                // TODO: Open output stream, send logon
                self.in_state = FixInState::Disconnected;
                self.out_state = FixOutState::Logon;

                let mut transport = self.transport.take().unwrap();
                transport.connect(|transport| {
                    self.transport = Some(transport);
                    l.on_request(FixMsgType::Logon as FixMsgType<Void>, self);
                });
            }
            ConnectionType::Acceptor => {
                // TODO: Start listening for logon
                self.in_state = FixInState::Logon;
                self.out_state = FixOutState::Disconnected;

                let mut transport = self.transport.take().unwrap();
                transport.connect(|mut transport| {
                    println!("Connection accepted!");
                    transport.on_read(|transport| {
                        self.read_message(l, transport);
                    });
                    self.transport = Some(transport);
                });
            }
        }
        println!("Connected {:?} in: {:?} out {:?}",
                 self.conn_type,
                 self.in_state,
                 self.out_state);
    }

    fn request_done(&mut self) {}
}

impl<T, E> FixSessionControl for FixConnection<T, E>
    where T: FixTransport,
          E: FixTimerFactory
{
    fn start_session<L>(&mut self, l: &mut L) where L: FixApplication {}

    fn end_session<L>(&mut self, l: &mut L)
        where L: FixApplication
    {
        match self.in_state {
            FixInState::Disconnected => {
                // nothing need to be done here
            }
            _ => {
                self.in_state = FixInState::Disconnected;
                self.out_state = FixOutState::Logout;
                l.on_request(FixMsgType::Logout as FixMsgType<Void>, self);
            }
        }
    }

    fn force_expected_incoming_seq(&mut self, seq: u32) {
        self.nextInSeq = seq;
    }

    fn get_expected_incoming_seq(&mut self) -> u32 {
        self.nextInSeq
    }
}

impl<T, E> FixInChannel for FixConnection<T, E>
    where T: FixTransport,
          E: FixTimerFactory
{
    fn read_fix_message<L>(&mut self, fm: &mut L)
        where L: FixApplication
    {
        let mut transport = self.transport.take().unwrap();
        self.read_message(fm, &mut transport);
        self.transport = Some(transport);
    }
}

impl<T, E> FixOutChannel for FixConnection<T, E>
    where T: FixTransport,
          E: FixTimerFactory
{
    type FMS = FixConnection<T, E>;

    fn get_out_stream(&mut self) -> &mut Self::FMS {
        self
    }
}

impl<T, E> FixTagHandler for FixConnection<T, E>
    where T: FixTransport,
          E: FixTimerFactory
{
    fn tag_value(&mut self, t: u32, v: &[u8]) {
        self.fix_writer.tag_value(t, v);
    }
}

impl<T, E> FixStream for FixConnection<T, E>
    where T: FixTransport,
          E: FixTimerFactory
{
    type MSG_TYPES = ();
    fn fix_message_done(&mut self, res: Result<(), FixStreamException>) {
        println!("Fix message done!");
        self.fix_writer.fix_message_done(res);

        let len = {
            let msg = self.fix_writer.get_bytes();
            self.transport.as_mut().unwrap().write(msg)
        };

        self.fix_writer.drain_head(len);
    }
    fn fix_message_start(&mut self, msg_type: FixMsgType<Self::MSG_TYPES>, is_replayable: bool) {
        println!("Fix message start!");
        self.fix_writer.fix_message_start(msg_type, is_replayable);
    }
}

impl<T, E> FixErrorChannel for FixConnection<T, E>
    where T: FixTransport,
          E: FixTimerFactory
{
    fn error(&mut self, e: FixStreamException) {}
}

impl<'a, A> FixStream for FixSessionHandler<'a, A>
    where A: 'a + FixApplication
{
    type MSG_TYPES = <A::FIX_STREAM as FixStream>::MSG_TYPES;
    fn fix_message_start(&mut self, msg_type: FixMsgType<Self::MSG_TYPES>, is_replayable: bool) {
        print!("Connection got ");
        match msg_type {
            FixMsgType::Logon => {
                println!("Logon");
                self.app_handler = None;
            }
            _ => {
                self.app_handler.as_mut().map(|fix_app| {
                    fix_app.app_handler().fix_message_start(msg_type, is_replayable);
                });
            }
        }
    }
    fn fix_message_done(&mut self, res: Result<(), FixStreamException>) {
        self.app_handler.as_mut().map(|fix_app| {
            fix_app.app_handler().fix_message_done(res);
        });
    }
}

impl<'a, A> FixTagHandler for FixSessionHandler<'a, A>
    where A: 'a + FixApplication
{
    fn tag_value(&mut self, t: u32, v: &[u8]) {
        self.app_handler.as_mut().map(|fix_app| {
            fix_app.app_handler().tag_value(t, v);
        });
    }
}
