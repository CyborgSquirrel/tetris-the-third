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
		let keyboard = &toml["keyboard"];
		let left = Keycode::from_name(keyboard["left"].as_str().unwrap());
		let left_alt = Keycode::from_name(keyboard["left_alt"].as_str().unwrap());
		let right = Keycode::from_name(keyboard["right"].as_str().unwrap());
		let right_alt = Keycode::from_name(keyboard["right_alt"].as_str().unwrap());
		
		let rot_left = Keycode::from_name(keyboard["rot_left"].as_str().unwrap());
		let rot_right = Keycode::from_name(keyboard["rot_right"].as_str().unwrap());
		let rot_right_alt = Keycode::from_name(keyboard["rot_right_alt"].as_str().unwrap());
		
		let softdrop = Keycode::from_name(keyboard["softdrop"].as_str().unwrap());
		let softdrop_alt = Keycode::from_name(keyboard["softdrop_alt"].as_str().unwrap());
		let harddrop = Keycode::from_name(keyboard["harddrop"].as_str().unwrap());
		
		let store = Keycode::from_name(keyboard["store"].as_str().unwrap());
		
		
		let controller = &toml["controller"];
		let controller_left = Controlcode::from_name(controller["left"].as_str().unwrap());
		let controller_right = Controlcode::from_name(controller["right"].as_str().unwrap());

		let controller_rot_left = Controlcode::from_name(controller["rot_left"].as_str().unwrap());
		let controller_rot_right = Controlcode::from_name(controller["rot_right"].as_str().unwrap());

		let controller_softdrop = Controlcode::from_name(controller["softdrop"].as_str().unwrap());
		let controller_harddrop = Controlcode::from_name(controller["harddrop"].as_str().unwrap());
		
		let controller_store = Controlcode::from_name(controller["store"].as_str().unwrap());
		
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
	pub players: [Player;2],
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
		
		let players = [
			Player::from_toml(&players[0]["controls"]),
			Player::from_toml(&players[1]["controls"]),
		];
		
		Config {
			players,
		}
	}
}