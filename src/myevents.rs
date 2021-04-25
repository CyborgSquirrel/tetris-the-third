#![allow(dead_code)]
use std::{collections::HashMap, ffi::c_void, sync::Mutex};

use sdl2::{EventSubsystem, controller::{Axis, Button}, event::Event};
use lazy_static::lazy_static;

#[derive(Debug,Clone,Copy)]
pub struct MyControllerButtonDown {
	pub timestamp: u32,
	pub which: usize,
	pub button: Button,
}

#[derive(Debug,Clone,Copy)]
pub struct MyControllerButtonUp {
	pub timestamp: u32,
	pub which: usize,
	pub button: Button,
}

#[derive(Debug,Clone,Copy)]
pub struct MyControllerAxisDown {
	pub timestamp: u32,
	pub which: usize,
	pub axis: Axis,
}

#[derive(Debug,Clone,Copy)]
pub struct MyControllerAxisUp {
	pub timestamp: u32,
	pub which: usize,
	pub axis: Axis,
}

// Here I handle my custom events. This code is actually a modified version of
// the code from the sdl2 crate. I had to modify it, because their
// implementation caused memory leaks/heap corruption if you never casted an
// event/casted an event more than once.

// This implementation is bad. You have to always make sure to call
// drop_if_user_event after you're done with the event, it certainly breaks
// some rust rules, etc.. Despite that, I've spent hours trying to make a
// better version, and I came up with nothing. So, I'll just leave it like it
// is.

struct CustomEventTypeMaps {
   sdl_id_to_type_id: HashMap<u32, ::std::any::TypeId>,
   type_id_to_sdl_id: HashMap<::std::any::TypeId, u32>,
}

impl CustomEventTypeMaps {
	fn new() -> Self {
		CustomEventTypeMaps {
			sdl_id_to_type_id: HashMap::new(),
			type_id_to_sdl_id: HashMap::new(),
		}
	}
}

lazy_static! {
	static ref CUSTOM_EVENT_TYPES: Mutex<CustomEventTypeMaps> =
		Mutex::new(CustomEventTypeMaps::new());
}

pub fn register_custom_event<T: ::std::any::Any+Copy>(event_subsystem: &EventSubsystem) -> Result<(), String> {
	use std::any::TypeId;
	let event_id = *(unsafe { event_subsystem.register_events(1) })?.first().unwrap();
	let mut cet = CUSTOM_EVENT_TYPES.lock().unwrap();
	let type_id = TypeId::of::<Box<T>>();

	if cet.type_id_to_sdl_id.contains_key(&type_id) {
		return Err("The same event type can not be registered twice!".to_owned());
	}

	cet.sdl_id_to_type_id.insert(event_id, type_id);
	cet.type_id_to_sdl_id.insert(type_id, event_id);

	Ok(())
}

pub fn drop_if_user_event<T: ::std::any::Any+Copy>(event: &Event) {
	use std::any::TypeId;
	let type_id = TypeId::of::<Box<T>>();

	let (event_id, event_box_ptr) = match *event {
		sdl2::event::Event::User { type_, data1, .. } => (type_, data1),
		_ => return,
	};

	let cet = CUSTOM_EVENT_TYPES.lock().unwrap();
	
	let event_type_id = match cet.sdl_id_to_type_id.get(&event_id) {
		Some(id) => id,
		None => {
			panic!("internal error; could not find typeid")
		}
	};

	if &type_id != event_type_id {
		return;
	}
	
	unsafe {
		let _ = Box::from_raw(event_box_ptr as *mut T);
	};
}

pub fn as_user_event_type<T: ::std::any::Any+Copy>(event: &Event) -> Option<T> {
	use std::any::TypeId;
	let type_id = TypeId::of::<Box<T>>();

	let (event_id, event_box_ptr) = match *event {
		sdl2::event::Event::User { type_, data1, .. } => (type_, data1),
		_ => return None,
	};

	let cet = CUSTOM_EVENT_TYPES.lock().unwrap();

	let event_type_id = match cet.sdl_id_to_type_id.get(&event_id) {
		Some(id) => id,
		None => {
			panic!("internal error; could not find typeid")
		}
	};

	if &type_id != event_type_id {
		return None;
	}
	
	let event = unsafe {
		let event_box = Box::from_raw(event_box_ptr as *mut T);
		let event = *event_box.clone();
		Box::leak(event_box);
		event
	};

	Some(event)
}

pub fn push_custom_event<T: ::std::any::Any+Copy>(event_subsystem: &EventSubsystem, event: T) -> Result<(), String> {
	use std::any::TypeId;
	let cet = CUSTOM_EVENT_TYPES.lock().unwrap();
	let type_id = TypeId::of::<Box<T>>();

	let user_event_id = *match cet.type_id_to_sdl_id.get(&type_id) {
		Some(id) => id,
		None => {
			return Err("Type is not registered as a custom event type!".to_owned());
		}
	};

	let event_box = Box::new(event);
	let event = Event::User {
		timestamp: 0,
		window_id: 0,
		type_: user_event_id,
		code: 0,
		data1: Box::into_raw(event_box) as *mut c_void,
		data2: ::std::ptr::null_mut() as *mut c_void,
	};

	event_subsystem.push_event(event)?;

	Ok(())
}