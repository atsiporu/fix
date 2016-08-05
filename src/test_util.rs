use fix::FixMsgType;
use fix::FixAppMsgType;
use fix::FixStream;
use fix::FixTagHandler;
use fix::FixStreamException;
use fix::FixParseIdLenSum;
use fix::FixTransport;
use std::result::Result;
use std::fmt::format;
use fix_connection::FixConnectionImpl;
use super::util;
use std::cell::RefCell;
use std::collections::HashMap;

extern crate scopefi;
use std::cell::UnsafeCell;

const ASCII_ZERO: i32 = ('0' as i32);

pub struct TestFixTransport<'b> {
    remote: &'b RefCell<TestFixRemote>,
}

pub struct TestFixRemote {
    sum: u32,
    len: u32,
    lenOff: usize,
    data: UnsafeCell<Vec<u8>>,
}

pub struct TestFixMessage {
    pub msg_type: Option<&'static str>,
    pub is_replayable: bool,
    pub done: bool,
    pub tag_ids: Vec<u32>,
    pub tag_values: HashMap<u32, Vec<u8>>,
}

impl TestFixMessage {
    pub fn new() -> TestFixMessage
    {
        TestFixMessage {
            msg_type: None,
            is_replayable: false,
            done: false,
            tag_ids: Vec::new(),
            tag_values: HashMap::new(),
        }
    }
}

impl FixTagHandler for TestFixMessage {
    fn tag_value(&mut self, t: u32, v: &[u8])
    {
        self.tag_ids.push(t);
        let mut my_v = Vec::new();
        my_v.extend_from_slice(v);
        self.tag_values.insert(t, my_v);
    }
}

impl<T> FixStream<T> for TestFixMessage
where T: FixAppMsgType  
{
    fn fix_message_done(&mut self, res: Result<(), FixStreamException>)
    {
        self.done = true;
    }

    fn fix_message_start(&mut self, msg_type: FixMsgType<T>, is_replayable: bool)
    {
        self.is_replayable = is_replayable;
        self.done = false;
        self.msg_type = Some(msg_type.get_value());
    }
}

impl TestFixRemote {
    pub fn new() -> TestFixRemote {
        TestFixRemote {
            sum: 0, 
            len: 0,
            lenOff: 0,
            data: UnsafeCell::new(vec![0u8;0]),
        }
    }
}

impl<'b> TestFixTransport<'b> {
    pub fn new<'a:'b>(remote: &'a RefCell<TestFixRemote>) -> FixConnectionImpl<TestFixTransport<'b>> {
        let tft = TestFixTransport {
            remote: remote,
        };

        FixConnectionImpl::new(tft)
    }
}

impl<'b> FixTransport for TestFixTransport<'b> {
    fn connect<F>(&self, on_success: F) where F: Fn() -> () {
    
    }

    fn view(&self) -> &[u8] {
        unsafe {& *self.remote.borrow().data.get()}
    }

    fn consume(&self, len: usize) {
        unsafe {let _: Vec<u8> = (&mut *self.remote.borrow().data.get()).drain(0..len).collect();}
    }

    fn write(&self, buf: &[u8]) -> usize {
        0usize
    }
}

impl<T> FixStream<T> for TestFixRemote
where T: FixAppMsgType 
{
    fn fix_message_start(&mut self, msg_type: FixMsgType<T>, is_replayable: bool)
    {
        self.sum = 0; // start accumulation of checksum
        
        // version first
        self.tag_value(8, &"FIX.4.2".to_string().into_bytes());
        
        // message length
        self.tag_value(9, &"".to_string().into_bytes()); // place holder
        let buf = unsafe {&mut *self.data.get()};
        self.lenOff = buf.len() - 1 /*SOH*/;
        self.len = 0; // start accumulation of length
       
        // message type
        let v = msg_type.get_value();
        self.tag_value(35, &v.to_string().into_bytes());
    }

    fn fix_message_done(&mut self, res: Result<(), FixStreamException>)
    {
        let buf = unsafe {&mut *self.data.get()};
        
        let len = format!("{:03}",  self.len).into_bytes();
        self.sum += len.iter()
            .fold((0u32, self.lenOff), |acc, &x| { buf.insert(acc.1, x as u8); (acc.0 + x as u32, acc.1 + 1) }).0;


        let chksum = format!("{:03}",  self.sum % 256u32);
        self.tag_value(10, &chksum.into_bytes()); 
    }
}

impl FixTagHandler for TestFixRemote {
    fn tag_value(&mut self, tag: u32, value: &[u8])
    {
        let buf = unsafe {&mut *self.data.get()};

        let (sum, len) = util::put_tag_id_eq(tag, buf);
        self.sum += sum;
        self.len += len;
        
        let (sum, len) = util::put_tag_val_soh(value, buf);
        self.sum += sum;
        self.len += len;
    }
}
