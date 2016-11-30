#![feature(box_patterns)]
#![feature(conservative_impl_trait)] 
#![feature(box_syntax)]

/// FIX protocol library.
/// This library goal is to simplify creation of FIX application.
/// It's flexible enough to accomodate all common case FIX usage.
///
/// Library employs end to end principle and trys to avoid creation of 
/// any intermediate objects when possible.
///
/// The main structure is:
///
/// FixApplication <>------ FixService <>------ FixConnection
///
/// FixApplication is a user defined fix application
/// FixService is either FixServer or FixClient the difference is who will 
/// initiate connection
///
/// FixConnection - main state machine responsible for verifying FIX session
/// states and transitions, as well as enforcing FIX rules such as sequencing and etc.
///
/// FixConnection <>------ FixTransport
///               <>------ FixEnvironment
/// 
/// FixTransport - is underlying connection to the other side of fix session.
/// (for example, in case of TCP it's either TCP client or server that connects to / listens on 
/// particular end point)
///
/// FixEnvironment - "factory" that abstracts creation of different things required for 
/// successful functioning of FIX machine (i.e. timers, and etc.)
///


pub mod fix;
pub mod connection;
pub mod util;
pub mod fix_tags;
mod test_util;

#[cfg(test)]
mod test {
	use fix::{FixMsgType, FixTimerFactory};
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
    use std::sync::Arc;
	use test_util::{TestFixMessage, TestFixEnvironment};
	use std::time::Duration;

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
	
	#[test]
	fn test_fix_environment() {
		let env = &mut TestFixEnvironment::new();
		let count: *mut _ = &mut 0;
        let trigger = || {
            *count += 1;
			println!("Timeout!");
		};
		let h = env.set_timeout(trigger, Duration::from_secs(10));
		env.run_for(Duration::from_secs(20));
        assert_eq!(2, unsafe { *count })
	}
}
