use std::collections::VecDeque;

use crate::{Player, State, command::Command, ui::GameModeSelection, unit::Unit};
use itertools::izip;
use serde::{Serialize, Deserialize};

use crate::PlayerKind;
use crate::unit::Mode;
use crate::unit::Kind;
use crate::unit::UnitCommandKind;
use crate::MinoController;

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Room {
	pub selected_game_mode: GameModeSelection,
	pub players: Vec<Player>,
	pub units: Vec<Unit>,
	pub commands: VecDeque<crate::unit::UnitCommand>,
	
	pub just_added_player: bool,
	pub just_initted: bool,
}

impl Room {
	pub fn reset_flags(&mut self) {
		self.just_added_player = false;
		self.just_initted = false;
	}
}

#[derive(Serialize, Deserialize, Clone)]
pub enum RoomCommand {
	Init(Room),
	StartGame,
	StartGameFromSave(Unit),
	AddPlayer(Player),
}
impl<'a> Command<'a> for RoomCommand {
	type Params = (&'a mut Room, &'a mut State);
	fn execute<F>(
		self, _append: F,
		(room, state): Self::Params,
	) {
		match self {
			RoomCommand::Init(init_room) => {
				*room = init_room;
				room.just_initted = true;
			}
			RoomCommand::StartGame => {
				room.units.clear();
				let mut configs = (0..4usize).cycle();
				*state = State::play();
				let players_len = room.players.len();
				for (unit_id, player) in izip!(0.., &room.players) {
					let mut unit = match player.kind {
						PlayerKind::Local => Unit::local(room.selected_game_mode.ctor()(), MinoController::new(configs.next().unwrap(), None)),
						PlayerKind::Network => Unit::network(room.selected_game_mode.ctor()()),
					};
					let Unit{kind, base} = &mut unit;
					
					if let Mode::Versus {target_unit_id,..} = &mut base.mode {
						*target_unit_id = (unit_id+1usize).rem_euclid(players_len);
					}
					
					if let Kind::Local{rng,..} = kind {
						room.commands.push_back((unit_id, UnitCommandKind::NextMino(rng.next_mino_centered_bro(&base.well))).wrap());
					}
					
					room.units.push(unit);
				}
			}
			RoomCommand::StartGameFromSave(unit) => {
				*state = State::play();
				room.units.push(unit);
			}
			RoomCommand::AddPlayer(player) => {
				println!("YO");
				room.players.push(player);
				room.just_added_player = true;
			}
		}
	}
}