use fix::*;
use util::*;
use std::marker::PhantomData;

/// ///////////////////////////////////////////////////////////////
/// Temporary
/// ///////////////////////////////////////////////////////////////


#[derive(Debug)]
pub enum ConnectionType {
    Initiator,
    Acceptor,
}

pub struct FixConnection<'a, T, E>
    where T: 'a + FixTransport,
          E: FixTimerFactory
{
    timers: E,
    transport: Option<&'a mut T>,
    in_state: FixInState,
    out_state: FixOutState,
    conn_type: ConnectionType,
    nextInSeq: u32,
    fix_writer: FixMessageWriter<()>,
}

impl<'a, T, E> FixConnection<'a, T, E>
    where T: FixTransport,
          E: FixTimerFactory
{
    pub fn new(version: String, transport: &mut T, timers: E, conn_type: ConnectionType) -> FixConnection<'a, T, E> 
    {
        FixConnection {
            timers: timers,
            transport: Some(transport),
            in_state: FixInState::Disconnected,
            out_state: FixOutState::Disconnected,
            conn_type: conn_type,
            nextInSeq: 0,
            fix_writer: FixMessageWriter::new(version),
        }
    }

    pub fn read_message<S>(&mut self, app: &mut S)
        where S: FixApplication
    {
        match self._read_message(app) {
            Ok(Some((size, request))) => {
                println!("Success! {:?}", size);
                self.transport.as_mut().unwrap().consume(size);
                if request.is_some() {
                    app.on_request(request.unwrap(), self);
                }
            }
            Ok(None) => {
                /*
                self.transport.as_mut().unwrap().on_read(|transport| {
                    self.transport = Some(transport);
                    println!("On read!");
                });
                */
            }
            Err(e) => {
                // shutdown
                println!("Error! {:?}", e);
            }
        }
    }

    pub fn _read_message<S>(&mut self, app: &mut S) -> Result<Option<(usize, Option<SessionRequest>)>, FixStreamException>
        where S: FixApplication
    {
        let mut request = None;

        let mut t = self.transport.as_ref().unwrap();
        
        println!("Read message buf: {:?}", String::from_utf8_lossy(t.view()));
        
        let mut chksum = 0;
        let mut s = super::util::Slicer {buf: t.view(), len: 0};
        let required_tags = [8, 9, 35];
        
        for eid in &required_tags[..] {
            //println!("total: {:?}", String::from_utf8_lossy(s.buf()));
            let res = super::util::get_tag(&mut s);
            match res {
                Ok(Some((id, _, _))) if id != *eid as u32 => {
                    return Err(format!("Missplaced tag expected {} got {}", *eid, id));
                },
                Ok(Some((id, v, sum))) if id == 35u32 => {
                    chksum += sum;
                    let msg_type = FixMsgType::from(v);
                    request = Option::<SessionRequestType>::from(&msg_type);
                    if request.is_none() {
                        app.in_stream().fix_message_start(msg_type, true);
                    }
                },
                Ok(Some((id, _, sum))) => {
                    chksum += sum;
                },
                Ok(None) => return Ok(None),
                Err(err) => return Err(err),
            };
        }

        loop {
            //println!("tags: {:?}", String::from_utf8_lossy(s.buf()));
            match super::util::get_tag(&mut s) {
                Ok(Some((id, v, sum))) if id != 10u32 => {
                    chksum += sum;
                    println!("tag: {} {:?}", id, String::from_utf8_lossy(v));
                    if request.is_none() {
                        app.in_stream().tag_value(id, v);
                    }
                },
                Ok(Some((_, v, _))) => {
                    let sum = v.iter().fold(0, |acc: u32, &x| acc * 10 + (x as u32 -'0' as u32));
                    if sum == (chksum % 256) { 
                        if request.is_none() {
                            app.in_stream().fix_message_done(Ok(()));
                        }
                        return Ok(Some((s.len, request.and_then(|v| Some(SessionRequest::In(v))))));
                    }
                    let err_str = format!("Malformed message: calc sum {} != {}", chksum%256, sum);
                    if request.is_none() {
                        app.in_stream().fix_message_done(Err(err_str));
                    }
                    let err_str = format!("Malformed message: calc sum {} != {}", chksum%256, sum);
                    return Err(err_str);
                },
                Err(err) => {
                    return Err(err)
                },
                Ok(None) => {
                    return Ok(None)
                },
            }
        }
    }
}

impl<'a, T, E> FixService for FixConnection<'a, T, E>
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

                let t : &'a mut T = self.transport.take().unwrap();

                t.connect(move |transport| {
                    self.transport = Some(transport);
                    l.on_request(SessionRequest::In(SessionRequestType::Logon(None)), self);
                });
            }
            ConnectionType::Acceptor => {
                // TODO: Start listening for logon
                self.in_state = FixInState::Logon;
                self.out_state = FixOutState::Disconnected;

                self.transport.take().unwrap().connect(|mut transport| {
                    println!("Connection accepted!");
                    transport.on_read(|transport| {
                        self.transport = Some(transport);
                        self.read_message(l);
                    });
                });
            }
        }
        println!("Connected {:?} in: {:?} out {:?}",
                 self.conn_type,
                 self.in_state,
                 self.out_state);
    }

    fn request_done(&mut self, r: SessionRequest)
    {
        self.fix_message_start(FixMsgType::Logon, false);
        self.fix_message_done(Ok(()));
        let match_me = (self.out_state, r);
        match match_me {
            (FixOutState::Connected, SessionRequest::In(SessionRequestType::Logon(_))) => {
            //   self.fix_message_start(FixMsgType::Logon, false);
            //   self.fix_message_done(Ok(()));
            },
            _ => {
            }
        }
    }
}

impl<'a, T, E> FixSessionControl for FixConnection<'a, T, E>
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
                l.on_request(SessionRequest::In(SessionRequestType::Logout(None)), self);
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

impl<'a, T, E> FixInChannel for FixConnection<'a, T, E>
    where T: FixTransport,
          E: FixTimerFactory
{
    fn read_fix_message<L>(&mut self, app: &mut L)
        where L: FixApplication
    {
        self.read_message(app);
    }
}

impl<'a, T, E> FixOutChannel for FixConnection<'a, T, E>
    where T: FixTransport,
          E: FixTimerFactory
{
    type FMS = FixConnection<'a, T, E>;

    fn get_out_stream(&mut self) -> &mut Self::FMS {
        self
    }
}

impl<'a, T, E> FixTagHandler for FixConnection<'a, T, E>
    where T: FixTransport,
          E: FixTimerFactory
{
    fn tag_value(&mut self, t: u32, v: &[u8]) {
        self.fix_writer.tag_value(t, v);
    }
}

impl<'a, T, E> FixStream for FixConnection<'a, T, E>
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
	fn fix_message_start(&mut self, msg_type: FixMsgType<Self::MSG_TYPES>, is_replayable: bool)
    {
        println!("Fix message start!");
        self.fix_writer.fix_message_start(msg_type, is_replayable);
    }
}

impl<'a, T, E> FixErrorChannel for FixConnection<'a, T, E>
    where T: FixTransport,
          E: FixTimerFactory
{
    fn error(&mut self, e: FixStreamException) {}
}

