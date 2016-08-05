use fix::FixMsgType;
use fix::FixAppMsgType;
use fix::FixStream;
use fix::FixTagHandler;
use fix::FixStreamException;
use fix::FixParseIdLenSum;
use std::result::Result;
use std::fmt::format;
use fix_connection::NullFixMessage;

const ASCII_ZERO: i32 = ('0' as i32);
const SOH: u8 = '\x01' as u8;
const EQ: u8 = '=' as u8;

struct Slicer<'a> {
    buf: &'a[u8],
    len: usize
}

impl<'a> Slicer<'a> {
    pub fn buf(&mut self) -> &'a[u8] {
        &self.buf[self.len..]
    }

    pub fn consume(&mut self, len: usize) {
        self.len += len;
    }
}

pub fn parse_fix_message<M, T>(buf: &[u8], fmh: &mut T) -> Result<Option<(usize)>, FixStreamException>
where M: FixAppMsgType, T: FixStream<M>
{
    let mut chksum = 0;
    let mut s = Slicer {buf: buf, len: 0};
    let required_tags = [8, 9, 35];
    for eid in &required_tags[..] {
        //println!("total: {:?} {:?}", total_len, String::from_utf8_lossy(&buf[total_len..]));
        let res = get_tag(&mut s);
        match res {
            Ok(Some((id, v, sum))) if id == 35u32 => {
                chksum += sum;
                fmh.fix_message_start(FixMsgType::lookup(v), true);
                break;
            },
            Ok(Some((id, _, sum))) if id == *eid as u32 => {
                chksum += sum;
            },
            Ok(None) => return Ok(None),
            Ok(Some(_)) => return Err(format!("Missplaced tag: {}", *eid)),
            Err(err) => return Err(err),
        };
    }

    loop {
        match get_tag(&mut s) {
            Ok(Some((id, v, sum))) if id != 10u32 => {
                chksum += sum;
                fmh.tag_value(id, v);
            },
            Ok(Some((_, v, _))) => {
                let sum = v.iter().fold(0, |acc: u32, &x| acc * 10 + (x as u32 -'0' as u32));
                if sum == (chksum % 256) { 
                    fmh.fix_message_done(Ok(()));
                    return Ok(Some(s.len));
                }
                let err_str = format!("Malformed message: calc sum {} != {}", chksum%256, sum);
                fmh.fix_message_done(Err(err_str));
                let err_str = format!("Malformed message: calc sum {} != {}", chksum%256, sum);
                return Err(err_str);
            },
            Err(err) => return Err(err),
            Ok(None) => return Ok(None),
        }
    }
}

fn get_tag<'a> (s: &mut Slicer<'a>) -> Result<Option<(u32, &'a[u8], u32)>, FixStreamException>
{ 
    let mut chksum = 0;
    match try!(get_tag_id(s.buf())) {
        Some((id, len, sum)) => {
            s.consume(len);
            chksum += sum;
            match get_tag_value(s.buf()) {
                Some((v, sum)) => {
                    s.consume(v.len() + 1/*SOH*/);
                    chksum += sum;
                    return Ok(Some((id, v, chksum)))
                },
                None => return Ok(None),
            }
        },
        _ => return Ok(None)
    }

    Ok(None)
}


pub fn get_tag_value<'a>(buf: &'a [u8]) -> Option<(&'a [u8], u32)> {
    let mut value_end_pos = 0;
    let mut sum: u32 = 0;
    for v in buf {
        sum += *v as u32;
        if *v == 0x01u8 {
            return Some((&buf[0..value_end_pos], sum));
        }
        value_end_pos += 1;
    }
    None
}

pub fn get_tag_id(buf: &[u8]) -> Result<Option<FixParseIdLenSum>, FixStreamException> {
    
    let mut tag_id = 0;
    let mut count = 0;
    let mut sum: u32 = 0;

    for v in buf {
        
        count += 1;
        sum += *v as u32;

        if *v == EQ as u8 {
            return Ok(Some((tag_id, count, sum)))
        }

        tag_id = tag_id * 10;

        let value = *v as i32 - ASCII_ZERO;
        if value < 0 { 
            return Err(String::from("Negative tag"));
        }
        tag_id += value as u32;
        //println!("iter: {:?}", value);
    }

    return Ok(None);
}

pub fn put_tag_id_eq(tag_id: u32, to: &mut Vec<u8>) -> (u32, u32)
{
    // TODO: Think how to speed this up
    let mut len = 0u32;
    let mut sum = 0u32;
    let pos = to.len();
    let mut tag_id = tag_id;
    while tag_id > 0 {
        let v = (tag_id % 10) as u8 + ' ' as u8;
        to.insert(pos, v);
        sum += v as u32;
        len += 1;
        tag_id = tag_id / 10;
    }
    to.push(EQ);
    sum += EQ as u32;
    len += 1;
    (sum, len)
}

pub fn put_tag_val_soh(val: &[u8], to: &mut Vec<u8>) -> (u32, u32)
{
    let acc = val.into_iter().fold((0u32, 0u32), |mut acc, &v| {acc.0 += v as u32; acc.1 += 1u32; acc}); 
    to.extend_from_slice(val);
    to.push(SOH);
    (acc.0 + SOH as u32, acc.1 + 1u32)
}

pub struct FixMessageWriter {
    sum: u32,
    buf: Vec<u8>,
}
impl<T> FixStream<T> for FixMessageWriter
where T: FixAppMsgType
{
    fn fix_message_start(&mut self, msg_type: FixMsgType<T>, is_replayable: bool)
    {
    }

    fn fix_message_done(&mut self, res: Result<(), FixStreamException>)
    {
    }
}

impl FixTagHandler for FixMessageWriter {
    fn tag_value(&mut self, tag: u32, value: &[u8]) {
        
    }
}


