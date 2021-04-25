use crate::{command::Command, config::{self, InputMethod}};
use std::{collections::VecDeque, time::Duration};
use sdl2::event::Event;
use crate::unit::{get_level_fall_duration,UnitCommandKind};
use serde::{Serialize,Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub enum RotDirection {None, Left, Right}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveState {Still, Instant, Prepeat, Repeat}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum MoveDirection {None, Left, Right}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FallState {Fall, Softdrop, Harddrop}

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
	 
	pub config_id: usize,
}

impl MinoController {
	pub fn new(config_id: usize) -> Self {
	   MinoController {
			move_direction: MoveDirection::None,
			move_state: MoveState::Still,
			rot_direction: RotDirection::None,
			fall_state: FallState::Fall,
			
			store: false,
			
			fall_countdown: Duration::from_secs(0),
			move_repeat_countdown: Duration::from_secs(0),
			
			fall_duration: get_level_fall_duration(1),
			
			config_id,
	   }
	}
	pub fn update(&mut self, binds: &Vec<config::PlayerBinds>, input_method: &InputMethod, event: &Event) {
		let MinoController {
			move_direction,
			move_state,
			rot_direction,
			fall_state,
			store,
			config_id,
			..
		} = self;
		
		let b = &binds[*config_id];
		let im = input_method;
		
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
	pub fn append_commands(&mut self, unit_id: usize, queue: &mut VecDeque<crate::unit::UnitCommand>, config: &Vec<config::Player>, dpf: Duration) {
		let MinoController {
			move_direction,
			move_state,
			rot_direction,
			fall_state,
			store,
			config_id,
			move_repeat_countdown,
			fall_countdown,
			fall_duration,
			..
		} = self;
		
		let mut append = |command|queue.push_back((unit_id,command).wrap());
		let move_repeat_duration = &config[*config_id].move_repeat_duration;
		let move_prepeat_duration = &config[*config_id].move_prepeat_duration;
		
		// MOVEMENT
		
		if MoveState::Instant == *move_state {
			match move_direction{
				MoveDirection::Left => append(UnitCommandKind::MoveLeft),
				MoveDirection::Right => append(UnitCommandKind::MoveRight),
				_ => panic!(),
			};
			*move_repeat_countdown = Duration::from_secs(0);
			*move_state = MoveState::Prepeat;
		}
		if MoveState::Prepeat == *move_state {
			if *move_repeat_countdown >= *move_prepeat_duration {
				*move_repeat_countdown -= *move_prepeat_duration;
				match move_direction{
					MoveDirection::Left => append(UnitCommandKind::MoveLeft),
					MoveDirection::Right => append(UnitCommandKind::MoveRight),
					_ => panic!(),
				};
				*move_state = MoveState::Repeat;
			}
		}
		if MoveState::Repeat == *move_state {
			while *move_repeat_countdown >= *move_repeat_duration {
				*move_repeat_countdown -= *move_repeat_duration;
				match move_direction{
					MoveDirection::Left => append(UnitCommandKind::MoveLeft),
					MoveDirection::Right => append(UnitCommandKind::MoveRight),
					_ => panic!(),
				};
			}
		}
		if MoveState::Still != *move_state {
			*move_repeat_countdown += dpf;
		}
		
		// ROTATION
		
		match rot_direction {
			RotDirection::Left => append(UnitCommandKind::RotateLeft),
			RotDirection::Right => append(UnitCommandKind::RotateRight),
			_ => (),
		};
		*rot_direction = RotDirection::None;
		
		// GRAVITY
		
		match fall_state {
			FallState::Fall => *fall_countdown += dpf,
			FallState::Softdrop => *fall_countdown += 3*dpf,
			FallState::Harddrop => (),
		}
		
		let mut g = 0;
		if FallState::Harddrop == *fall_state {
			g = i32::MAX;
			*fall_state = FallState::Fall;
			*fall_countdown = Duration::from_secs(0);
		}else {
			while *fall_countdown >= *fall_duration {
				g += 1;
				*fall_countdown -= *fall_duration;
			}
		}
		
		append(UnitCommandKind::ApplyGravity(g));
		
		if *store {
			append(UnitCommandKind::Store);
			*store = false;
		}
	}
}
