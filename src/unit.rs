use std::collections::VecDeque;
use itertools::izip;
use serde::{Serialize,Deserialize};
use crate::{command::{Command, CommandWrapper}, game, mino_controller::MinoController};
use crate::mino::Mino;
use std::time::Duration;
use std::convert::TryFrom;

use crate::{vec2i,vec2f};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum State {
	Play,
	LineClear {countdown: Duration},
	GameOfLife {countdown: Duration},
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
	pub just_won: bool,
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
			just_won: false,
			just_changed_level: false,
		}
	}
	pub fn reset_flags(&mut self) {
		self.just_changed_mino = false;
		self.just_cleared_lines = false;
		self.just_lost = false;
		self.just_won = false;
		self.just_changed_level = false;
	}
	pub fn win(&mut self) {
		if !matches!(self.state, State::Win) {
			self.state = State::Win;
			self.just_won = true;
		}
	}
	pub fn lose(&mut self) {
		if !matches!(self.state, State::Lose) {
			self.state = State::Lose;
			self.just_lost = true;
		}
	}
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Kind {
	Local {
		rng: LocalMinoRng,
		mino_controller: MinoController,
	},
	Network
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
						// queue.push_back(rng.generate());
						queue.push_back(Mino::i());
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
		Unit {
			base: Base::new(mode),
			kind: Kind::local(mino_controller),
		}
	}
	pub fn network(mode: Mode) -> Unit {
		Unit {
			base: Base::new(mode),
			kind: Kind::Network,
		}
	}
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LocalMinoRng {
	pub queue: VecDeque<Mino>,
	pub rng: game::MinoRng,
}

impl LocalMinoRng {
	pub fn next_mino(&mut self) -> Mino {
		let Self {queue, rng} = self;
		let mino = queue.pop_front().unwrap();
		queue.push_back(rng.generate());
		mino
	}
	pub fn next_mino_centered(&mut self, well: &game::Well) -> Mino {
		let mut mino = self.next_mino();
		game::center_mino(&mut mino, well);
		mino
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
	Marathon {level: u32, level_target: u32, lines_before_next_level: i32},
	Sprint {lines_cleared_target: u32},
	Versus {lines_received: VecDeque<usize>, lines_received_sum: usize, target_unit_id: usize},
	GameOfLife {count: u32},
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
	pub fn default_game_of_life() -> Mode {
		Mode::GameOfLife {
			count: 0,
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
	PreClearLines, ClearLines,
	PreGameOfLife, GameOfLife,
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
						if !can_add {
							base.lose();
						}else {
							base.can_store_mino = true;
							game::add_mino_to_well(&falling_mino, &mut base.well);
							
							if let Kind::Local {rng, ..} = &mut unit.kind {
								append(unit_id, NextMino(rng.next_mino_centered(&unit.base.well)));
								append(unit_id, PreClearLines);
								append(unit_id, PreGameOfLife);
								println!("hi");
							}
						}
					}
				}
			}
			PreClearLines => {
				let mut clearable_lines = 0;
				let mut sendable_lines = 0;
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
				
				if let Mode::Versus {lines_received,..} = &mut base.mode {
					if let Kind::Local {..} = unit.kind {
						while let Some(lines) = lines_received.pop_front() {
							let row = rand::random::<usize>() % base.well.num_rows();
							append(unit_id, AddLines(lines,row));
						}
					}
				}
				
				if clearable_lines > 0 {
					base.just_cleared_lines = true;
					base.state = State::LineClear {countdown: Duration::from_secs(0)};
					base.lines_cleared += clearable_lines;
					match &mut base.mode {
						Mode::Marathon {level,lines_before_next_level,..} => {
							let level_changed = update_level(level, lines_before_next_level, clearable_lines);
							if level_changed {base.just_changed_level = true}
						}
						Mode::Versus {target_unit_id,..} => {
							if let Kind::Local {..} = unit.kind {
								append(*target_unit_id, SendLines(sendable_lines));
							}
						}
						_ => {}
					}
				}
			}
			ClearLines => {
				for line in &mut base.animate_line {*line = false}
				game::try_clear_lines(&mut base.well);
				match &base.mode {
					Mode::Marathon {level,level_target,..} =>
					if *level >= *level_target {base.win()}
					Mode::Sprint {lines_cleared_target} =>
					if base.lines_cleared >= *lines_cleared_target {base.win()}
					_ => {}
				}
			}
			PreGameOfLife => {
				base.state = State::GameOfLife {countdown: Duration::from_secs(0)};
				println!("hello");
			}
			GameOfLife => {
				if let Mode::GameOfLife {count} = &mut base.mode {
					*count += 1;
					
					// println!("{:?}", count);
					
					if *count % 4 == 0 { //TODO: change me
						let mut new_well = game::Well::filled_with(None, base.well.num_rows(), base.well.num_columns());
						let yo = |x: Option<usize>, y: Option<usize>| -> bool {
							if let (Some(x), Some(y)) = (x, y) {
								if let Some(block) = base.well.get(x, y) {
									block.is_some()
								}else {false}
							}else {false}
						};
						let dx = vec![0, 1, 0, -1];
						let dy = vec![1, 0, -1, 0];
						for x in 0..new_well.column_len() as i32 {
							for y in 0..new_well.row_len() as i32 {
								let mut count = 0;
								for (dx, dy) in izip!(&dx, &dy) {
									let (nx, ny) = (x - *dx, y - *dy);
									let (nx, ny) = (usize::try_from(nx).ok(), usize::try_from(ny).ok());
									count += yo(nx, ny) as i32;
								}
								let block = &base.well[(x as usize, y as usize)];
								new_well[(x as usize, y as usize)] =
									if block.is_some() {if count == 3 {*block} else {None}}
									else {if count == 3 {Some(crate::block::Data::BLUE)} else {None}}
							}
						}
						base.well = new_well;
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
								append(unit_id, NextMino(rng.next_mino_centered(&unit.base.well)));
							}
						}
						unit.base.stored_mino = Some(falling_mino);
					}
				}
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
				if let Mode::Versus {lines_received_sum, ..} = &mut base.mode {
					*lines_received_sum -= lines;
				}
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