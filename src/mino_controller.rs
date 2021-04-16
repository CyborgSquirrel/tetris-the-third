use crate::config;
use std::time::Duration;
use sdl2::event::Event;
use crate::unit::get_level_fall_duration;
use serde::{Serialize,Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub enum RotDirection {
	None,
	Left,
	Right,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveState {
	Still,
	Instant,
	Prepeat,
	Repeat,
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum MoveDirection {
	None,
	Left,
	Right,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FallState {
	Fall,
	Softdrop,
	Harddrop,
}

#[derive(Clone, Serialize, Deserialize)]
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
	pub fn update(&mut self, binds: &[config::PlayerBinds;4], event: &Event) {
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
		
		let b = &binds[*config_id];
		let im = config::InputMethod::new(true, *joystick_id);
		
		if b.left.is_down(event, &im) ||
		b.left_alt.is_down(event, &im) {
			*move_direction = MoveDirection::Left;
			*move_state = MoveState::Instant;
		}
		
		if b.right.is_down(event, &im) ||
		b.right_alt.is_down(event, &im) {
			*move_direction = MoveDirection::Right;
			*move_state = MoveState::Instant;
		}
		
		if b.left.is_up(event, &im) ||
		b.left_alt.is_up(event, &im) {
			if *move_direction == MoveDirection::Left {
				*move_direction = MoveDirection::None;
				*move_state = MoveState::Still;
			}
		}
		
		if b.right.is_up(event, &im) ||
		b.right_alt.is_up(event, &im) {
			if *move_direction == MoveDirection::Right {
				*move_direction = MoveDirection::None;
				*move_state = MoveState::Still;
			}
		}
		
		if b.rot_left.is_down(event, &im) {
			*rot_direction = RotDirection::Left
		}
		
		if b.rot_right.is_down(event, &im) ||
		b.rot_right_alt.is_down(event, &im) {
			*rot_direction = RotDirection::Right
		}
		
		if b.softdrop.is_down(event, &im) ||
		b.softdrop_alt.is_down(event, &im) {
			*fall_state = FallState::Softdrop;
		}
		
		if b.softdrop.is_up(event, &im) ||
		b.softdrop_alt.is_up(event, &im) {
			*fall_state = FallState::Fall
		}
		
		if b.harddrop.is_down(event, &im) {
			*fall_state = FallState::Harddrop;
		}
		
		if b.store.is_down(event, &im) {
			*store = true;
		}
	}
}
