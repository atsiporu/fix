#![feature(unboxed_closure)]
#![feature(box_patterns)]
#![feature(conservative_impl_trait)] 
#![feature(box_syntax)]
#![feature(loop_break_value)]

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

extern crate futures;

pub mod fix;
pub mod connection;
pub mod util;
pub mod fix_tags;
mod test_util;

#[cfg(test)]
mod test {
	use fix::*;
	use std::fmt::Debug;
	use super::util;
	use super::test_util;
	use std::cell::RefCell;
	use std::cell::RefMut;
	use std::rc::Rc;
    use std::sync::{Arc};
    use std::sync::atomic::{AtomicUsize, Ordering};
	use test_util::{TestFixMessage, TestFixEnvironment};
	use std::time::Duration;
    use connection::{FixConnection, ConnectionType};

	
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
	fn test_message_scope() {
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
	fn test_fix_logon_acceptor() {

		let r = RefCell::new(test_util::TestFixRemote::new());
        let (mut tft, env, mut fix_app) = test_util::fix_parts(&r);

        let mut fc = FixConnection::new(String::from("FIX.4.2"), tft, env, ConnectionType::Acceptor);

        fc.connect(&mut fix_app);

        // send logon followed by unknown message
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
		
		let res = fc.read_fix_message(&mut fix_app);
		// session level message not communicated
		assert_eq!(None, fix_app.message.msg_type);
		assert_eq!(Some(SessionRequest::In(SessionRequestType::Logon(None))), fix_app.requests.pop());

		let res = fc.read_fix_message(&mut fix_app);
		// application level message communicated
		assert_eq!(Some("TT".to_string()), fix_app.message.msg_type);
		assert_eq!(&"Hello".to_string(), fix_app.message.tag_values.get(&58).unwrap());
	}
	
    #[test]
	fn test_fix_logon_initiator() {

		let r = RefCell::new(test_util::TestFixRemote::new());
        let (mut tft, env, mut fix_app) = test_util::fix_parts(&r);

        let mut fc = FixConnection::new(String::from("FIX.4.2"), tft, env, ConnectionType::Initiator);

        fc.connect(&mut fix_app);

        // send logon followed by unknown message
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
		
		let res = fc.read_fix_message(&mut fix_app);
		// session level message not communicated
		assert_eq!(None, fix_app.message.msg_type);
		assert_eq!(Some(SessionRequest::In(SessionRequestType::Logon(None))), fix_app.requests.pop());

		let res = fc.read_fix_message(&mut fix_app);
		// application level message communicated
		assert_eq!(Some("TT".to_string()), fix_app.message.msg_type);
		assert_eq!(&"Hello".to_string(), fix_app.message.tag_values.get(&58).unwrap());
	}

	#[test]
	fn test_fix_environment() {
		let env = &mut TestFixEnvironment::new();
		let count = AtomicUsize::new(0);
        let trigger = || {
            count.fetch_add(1, Ordering::Relaxed);
			println!("Timeout!");
		};
		let h = env.set_timeout(trigger, Duration::from_secs(10));
		env.run_for(Duration::from_secs(20));
        assert_eq!(2, count.load(Ordering::Relaxed))
	}
}
