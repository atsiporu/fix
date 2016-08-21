mod fix;
mod connection;
mod util;
mod test_util;

#[cfg(test)]
mod test {
	use fix::FixMsgType;
	use fix::FixAppMsgType;
	use fix::FixStream;
	use fix::FixInChannel;
	use fix::FixTagHandler;
	use fix::ParseControl;
	use std::fmt::Debug;
	use super::util;
	use super::test_util;
	use std::cell::RefCell;
	use std::cell::RefMut;
	use std::rc::Rc;
    use test_util::TestFixMessage;

    /*
    impl FixAppMsgType for () {
        fn lookup(btype: &[u8]) -> Option<Self>
        {
            None
        }
    }
	*/

	pub struct TestScope<T> {
		starts: Vec<T>,
		ends: Vec<T>
	}
	impl<T> TestScope<T> {
		fn new() -> TestScope<T> {
			TestScope { starts: Vec::new(), ends: Vec::new() }
		}
		fn pop_started_scope(&mut self) -> Option<T> {
			self.starts.pop()
		}
		fn pop_ended_scope(&mut self) -> Option<T> {
			self.ends.pop()
		}
	}
	
	pub struct TestScope2;

	impl FixStream for TestScope2
    {
        type MSG_TYPES = ();
		fn fix_message_start(&mut self, msg_type: FixMsgType<()>, is_replayable: bool)
        {
			println!("Message start {:?}", msg_type);
			//self.tail(scope);
		}
		fn fix_message_done(&mut self, res: Result<(), String>) {
			println!("Message done {:?}", res);
		}
	}
	impl FixTagHandler for TestScope2 {
		fn tag_value(&mut self, t: u32, v: &[u8]) {
			println!("{:?} = {:?}", t, String::from_utf8_lossy(v));
		}
	}

	#[test]
	fn it_scope() {
		use std::io::prelude;
		let mut s = TestScope2;
		s.fix_message_start(FixMsgType::Logon, false);
		s.tag_value(1, &[3u8, 4u8]);
		s.tag_value(1, &[3u8]);
		s.fix_message_done(Ok(()));
	}

	#[test]
	fn test_fix_util() {
		let arr  = b"121=1234\x01";
		let (tag_id, cnt, _) = util::get_tag_id(arr).unwrap().unwrap();
		assert_eq!(121, tag_id);
		assert_eq!(4/*includes =*/, cnt);
		let val = util::get_tag_value(&arr[cnt as usize..]).unwrap();
		let val_i = val.0.iter().fold(0, |acc: i32, &x| acc * 10 + (x as i32 -'0' as i32));
		assert_eq!(1234i32, val_i);
	}

	#[test]
	fn test_fix_message() {
		println!("test_fix_message");
		let arr = b"8=FIX.4.2\x019=1234\x0135=A\x0158=Hello\x0110=230\x01\
8=FIX.4.2\x019=1234\x0135=A\x0158=Hello\x0110=230\x01\
8=FIX.4.2\x019=1234\x0135=A\x0158=Hello\x0110=230\x01\
8=FIX.4.2\x019=1234\x0135=A\x0158=Hello\x0110=230";
		let mut s = TestScope2;
		let mut len = 0;
		for _ in 0..3 {
			let res = util::parse_fix_message(&arr[len..], &mut s);
			println!("Fix message result: {:?} buf len {:?}", res, &arr[len..].len());
			len += res.unwrap().unwrap();
		}
		let res = util::parse_fix_message(&arr[len..], &mut s);
		println!("Fix message result: {:?} buf len {:?}", res, &arr[len..].len());
	}

	#[test]
	fn test_fix_logon() {

        let mut fmh = TestFixMessage::new();
		let r = RefCell::new(test_util::TestFixRemote::new());
		let mut fc = test_util::TestFixTransport::new(&r);
		
        let my_fix_type = "TT".to_string();
		{
			let mut rs = r.borrow_mut();
			rs.fix_message_start(FixMsgType::Logon, false);
			rs.tag_value(58, "Hello".to_string().as_bytes());
			rs.fix_message_done(Ok(()));
			
            rs.fix_message_start(FixMsgType::Unknown(my_fix_type.as_bytes()), false);
			rs.tag_value(58, "Hello".to_string().as_bytes());
			rs.fix_message_done(Ok(()));
		}

        //let fc = &mut fc as &mut FixInChannel;
        
        let res = fc.read_fix_message(&mut fmh);
        // session level message not communicated
        assert_eq!(None, fmh.msg_type);

        let res = fc.read_fix_message(&mut fmh);
        // application level message communicated
        assert_eq!(Some("TT".to_string()), fmh.msg_type);
        assert_eq!(&"Hello".to_string(), fmh.tag_values.get(&58).unwrap());
	}
}
