#![feature(box_syntax)]

extern crate fixr;
extern crate tbr;

use std::sync::mpsc::{self, Sender, TryRecvError};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};
use fixr::fix::*;
use fixr::connection::{FixConnection, ConnectionType};
use std::io::prelude::*;
use std::ptr;
use std::net::{TcpListener, TcpStream};
use std::io::ErrorKind;
use std::sync::atomic::AtomicUsize;
use tbr::*;
use std::sync::{Arc, Mutex};

pub struct TestTransport
{
    connector: Box<Fn() -> TcpStream>,
    tcp: Option<Reader<TcpStream>>,
    reader: Option<JoinHandle<()>>
}

impl FixTransport for TestTransport
{
	fn connect<F>(mut self, mut on_success: F) where F: FnOnce(Self) -> (), Self: Sized
	{
        let (r, w) = ThreadedBufReader::with_capacity((self.connector)(), 1024);
        self.reader = Some(thread::spawn(move || {
            loop {
                w.fill_buf_local().unwrap();
                thread::sleep_ms(1);
            }
        }));

        self.tcp = Some(r);
		on_success(self);
	}

	fn view(&self) -> &[u8] {
        self.tcp
            .as_ref()
            .map_or(&[0;0], |stream| {
                stream.read()
            })
    }

	fn consume(&mut self, len: usize)
    {
        self.tcp
            .as_ref()
            .map(|stream| {
                stream.consume_local(len)
            });
	}

	fn write(&mut self, buf: &[u8]) -> usize {
		let len = buf.len();
        
        self.tcp
            .as_ref()
            .map_or(0, |stream| {
                
                let writer: &mut TcpStream = stream.into();
                println!("Writing {:?}", buf);
                writer.write(buf)
                      .or_else(|err| {
                          if err.kind() == ErrorKind::WouldBlock {
                              Ok(0)
                          }
                          else { 
                              Err(err) 
                          } 
                      }).unwrap()
            })
	}
	
    fn on_read<F>(&mut self, mut on_success: F) where F: FnOnce(&mut Self) -> ()
    {
        thread::spawn(move || {
            thread::sleep_ms(1000); // example sleep a second
        }).join();
        on_success(self);
    }
}


pub struct TestFixTimerHandler
{
	cancel_signal: Sender<()>,
}

impl FixTimerHandler for TestFixTimerHandler
{
	fn cancel(self) {
	}
}

pub struct TestFixEnvironment
{
}

pub struct ExamplFixApp
{
    stream: TestFixStream,
}

pub struct TestFixStream 
{
}

impl FixTagHandler for TestFixStream
{
	fn tag_value(&mut self, t: u32, v: &[u8])
    {
    }
}
impl FixStream for TestFixStream
{
    type MSG_TYPES = ();
    fn fix_message_done(&mut self, res: Result<(), FixStreamException>)
    {
    }
	fn fix_message_start(&mut self, msg_type: FixMsgType<Self::MSG_TYPES>, is_replayable: bool)
    {
    }
}

impl FixApplication for ExamplFixApp {
    type FIX_STREAM = TestFixStream;

	fn on_request<T, S>(&mut self, msg: FixMsgType<T>, svs: &mut S)
    where T: FixAppMsgType, S: FixService, <S as FixOutChannel>::FMS: FixStream
    {
        println!("on_request: Requested: {:?}", &msg.as_bytes());
        let stream = svs.get_out_stream();
        stream.fix_message_start(FixMsgType::Logon, false);
        stream.fix_message_done(Ok(()));
    }

    fn on_message_pending<C>(&mut self, in_ch: &mut C) 
    where C: FixInChannel
    {
        println!("Message pending!");
        in_ch.read_fix_message(self);
    }

    fn app_handler(&mut self) -> &mut Self::FIX_STREAM
    {
        &mut self.stream
    }
}

impl FixTimerFactory for TestFixEnvironment
{
	fn set_timeout<F>(&mut self, on_timeout: F, duration: Duration) -> Box<FixTimerHandler>
		where F: 'static + Fn() -> () + Send
	{
		let (tx, rx) = mpsc::channel();
		let timer_thread = thread::spawn(move || {
			let mut now = SystemTime::now();
			loop {
				match rx.try_recv() {
					Ok(_) | Err(TryRecvError::Disconnected) => {
						// timeout canceled
						break;
					},
					Err(_) => {} // other error continue
				};
				match now.elapsed() {
					Ok(elapsed) => {
						if elapsed > duration {
							on_timeout();
							now = SystemTime::now();
						}
						thread::sleep(duration / 3);
					},
					Err(err) => {
						println!("{}", err);
					},
				};
			}
		});

		box TestFixTimerHandler {
			cancel_signal: tx
		}
	}
}

use std::env;
use std::net::ToSocketAddrs;
use std::net::Ipv4Addr;

fn main()
{
	let args = &mut env::args();

	args.next(); // consume program name

	if args.len() != 3 {
		println!("usage: <ip> <port> <-c|-l>");
		return;
	}

	let ip: Vec<u8> = args.next().unwrap().split('.').map(|x| { u8::from_str_radix(x, 10).unwrap() }).collect();
	let port = u16::from_str_radix(args.next().unwrap().as_str(), 10).unwrap();
	let mode = args.next().unwrap();
    let sockaddr = (Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3]), port);
	
    let mut timers = TestFixEnvironment {};

    let (conn_type, stream): (ConnectionType, Box<Fn() -> TcpStream>) = match mode.as_str() {
		"-c" => {
            let stream_provider = move || {
                TcpStream::connect(sockaddr).unwrap()
            };
            (ConnectionType::Initiator, box stream_provider)
		},
		"-l" => {
            let listener = TcpListener::bind(sockaddr).unwrap();
            let stream_provider = move || {
                listener.accept().unwrap().0
            };
            (ConnectionType::Acceptor, box stream_provider)
		},
		_ => {
			panic!("usage: <ip> <port> <-c|-l>");
		}
	};

    let transport = TestTransport { connector: stream, tcp: None, reader: None }; 
    let mut conn = FixConnection::new(transport, timers, conn_type);
    let mut fix_app = ExamplFixApp { stream: TestFixStream {} };
    conn.connect(&mut fix_app);

	loop {
		thread::sleep_ms(100);
	}
}
