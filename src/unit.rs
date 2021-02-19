use std::collections::VecDeque;
use serde::{Serialize,Deserialize};
use crate::game;
use crate::player;
use crate::mino::Mino;
use std::time::Duration;
use crate::NetworkState;
use crate::NetworkEvent;
use crate::{vec2i,vec2f};

#[derive(Debug, Serialize, Deserialize)]
pub enum State {
	Play,
	LineClear{countdown: Duration},
	Over,
	Win,
}

pub struct Unit {
	pub base: Base,
	pub kind: Kind,
	// pub lines_cleared_text: sdl2::render::Texture<'a>,
}

#[derive(Debug, Serialize, Deserialize)]
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

pub enum Kind {
	Local {
		rng: LocalMinoRng,
		player: player::Player,
	},
	Network {
		rng_queue: VecDeque<Mino>,
	}
}

impl Kind {
	pub fn local(player: player::Player) -> Kind {
		let mut rng = game::MinoRng::fair();
		Kind::Local {
			player,
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
	pub fn local(mode: Mode, player: player::Player) -> Unit {
		let well = game::Well::filled_with(None, 10, 20);
		let kind = Kind::local(player);
		Unit {
			base: Base {
				animate_line: vec![false; 20],
				state: State::Play,
				lines_cleared: 0,
				mode,
				can_store_mino: true,
				stored_mino: None,
				falling_mino: None,
				well,
			},
			kind,
		}
	}
	pub fn network(mode: Mode) -> Unit {
		let well = game::Well::filled_with(None, 10, 20);
		Unit {
			base: Base {
				animate_line: vec![false; 20],
				state: State::Play,
				lines_cleared: 0,
				mode,
				can_store_mino: true,
				stored_mino: None,
				falling_mino: None,
				well,
			},
			kind: Kind::Network {
				rng_queue: VecDeque::new(),
			}
		}
	}
}

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

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
	Marathon{level: u32, level_target: u32, lines_before_next_level: i32},
	Sprint{lines_cleared_target: u32},
	Versus{lines_received: u32, target_unit_id: usize},
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
			lines_received: 0,
			target_unit_id: 0,
		}
	}
}

pub fn get_lines_before_next_level(level: u32) -> i32 {
	10 * (level as i32)
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
	StoreMino,
	Init,
}