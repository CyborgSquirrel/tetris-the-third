use toml::Value;
use sdl2::{event::Event, keyboard::Keycode};
use std::fs::File;
use std::io::prelude::*;
use std::time::Duration;
use serde::{Serialize,Deserialize};

use crate::myevents;

#[derive(Default)]
pub struct Player {
	pub move_prepeat_duration: Duration,
	pub move_repeat_duration: Duration,
}

impl Player {
	fn from_toml(toml: &Value) -> Self {
		let move_prepeat_duration = toml.get("move_prepeat_duration")
			.and_then(Value::as_float)
			.map(Duration::from_secs_f64)
			.unwrap_or(Duration::from_secs_f64(0.15));

		let move_repeat_duration = toml.get("move_repeat_duration")
			.and_then(Value::as_float)
			.map(Duration::from_secs_f64)
			.unwrap_or(Duration::from_secs_f64(0.05));
		
		Player {
			move_prepeat_duration,
			move_repeat_duration,
		}
	}
}

#[derive(Debug,Clone,Copy)]
pub enum Conbind {Button(sdl2::controller::Button), Axis(sdl2::controller::Axis)}
impl Conbind {
	fn from_name(name: &str) -> Option<Self> {
		if let Some(button) = sdl2::controller::Button::from_string(name) {Some(Conbind::Button(button))}
		else if let Some(axis) = sdl2::controller::Axis::from_string(name) {Some(Conbind::Axis(axis))}
		else {None}
	}
}
impl From<sdl2::controller::Button> for Conbind {
	fn from(button: sdl2::controller::Button) -> Self {Conbind::Button(button)}
}
impl From<sdl2::controller::Axis> for Conbind {
	fn from(axis: sdl2::controller::Axis) -> Self {Conbind::Axis(axis)}
}

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct InputMethod {
	pub keyboard: Option<()>,
	pub controller: Option<usize>,
}
impl InputMethod {
	pub fn new(keyboard: bool, controller: Option<usize>) -> Self {
		let keyboard = if keyboard {Some(())} else {None};
		Self {keyboard, controller}
	}
}

#[derive(Debug, Default)]
pub struct Bind {
	key: Option<Keycode>,
	con: Option<Conbind>,
}
impl Bind {
	// TODO: improve this by making it templated and turning Button/Axis straight
	// into Conbind
	pub fn new(key: Keycode, con: Conbind) -> Self {
		let key = Some(key);
		let con = Some(con);
		Bind {key, con}
	}
	fn from_name(key_name: Option<&str>, button_name: Option<&str>) -> Self {
		Bind {
			key: key_name.and_then(Keycode::from_name),
			con: button_name.and_then(Conbind::from_name),
		}
	}
	
	// Sorry...
	pub fn is_down(&self, event: &Event, input_method: &InputMethod) -> bool {
		(if let (Event::KeyDown{keycode:Some(key),repeat:false,..},Some(_)) =
			(event,input_method.keyboard)
			{self.key.map_or(false, |a|a==*key)} else {false}) ||
		(if let(Some(myevents::MyControllerButtonDown{button,which,..}),Some(id),Some(Conbind::Button(my_button))) =
			(myevents::as_user_event_type(event),input_method.controller,self.con)
			{my_button==button&&id==which} else {false}) ||
		(if let(Some(myevents::MyControllerAxisDown{axis,which,..}),Some(id),Some(Conbind::Axis(my_axis))) =
			(myevents::as_user_event_type(event),input_method.controller,self.con)
			{my_axis==axis&&id==which} else {false})
	}
	pub fn is_up(&self, event: &Event, input_method: &InputMethod) -> bool {
		(if let (Event::KeyUp{keycode:Some(key),repeat:false,..},Some(_)) =
			(event,input_method.keyboard)
			{self.key.map_or(false, |a|a==*key)} else {false}) ||
		(if let(Some(myevents::MyControllerButtonUp{button,which,..}),Some(id),Some(Conbind::Button(my_button))) =
			(myevents::as_user_event_type(event),input_method.controller,self.con)
			{my_button==button&&id==which} else {false}) ||
		(if let(Some(myevents::MyControllerAxisUp{axis,which,..}),Some(id),Some(Conbind::Axis(my_axis))) =
			(myevents::as_user_event_type(event),input_method.controller,self.con)
			{my_axis==axis&&id==which} else {false})
	}
}

pub struct MenuBinds {
	pub up: Bind, pub down: Bind,
	pub left: Bind, pub right: Bind,
	pub ok: Bind, pub add_player: Bind,
	pub pause: Bind, pub restart: Bind,
}

#[derive(Debug, Default)]
pub struct PlayerBinds {
	pub left: Bind, pub left_alt: Bind,
	pub right: Bind, pub right_alt: Bind,
	
	pub rot_left: Bind,
	pub rot_right: Bind, pub rot_right_alt: Bind,
	
	pub softdrop: Bind, pub softdrop_alt: Bind,
	pub harddrop: Bind,
	
	pub store: Bind,
}
impl PlayerBinds {
	fn from_toml(toml: &Value) -> Self {
		fn get_as_str<'a>(toml: &'a Value, key: &str) -> Option<&'a str>{
			toml.get(key).and_then(toml::Value::as_str)
		}
		
		let controls = &toml["controls"];
		
		let keyboard = &controls.get("keyboard");
		let controller = &controls.get("controller");
		
		let bind_from_name =
		|bind_name|Bind::from_name(
			keyboard.and_then(|a|get_as_str(a, bind_name)),
			controller.and_then(|a|get_as_str(a, bind_name)));
		
		let left = bind_from_name("left");
		let left_alt = bind_from_name("left_alt");
		let right = bind_from_name("right");
		let right_alt = bind_from_name("right_alt");
		
		let rot_left = bind_from_name("rot_left");
		let rot_right = bind_from_name("rot_right");
		let rot_right_alt = bind_from_name("rot_right_alt");
		
		let softdrop = bind_from_name("softdrop");
		let softdrop_alt = bind_from_name("softdrop_alt");
		let harddrop = bind_from_name("harddrop");
		
		let store = bind_from_name("store");
		
		PlayerBinds {
			left, left_alt,
			right, right_alt,
			
			rot_left,
			rot_right, rot_right_alt,
			
			softdrop, softdrop_alt,
			harddrop,
			
			store,
		}
	}
}

pub struct Config {
	pub width: Option<u32>,
	pub height: Option<u32>,
	pub borderless: bool,
	pub block_size: u32,
	pub players: Vec<Player>,
	pub binds: Vec<PlayerBinds>,
}

impl Config {
	pub fn from_file() -> Config {
		let mut file = File::open("config.toml")
			.expect("Couldn't open config file");
		let mut contents = String::new();
		file.read_to_string(&mut contents)
			.expect("Couldn't read from config file");
		Config::from_string(contents)
	}
	fn from_string(string: String) -> Config {
		let toml = string.parse::<Value>().unwrap();
		
		let players = &toml["players"].as_array().unwrap();
		let binds_from_toml = |index|players.get(index).map(|v|PlayerBinds::from_toml(v)).unwrap_or_default();
		let player_from_toml = |index|players.get(index).map(|v|Player::from_toml(v)).unwrap_or_default();
		
		let binds: Vec<_> = (0..).map(|i|binds_from_toml(i)).take(crate::MAX_PLAYERS).collect();
		let players: Vec<_> = (0..).map(|i|player_from_toml(i)).take(crate::MAX_PLAYERS).collect();
		
		let width = toml.get("width").and_then(Value::as_integer).map(|a|a as u32);
		let height = toml.get("height").and_then(Value::as_integer).map(|a|a as u32);
		
		let borderless = toml.get("borderless").and_then(Value::as_bool).unwrap_or(false);
		let block_size = toml.get("block_size").and_then(Value::as_integer).unwrap_or(30) as u32;
		
		Config {
			width,
			height,
			borderless,
			block_size,
			players,
			binds,
		}
	}
}