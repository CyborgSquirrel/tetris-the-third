use crate::config;
use std::time::Duration;
use crate::util::*;
use sdl2::event::Event;
use crate::unit::get_level_fall_duration;
use serde::{Serialize,Deserialize};

#[derive(Serialize, Deserialize)]
pub enum RotDirection {
	None,
	Left,
	Right,
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveState {
	Still,
	Instant,
	Prepeat,
	Repeat,
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum MoveDirection {
	None,
	Left,
	Right,
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
pub enum FallState {
	Fall,
	Softdrop,
	Harddrop,
}

#[derive(Serialize, Deserialize)]
pub struct MinoController {
	pub move_direction: MoveDirection,
	pub move_state: MoveState,
	pub rot_direction: RotDirection,
	pub fall_state: FallState,
	 
	pub store: bool,
	 
	pub fall_countdown: Duration,
	pub move_repeat_countdown: Duration,
	 
	pub fall_duration: Duration,
	 
	pub joystick_id: Option<u32>,
	pub config_id: usize,
}

impl MinoController {
	pub fn new(config_id: usize, joystick_id: Option<u32>) -> Self {
	    MinoController {
			move_direction: MoveDirection::None,
			move_state: MoveState::Still,
			rot_direction: RotDirection::None,
			fall_state: FallState::Fall,
			
			store: false,
			
			fall_countdown: Duration::from_secs(0),
			move_repeat_countdown: Duration::from_secs(0),
			
			fall_duration: get_level_fall_duration(1),
			
			joystick_id,
			config_id,
	    }
	}
	pub fn update(&mut self, keybinds: &mut [config::Player;4], event: &Event) {
		let MinoController {
			move_direction,
			move_state,
			rot_direction,
			fall_state,
			store,
			joystick_id,
			config_id,
			..
		} = self;
		
		let keybinds = &mut keybinds[*config_id];
		
		if is_key_down(event, keybinds.left) ||
		is_key_down(event, keybinds.left_alt) ||
		is_controlcode_down(event, &mut keybinds.controller_left, *joystick_id) {
			*move_direction = MoveDirection::Left;
			*move_state = MoveState::Instant;
		}
		
		if is_key_down(event, keybinds.right) ||
		is_key_down(event, keybinds.right_alt) ||
		is_controlcode_down(event, &mut keybinds.controller_right, *joystick_id) {
			*move_direction = MoveDirection::Right;
			*move_state = MoveState::Instant;
		}
		
		if is_key_up(event, keybinds.left) ||
		is_key_up(event, keybinds.left_alt) ||
		is_controlcode_up(event, &mut keybinds.controller_left, *joystick_id) {
			if *move_direction == MoveDirection::Left {
				*move_direction = MoveDirection::None;
				*move_state = MoveState::Still;
			}
		}
		
		if is_key_up(event, keybinds.right) ||
		is_key_up(event, keybinds.right_alt) ||
		is_controlcode_up(event, &mut keybinds.controller_right, *joystick_id) {
			if *move_direction == MoveDirection::Right {
				*move_direction = MoveDirection::None;
				*move_state = MoveState::Still;
			}
		}
		
		if is_key_down(event, keybinds.rot_left) ||
		is_controlcode_down(event, &mut keybinds.controller_rot_left, *joystick_id) {
			*rot_direction = RotDirection::Left
		}
		
		if is_key_down(event, keybinds.rot_right) ||
		is_key_down(event, keybinds.rot_right_alt) ||
		is_controlcode_down(event, &mut keybinds.controller_rot_right, *joystick_id) {
			*rot_direction = RotDirection::Right
		}
		
		if is_key_down(event, keybinds.softdrop) ||
		is_key_down(event, keybinds.softdrop_alt) ||
		is_controlcode_down(event, &mut keybinds.controller_softdrop, *joystick_id) {
			*fall_state = FallState::Softdrop;
		}
		
		if is_key_up(event, keybinds.softdrop) ||
		is_key_up(event, keybinds.softdrop_alt) ||
		is_controlcode_up(event, &mut keybinds.controller_softdrop, *joystick_id) {
			*fall_state = FallState::Fall
		}
		
		if is_key_down(event, keybinds.harddrop) ||
		is_controlcode_down(event, &mut keybinds.controller_harddrop, *joystick_id) {
			*fall_state = FallState::Harddrop;
		}
		
		if is_key_down(event, keybinds.store) ||
		is_controlcode_down(event, &mut keybinds.controller_store, *joystick_id) {
			*store = true;
		}
	}
}
