use std::collections::VecDeque;
use itertools::izip;
use serde::{Serialize,Deserialize};
use crate::{command::{Command, CommandWrapper}, game, mino_controller::MinoController};
use crate::mino::Mino;
use std::time::Duration;
use crate::NetworkState;
use crate::{vec2i,vec2f};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum State {
	Play,
	LineClear{countdown: Duration},
	Lose,
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
	
	pub just_changed_mino: bool,
	pub just_cleared_lines: bool,
	pub just_lost: bool,
	pub just_changed_level: bool,
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
			
			just_changed_mino: false,
			just_cleared_lines: false,
			just_lost: false,
			just_changed_level: false,
		}
	}
	pub fn reset_flags(&mut self) {
		self.just_changed_mino = false;
		self.just_cleared_lines = false;
		self.just_lost = false;
		self.just_changed_level = false;
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
	// pub fn next_falling_mino(&mut self, unit_id: usize, network_state: &mut NetworkState) {
	// 	self.base.just_changed_mino = true;
	// 	self.base.falling_mino.replace(match &mut self.kind {
	// 		Kind::Local {rng,..} => rng.next_mino_centered(network_state, unit_id, &self.base.well),
	// 		Kind::Network {rng_queue} => rng_queue.pop_front().unwrap(),
	// 	});
	// }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LocalMinoRng {
	pub queue: VecDeque<Mino>,
	pub rng: game::MinoRng,
}

impl LocalMinoRng {
	// pub fn next_mino(&mut self, network_state: &mut NetworkState, unit_id: usize) -> Mino {
	// 	let Self{queue, rng} = self;
	// 	let mino = queue.pop_front().unwrap();
	// 	queue.push_back(rng.generate());
	// 	network_state.broadcast_event(
	// 		&NetworkEvent::MinoGenerate {
	// 			unit_id,
	// 			mino: mino.clone(),
	// 		}
	// 	);
	// 	mino
	// }
	// pub fn next_mino_centered(&mut self, network_state: &mut NetworkState, unit_id: usize, well: &game::Well) -> Mino {
	// 	let mut mino = self.next_mino(network_state, unit_id);
	// 	game::center_mino(&mut mino, well);
	// 	mino
	// }
	pub fn next_mino_bro(&mut self) -> Mino {
		let Self{queue, rng} = self;
		let mino = queue.pop_front().unwrap();
		queue.push_back(rng.generate());
		mino
	}
	pub fn next_mino_centered_bro(&mut self, well: &game::Well) -> Mino {
		let mut mino = self.next_mino_bro();
		game::center_mino(&mut mino, well);
		mino
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
	Marathon{level: u32, level_target: u32, lines_before_next_level: i32},
	Sprint{lines_cleared_target: u32},
	Versus{lines_received: VecDeque<usize>, lines_received_sum: usize, target_unit_id: usize},
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
			lines_received_sum: 0,
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



pub type UnitCommand = CommandWrapper<UnitCommandInner>;



#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum UnitCommandKind {
	MoveLeft, MoveRight,
	RotateLeft, RotateRight,
	ApplyGravity(i32),
	Store,
	ClearLines,
	NextMino(Mino),
	SendLines(usize), AddLines(usize,usize),
}

pub type UnitCommandInner = (usize, UnitCommandKind);

impl<'a> Command<'a> for UnitCommandInner {
	type Params = &'a mut Unit;
	fn execute<F>(
		self, mut append: F,
		unit: Self::Params,
	) where F: FnMut(Self) {
		use UnitCommandKind::*;
		let (unit_id, kind) = self;
		let base = &mut unit.base;
		let mut append = |unit_id, command|append((unit_id, command));
		match kind {
			MoveLeft =>
				if let Some(falling_mino) = &mut base.falling_mino {
					game::try_left_mino(falling_mino, &base.well);
				}
			MoveRight =>
				if let Some(falling_mino) = &mut base.falling_mino {
					game::try_right_mino(falling_mino, &base.well);
				}
			RotateLeft =>
				if let Some(falling_mino) = &mut base.falling_mino {
					game::try_rotl_mino(falling_mino, &base.well);
				}
			RotateRight =>
				if let Some(falling_mino) = &mut base.falling_mino {
					game::try_rotr_mino(falling_mino, &base.well);
				}
			ApplyGravity(mut g) => {
				if let Some(falling_mino) = &mut base.falling_mino {
					while g > 0 && game::try_down_mino(falling_mino, &base.well) {
						g -= 1;
					}
					
					let add_mino = g > 0;
					if add_mino {
						let can_add = game::mino_fits_in_well(&falling_mino, &base.well);
						let mut clearable_lines = 0;
						let mut sendable_lines = 0;
						if !can_add {
							base.just_lost = true;
						}else {
							base.can_store_mino = true;
							game::add_mino_to_well(&falling_mino, &mut base.well);
							
							for (row,clearable) in izip!(base.well.columns_iter(),base.animate_line.iter_mut()) {
								let mut count = 0;
								let mut sendable = true;
								for block in row {
									count += block.is_some() as u32;
									sendable &= block.map_or(true, |block|block!=crate::block::Data::GRAY);
								}
								if count as usize == base.well.column_len() {
									clearable_lines += 1;
									sendable_lines += sendable as usize;
									*clearable = true;
								}
							}
							
							if let Mode::Versus {lines_received,lines_received_sum,..} = &mut base.mode {
								if let Kind::Local {..} = unit.kind {
									while let Some(lines) = lines_received.pop_front() {
										let row = rand::random::<usize>() % base.well.num_rows();
										append(unit_id, AddLines(lines,row));
										*lines_received_sum -= lines;
									}
								}
							}
							
							if clearable_lines > 0 {
								base.just_cleared_lines = true;
								base.state = State::LineClear{countdown: Duration::from_secs(0)};
								base.lines_cleared += clearable_lines;
								if let Mode::Marathon {level,lines_before_next_level,..} = &mut base.mode {
									let level_changed = update_level(level, lines_before_next_level, clearable_lines);
									if level_changed {
										base.just_changed_level = true;
									}
								}else if let Mode::Versus {target_unit_id,..} = &mut base.mode {
									if let Kind::Local {..} = unit.kind {
										append(*target_unit_id, SendLines(sendable_lines));
									}
								}
							}
							
							// Is this ok I wonder?
							if let Kind::Local {rng, ..} = &mut unit.kind {
								append(unit_id, NextMino(rng.next_mino_centered_bro(&unit.base.well)));
							}
						}
					}
				}
			}
			Store => {
				if base.can_store_mino {
					base.can_store_mino = false;
					if let Some(mut falling_mino) = base.falling_mino.take() {
						game::reset_mino(&mut falling_mino);
						if let Some(mut stored_mino) = base.stored_mino.take() {
							base.just_changed_mino = true;
							game::center_mino(&mut stored_mino, &base.well);
							base.falling_mino = Some(stored_mino);
						}else {
							if let Kind::Local {rng, ..} = &mut unit.kind {
								append(unit_id, NextMino(rng.next_mino_centered_bro(&unit.base.well)));
							}
						}
						unit.base.stored_mino = Some(falling_mino);
					}
				}
			}
			ClearLines => {
				for line in &mut base.animate_line {
					*line = false;
				}
				game::try_clear_lines(&mut base.well);
				base.state = State::Play;
			}
			NextMino(mino) => {
				base.just_changed_mino = true;
				base.falling_mino.replace(mino);
			}
			SendLines(lines) => {
				if let Mode::Versus {lines_received, lines_received_sum, ..} = &mut base.mode {
					lines_received.push_back(lines);
					*lines_received_sum += lines;
				}
			}
			AddLines(lines, gap) => {
				if lines == 0 {return}
				for y in 0..base.well.row_len() {
					for x in 0..base.well.column_len() {
						if base.well[(x,y)].is_some() {
							if y >= lines {
								base.well[(x,y-lines)] = base.well[(x,y)];
							}
						}
						base.well[(x,y)] = if y >= base.well.row_len()-lines && x != gap
						{Some(crate::block::Data::GRAY)} else {None}
					}
				}
			}
		}
	}
}

// pub fn update_local<F1,F2,F3>(
// 	unit_id: usize,
// 	units: &mut [Unit],
// 	network_state: &mut crate::NetworkState,
// 	config: &crate::Config,
// 	softdrop_duration: Duration,
// 	dpf: Duration,
// 	other_rng: &mut rand::rngs::SmallRng,
// 	mut on_lines_cleared: F1,
// 	mut on_level_changed: F2,
// 	mut on_lose: F3,
// ) 
// where F1: FnMut(u32), F2: FnMut(u32), F3: FnMut() {
// 	let Unit{base:Base{well,animate_line,lines_cleared,mode,falling_mino,can_store_mino,stored_mino,state,..},kind}= &mut units[unit_id];
// 	if let Kind::Local {mino_controller,rng} = kind {
// 		let mino_controller::MinoController {store,fall_countdown,rot_direction,move_direction,move_state,move_repeat_countdown,
// 		fall_duration,fall_state,..} = mino_controller;
		
// 		if let Some(falling_mino) = falling_mino {

// 			let mino_stored = game::mino_storage_system(
// 				falling_mino,
// 				stored_mino,
// 				well,
// 				Some(fall_countdown),
// 				store,
// 				can_store_mino,
// 				||rng.next_mino(network_state, unit_id),
// 				unit_id,
// 			);
			
// 			if mino_stored {
// 				network_state.broadcast_event(
// 					&NetworkEvent::UnitEvent {
// 						unit_id,
// 						event: UnitEvent::StoreMino,
// 					}
// 				);
// 			}
			
// 			let mut mino_translated = false;
			
// 			let crate::config::Player {move_prepeat_duration,move_repeat_duration,..} = &config.players[mino_controller.config_id];
			
// 			mino_translated |= 
// 				game::mino_rotation_system(
// 					falling_mino,
// 					&well,
// 					rot_direction);
			
// 			mino_translated |=
// 				game::mino_movement_system(
// 					falling_mino,
// 					&well,
// 					move_state, move_direction,
// 					move_repeat_countdown,
// 					*move_prepeat_duration, *move_repeat_duration,
// 					dpf);
		
// 			let (add_mino, mino_translated_while_falling) =
// 				game::mino_falling_system(
// 					falling_mino, &well,
// 					fall_countdown,
// 					*fall_duration, softdrop_duration,
// 					fall_state);
			
// 			mino_translated |= mino_translated_while_falling;
			
// 			*fall_countdown += dpf;
			
// 			if mino_translated {
// 				network_state.broadcast_event(
// 					&NetworkEvent::UnitEvent{unit_id,event:UnitEvent::TranslateMino{
// 						origin: falling_mino.origin,
// 						blocks: falling_mino.blocks.clone()
// 					}}
// 				);
// 			}
			
// 			if add_mino {
// 				let (can_add, clearable_lines, sendable_lines) =
// 				game::mino_adding_system(
// 					falling_mino, well,
// 					Some(fall_countdown),
// 					animate_line,
// 					can_store_mino,
// 					||rng.next_mino(network_state, unit_id));
				
// 				network_state.broadcast_event(
// 					&NetworkEvent::UnitEvent {
// 						unit_id,
// 						event: UnitEvent::AddMinoToWell
// 					}
// 				);
				
// 				if !can_add {
// 					*state = State::Lose;
// 					on_lose();
// 				}else {
// 					if let Mode::Versus {lines_received,lines_received_sum,..} = mode {
// 						while !lines_received.is_empty() {
// 							let lines = lines_received.pop_front().unwrap();
// 							let gap = other_rng.next_u32() as usize % well.num_rows();
// 							*lines_received_sum -= lines;
							
// 							game::try_add_bottom_line_with_gap(
// 								well, lines as usize, gap);
							
// 							network_state.broadcast_event(
// 								&NetworkEvent::UnitEvent {
// 									unit_id,
// 									event: UnitEvent::AddBottomLines {
// 										lines: lines as usize, gap
// 									}
// 								}
// 							);
// 						}
// 					}
// 					if clearable_lines > 0 {
// 						*state = State::LineClear{countdown: Duration::from_secs(0)};
						
// 						*lines_cleared += clearable_lines;
// 						on_lines_cleared(*lines_cleared);
						
// 						if let Mode::Marathon {level,lines_before_next_level,..} = mode {
// 							let level_changed = update_level(level, lines_before_next_level, clearable_lines);
// 							if level_changed {
// 								on_level_changed(*level);
// 								*fall_duration = get_level_fall_duration(*level);
// 							}
// 						}else if let Mode::Versus {target_unit_id,..} = mode {
// 							let target_unit_id = *target_unit_id;
// 							if let Unit{base:Base{mode:Mode::Versus{lines_received,lines_received_sum,..},..},..} = &mut units[target_unit_id] {
// 								lines_received.push_back(sendable_lines);
// 								*lines_received_sum += sendable_lines;
// 							}
// 						}
// 					}
// 				}
// 			}
// 		}
// 	}
// }