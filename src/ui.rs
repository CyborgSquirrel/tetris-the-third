use crate::vec2i;
use enum_select_derive::EnumSelect;
use serde::{Serialize,Deserialize};

pub trait EnumSelect {
	fn next_variant(self) -> Self;
	fn prev_variant(self) -> Self;
}

#[derive(PartialEq,EnumSelect,Clone,Copy)]
pub enum TitleSelection {
	Continue,
	NewGame,
	GameMode,	
	NetworkMode,
}

#[derive(EnumSelect,Clone,Copy)]
pub enum PauseSelection {
	Resume,
	Save,
	Restart,
	QuitToTitle,
	QuitToDesktop,
}
impl Default for PauseSelection {
	fn default() -> Self {PauseSelection::Resume}
}

#[derive(PartialEq,EnumSelect)]
pub enum NetworkStateSelection {
	Offline,
	Host,
	Client,
}

#[derive(Default)]
pub struct GameLayout {
	pub x: i32,
	pub y: i32,
	pub width: i32,
	pub expected_width: i32,
}

impl GameLayout {
	pub fn centered_x(&self) -> i32 {
		((self.width-self.expected_width) / 2) as i32
	}
	pub fn x(&self) -> i32 {
		return self.centered_x()+self.x;
	}
	pub fn y(&self) -> i32 {
		return self.y;
	}
	pub fn as_vec2i(&self) -> vec2i {
		vec2i!(self.x(),self.y())
	}
	pub fn row(&mut self, y: i32) {
		self.y += y;
	}
	pub fn row_margin(&mut self, y: i32) {
		self.y += y;
	}
	pub fn col(&mut self, x: i32) {
		self.y = 0;
		self.x += x;
	}
	pub fn col_margin(&mut self, x: i32) {
		self.y = 0;
		self.x += x;
	}
}

pub struct CenteredLayout {
	pub y: i32,
	pub width: u32
}

impl CenteredLayout {
	pub fn centered_x(&self, obj_width: u32) -> i32 {
		((self.width-obj_width) / 2) as i32
	}
	pub fn row(&mut self, y: i32) {
		self.y += y;
	}
	pub fn row_margin(&mut self, y: i32) {
		self.y += y;
	}
}

#[derive(Default,Clone,Copy)]
pub struct Pause {
	pub selection: PauseSelection,
}

#[derive(Debug, EnumSelect, Serialize, Deserialize, Clone, Copy)]
pub enum GameModeSelection {
	Marathon,
	Sprint,
	Versus,
	GameOfLife,
}

use crate::unit::Mode;
impl GameModeSelection {
	pub fn mode(&self) -> Mode {
		use GameModeSelection::*;
		match *self {
			Marathon => Mode::default_marathon(),
			Sprint => Mode::default_sprint(),
			Versus => Mode::default_versus(),
			GameOfLife => Mode::default_game_of_life(),
		}
	}
}

impl Default for GameModeSelection {
	fn default() -> Self {GameModeSelection::Marathon}
}