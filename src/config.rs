use toml::Value;
use sdl2::keyboard::Keycode;
use sdl2::controller::Button;
use sdl2::controller::Axis;
use std::fs::File;
use std::io::prelude::*;

#[derive(Debug, Clone, Copy)]
pub enum Controlcode {
	Button(Button),
	Axis(Axis,bool),
}

impl Controlcode {
	fn from_name(name: &str) -> Option<Controlcode> {
		if let Some(button) = Button::from_string(name) {
			Some(Controlcode::Button(button))
		}else if let Some(axis) = Axis::from_string(name) {
			Some(Controlcode::Axis(axis,false))
		}else{
			None
		}
	}
}

#[derive(Default)]
pub struct Player {
	pub left: Option<Keycode>,
	pub left_alt: Option<Keycode>,
	pub right: Option<Keycode>,
	pub right_alt: Option<Keycode>,
	
	pub rot_left: Option<Keycode>,
	pub rot_right: Option<Keycode>,
	pub rot_right_alt: Option<Keycode>,
	
	pub softdrop: Option<Keycode>,
	pub softdrop_alt: Option<Keycode>,
	pub harddrop: Option<Keycode>,
	
	pub store: Option<Keycode>,
	
	pub controller_left: Option<Controlcode>,
	pub controller_right: Option<Controlcode>,
	
	pub controller_rot_left: Option<Controlcode>,
	pub controller_rot_right: Option<Controlcode>,
	
	pub controller_softdrop: Option<Controlcode>,
	pub controller_harddrop: Option<Controlcode>,
	
	pub controller_store: Option<Controlcode>,
}

impl Player {
	fn from_toml(toml: &Value) -> Self {
		fn get_as_str<'a>(toml: &'a Value, key: &str) -> Option<&'a str>{
			toml.get(key).and_then(toml::Value::as_str)
		}
		
		let controls = &toml["controls"];
		
		let keyboard = &controls.get("keyboard");
		let get_as_keycode = |key|keyboard.and_then(|v|get_as_str(v, key)).and_then(Keycode::from_name);
		
		let left = get_as_keycode("left");
		let left_alt = get_as_keycode("left_alt");
		let right = get_as_keycode("right");
		let right_alt = get_as_keycode("right_alt");
		
		let rot_left = get_as_keycode("rot_left");
		let rot_right = get_as_keycode("rot_right");
		let rot_right_alt = get_as_keycode("rot_right_alt");
		
		let softdrop = get_as_keycode("softdrop");
		let softdrop_alt = get_as_keycode("softdrop_alt");
		let harddrop = get_as_keycode("harddrop");
		
		let store = get_as_keycode("store");
		
		
		let controller = &controls.get("controller");
		let get_as_controlcode = |key|controller.and_then(|v|get_as_str(v, key)).and_then(Controlcode::from_name);
		
		let controller_left = get_as_controlcode("left");
		let controller_right = get_as_controlcode("right");

		let controller_rot_left = get_as_controlcode("rot_left");
		let controller_rot_right = get_as_controlcode("rot_right");

		let controller_softdrop = get_as_controlcode("softdrop");
		let controller_harddrop = get_as_controlcode("harddrop");
		
		let controller_store = get_as_controlcode("store");
		
		Player {
			left,
			left_alt,
			right,
			right_alt,
			
			rot_left,
			rot_right,
			rot_right_alt,
			
			softdrop,
			softdrop_alt,
			harddrop,
			
			store,
	
			controller_left,
			controller_right,
			
			controller_rot_left,
			controller_rot_right,
			
			controller_softdrop,
			controller_harddrop,
			
			controller_store,
		}
	}
}

pub struct Config {
	pub players: [Player;4],
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
		let value = string.parse::<Value>().unwrap();
		
		let players = &value["players"].as_array().unwrap();
		let player_from_toml = |index|players.get(index).map(|v|Player::from_toml(v)).unwrap_or_default();
		
		let players = [
			player_from_toml(0),
			player_from_toml(1),
			player_from_toml(2),
			player_from_toml(3),
		];
		
		Config {
			players,
		}
	}
}