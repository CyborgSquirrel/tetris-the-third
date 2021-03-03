use std::collections::VecDeque;
use rand::RngCore;
use serde::{Serialize,Deserialize};
use crate::{game, mino_controller::MinoController};
use crate::mino_controller;
use crate::mino::Mino;
use std::time::Duration;
use crate::NetworkState;
use crate::NetworkEvent;
use crate::{vec2i,vec2f};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum State {
	Play,
	LineClear{countdown: Duration},
	Over,
	Win,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Unit {
	pub base: Base,
	pub kind: Kind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Base {
	pub well: game::Well,
	pub animate_line: Vec<bool>,
	pub state: State,
	
	pub lines_cleared: u32,
	pub mode: Mode,
	
	pub falling_mino: Option<Mino>,
	pub can_store_mino: bool,
	pub stored_mino: Option<Mino>,
}

impl Base {
	pub fn new(mode: Mode) -> Self {
		Base {
			animate_line: vec![false; 20],
			state: State::Play,
			lines_cleared: 0,
			mode,
			can_store_mino: true,
			stored_mino: None,
			falling_mino: None,
			well: game::Well::filled_with(None, 10, 20),
		}
	}
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Kind {
	Local {
		rng: LocalMinoRng,
		mino_controller: MinoController,
	},
	Network {
		rng_queue: VecDeque<Mino>,
	}
}

impl Kind {
	pub fn local(mino_controller: MinoController) -> Kind {
		let mut rng = game::MinoRng::fair();
		Kind::Local {
			mino_controller,
			rng: LocalMinoRng {
				queue: {
					let mut queue = VecDeque::with_capacity(5);
					for _ in 0..5 {
						queue.push_back(rng.generate());
					}
					queue
				},
				rng
			}
		}
	}
}

impl Unit {
	pub fn local(mode: Mode, mino_controller: MinoController) -> Unit {
		let kind = Kind::local(mino_controller);
		Unit {
			base: Base::new(mode),
			kind,
		}
	}
	pub fn network(mode: Mode) -> Unit {
		Unit {
			base: Base::new(mode),
			kind: Kind::Network {
				rng_queue: VecDeque::new(),
			}
		}
	}
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LocalMinoRng {
	pub queue: VecDeque<Mino>,
	pub rng: game::MinoRng,
}

impl LocalMinoRng {
	pub fn next_mino(&mut self, network_state: &mut NetworkState, unit_id: usize) -> Mino {
		let Self{queue, rng} = self;
		let mino = queue.pop_front().unwrap();
		queue.push_back(rng.generate());
		network_state.broadcast_event(
			&NetworkEvent::UnitEvent {
				unit_id,
				event: UnitEvent::GenerateMino {mino: mino.clone()}
			}
		);
		mino
	}
	pub fn next_mino_centered(&mut self, network_state: &mut NetworkState, unit_id: usize, well: &game::Well) -> Mino {
		let mut mino = self.next_mino(network_state, unit_id);
		game::center_mino(&mut mino, well);
		mino
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
	Marathon{level: u32, level_target: u32, lines_before_next_level: i32},
	Sprint{lines_cleared_target: u32},
	Versus{lines_received: VecDeque<u32>, target_unit_id: usize},
}

impl Mode {
	pub fn default_marathon() -> Mode {
		Mode::Marathon {
			level_target: 50, level: 1,
			lines_before_next_level: get_lines_before_next_level(1),
		}
	}
	pub fn default_sprint() -> Mode {
		Mode::Sprint {
			lines_cleared_target: 40
		}
	}
	pub fn default_versus() -> Mode {
		Mode::Versus {
			lines_received: VecDeque::new(),
			target_unit_id: 0,
		}
	}
}

pub fn get_lines_before_next_level(level: u32) -> i32 {
	10 * (level as i32)
}

pub fn get_level_fall_duration(level: u32) -> Duration {
	let base: Duration = Duration::from_secs_f64(0.40);
	let level = (level-1) as f64;
	base.div_f64(1f64 + level * 0.15)
}

pub fn update_level(
	level: &mut u32,
	lines_before_next_level: &mut i32,
	clearable_lines: u32
) -> bool {
	*lines_before_next_level -= clearable_lines as i32;
	let mut level_changed = false;
	while *lines_before_next_level <= 0 {
		*level += 1;
		*lines_before_next_level +=
			get_lines_before_next_level(*level);
		level_changed = true;
	}
	level_changed
}

#[derive(Debug, Serialize, Deserialize)]
pub enum UnitEvent {
	TranslateMino {
		origin: vec2f,
		blocks: [vec2i; 4],
	},
	AddMinoToWell,
	GenerateMino {
		mino: Mino,
	},
	AddBottomLines {
		lines: usize,
		gap: usize,
	},
	StoreMino,
	Init,
}

pub fn update_local<F1,F2>(
	unit_id: usize,
	units: &mut [Unit],
	network_state: &mut crate::NetworkState,
	config: &crate::Config,
	softdrop_duration: Duration,
	dpf: Duration,
	other_rng: &mut rand::rngs::SmallRng,
	mut on_lines_cleared: F1,
	mut on_level_changed: F2,
) 
where F1: FnMut(u32), F2: FnMut(u32) {
	let Unit{base:Base{well,animate_line,lines_cleared,mode,falling_mino,can_store_mino,stored_mino,state},kind}= &mut units[unit_id];
	if let Kind::Local {mino_controller,rng} = kind {
		let mino_controller::MinoController {store,fall_countdown,rot_direction,move_direction,move_state,move_repeat_countdown,
		fall_duration,fall_state,..} = mino_controller;
		
		if let Some(falling_mino) = falling_mino {

			let mino_stored = game::mino_storage_system(
				falling_mino,
				stored_mino,
				well,
				Some(fall_countdown),
				store,
				can_store_mino,
				||rng.next_mino(network_state, unit_id),
				unit_id,
			);
			
			if mino_stored {
				network_state.broadcast_event(
					&NetworkEvent::UnitEvent {
						unit_id,
						event: UnitEvent::StoreMino,
					}
				);
			}
			
			let mut mino_translated = false;
			
			let crate::config::Player {move_prepeat_duration,move_repeat_duration,..} = &config.players[mino_controller.config_id];
			
			mino_translated |= 
				game::mino_rotation_system(
					falling_mino,
					&well,
					rot_direction);
			
			mino_translated |=
				game::mino_movement_system(
					falling_mino,
					&well,
					move_state, move_direction,
					move_repeat_countdown,
					*move_prepeat_duration, *move_repeat_duration,
					dpf);
		
			let (add_mino, mino_translated_while_falling) =
				game::mino_falling_system(
					falling_mino, &well,
					fall_countdown,
					*fall_duration, softdrop_duration,
					fall_state);
			
			mino_translated |= mino_translated_while_falling;
			
			*fall_countdown += dpf;
			
			if mino_translated {
				network_state.broadcast_event(
					&NetworkEvent::UnitEvent{unit_id,event:UnitEvent::TranslateMino{
						origin: falling_mino.origin,
						blocks: falling_mino.blocks.clone()
					}}
				);
			}
			
			if add_mino {
				let (can_add, clearable_lines, sendable_lines) =
				game::mino_adding_system(
					falling_mino, well,
					Some(fall_countdown),
					animate_line,
					can_store_mino,
					||rng.next_mino(network_state, unit_id));
				
				network_state.broadcast_event(
					&NetworkEvent::UnitEvent {
						unit_id,
						event: UnitEvent::AddMinoToWell
					}
				);
				
				if !can_add {
					*state = State::Over;
				}else {
					if let Mode::Versus {lines_received,..} = mode {
						while !lines_received.is_empty() {
							let lines = lines_received.pop_front().unwrap() as usize;
							let gap = other_rng.next_u32() as usize % well.num_rows();
							
							game::try_add_bottom_line_with_gap(
								well, lines, gap);
							
							network_state.broadcast_event(
								&NetworkEvent::UnitEvent {
									unit_id,
									event: UnitEvent::AddBottomLines {
										lines, gap
									}
								}
							);
						}
					}
					if clearable_lines > 0 {
						*state = State::LineClear{countdown: Duration::from_secs(0)};
						
						*lines_cleared += clearable_lines;
						on_lines_cleared(*lines_cleared);
						
						if let Mode::Marathon {level,lines_before_next_level,..} = mode {
							let level_changed = update_level(level, lines_before_next_level, clearable_lines);
							if level_changed {
								on_level_changed(*level);
								*fall_duration = get_level_fall_duration(*level);
							}
						}else if let Mode::Versus {target_unit_id,..} = mode {
							let target_unit_id = *target_unit_id;
							if let Unit{base:Base{mode:Mode::Versus{lines_received,..},..},..} = &mut units[target_unit_id] {
								lines_received.push_back(sendable_lines);
							}
						}
					}
				}
			}
		}
	}
}

pub fn update_network<F1,F2>(
	unit_id: usize,
	units: &mut [Unit],
	event: UnitEvent,
	mut on_lines_cleared: F1,
	mut on_level_changed: F2,
) 
where F1: FnMut(u32), F2: FnMut(u32) {
	if let Unit{base:Base{falling_mino,well,can_store_mino,lines_cleared,animate_line,stored_mino,state,mode},kind:Kind::Network{rng_queue},..} = &mut units[unit_id] {
		match event {
			UnitEvent::TranslateMino {origin, blocks} => {
				if let Some(falling_mino) = falling_mino {
					falling_mino.origin = origin;
					falling_mino.blocks = blocks;
				} else {panic!()}
			}
			UnitEvent::AddMinoToWell => {
				let falling_mino = falling_mino.as_mut().unwrap();
				*state = State::LineClear {countdown:Duration::from_secs(0)};
				let (_can_add, clearable_lines, sendable_lines) = game::mino_adding_system(
					falling_mino, well,
					None,
					animate_line,
					can_store_mino,
					&mut ||rng_queue.pop_back().unwrap()
				);
				
				if clearable_lines > 0 {
					*lines_cleared += clearable_lines;
					on_lines_cleared(*lines_cleared);
					if let Mode::Marathon {level,lines_before_next_level,..} = mode {
						let level_changed = update_level(level, lines_before_next_level, clearable_lines);
						if level_changed {
							on_level_changed(*level);
						}
					}else if let Mode::Versus {target_unit_id,..} = mode {
						let target_unit_id = *target_unit_id;
						if let Unit{base:Base{mode:Mode::Versus{lines_received,..},..},..} = &mut units[target_unit_id] {
							lines_received.push_back(sendable_lines);
						}
					}
				}
			}
			UnitEvent::AddBottomLines {lines, gap} => {
				game::try_add_bottom_line_with_gap(
					well, lines, gap);
			}
			UnitEvent::GenerateMino {mino} => {
				rng_queue.push_back(mino);
			}
			UnitEvent::Init => {
				let mut mino = rng_queue.pop_back().unwrap();
				game::center_mino(&mut mino, well);
				*falling_mino = Some(mino);
			}
			UnitEvent::StoreMino => {
				if let Some(falling_mino) = falling_mino {
					game::mino_storage_system(
						falling_mino,
						stored_mino,
						well,
						None,
						&mut true,
						can_store_mino,
						||rng_queue.pop_back().unwrap(),
						0
					);
				}
			}
		}
	}
}