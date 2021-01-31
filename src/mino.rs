#![allow(dead_code)]
use crate::vec2::{vec2f,vec2i};
use crate::block::Data;
use serde::{Serialize,Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mino {
	pub origin: vec2f,
	pub rotation: i32,
	pub blocks: [vec2i; 4],
	pub blocks_data: [Data; 4],
}
impl Mino {
	fn new(
		origin: vec2f,
		b1: vec2i, b2: vec2i,
		b3: vec2i, b4: vec2i,
		d1: Data, d2: Data,
		d3: Data, d4: Data,
	) -> Self {
		Mino {origin, rotation:0, blocks:[b1,b2,b3,b4], blocks_data:[d1,d2,d3,d4]}
	}
	
	pub fn rotr(&mut self) {
		self.rotation += 1;
		for block in self.blocks.iter_mut() {
			*block = {
				let mut block = vec2f::from(*block);
				block -= self.origin;
				block = block.rot90r();
				block += self.origin;
				block.round()
			};
		}
	}
	pub fn rotl(&mut self) {
		self.rotation -= 1;
		for block in self.blocks.iter_mut() {
			*block = {
				let mut block = vec2f::from(*block);
				block -= self.origin;
				block = block.rot90l();
				block += self.origin;
				block.round()
			};
		}
	}
	
	pub fn translate(&mut self, v: vec2i) {
		self.origin += vec2f::from(v);
		for block in self.blocks.iter_mut() {
			*block += v;
		}
	}
	pub fn right(&mut self) {
		self.translate(vec2i!(1,0));
	}
	pub fn left(&mut self) {
		self.translate(vec2i!(-1,0));
	}
	pub fn down(&mut self) {
		self.translate(vec2i!(0,1));
	}
	
	pub fn make_shadow(&mut self) {
		self.blocks_data = [Data::SHADOW; 4];
	}
	
	pub fn l() -> Mino {
		Mino::new(
			vec2f!(0,1),
			vec2i!(0,0),vec2i!(0,1),
			vec2i!(0,2),vec2i!(1,2),
			Data::BLUE,Data::BLUE,
			Data::BLUE,Data::BLUE)
	}
	pub fn j() -> Mino {
		Mino::new(
			vec2f!(1,1),
			vec2i!(1,0),vec2i!(1,1),
			vec2i!(1,2),vec2i!(0,2),
			Data::ORANGE,Data::ORANGE,
			Data::ORANGE,Data::ORANGE)
	}
	pub fn o() -> Mino {
		Mino::new(
			vec2f!(0.5,0.5),
			vec2i!(0,0),vec2i!(1,0),
			vec2i!(0,1),vec2i!(1,1),
			Data::YELLOW,Data::YELLOW,
			Data::YELLOW,Data::YELLOW)
	}
	pub fn z() -> Mino {
		Mino::new(
			vec2f!(1,1),
			vec2i!(0,1),vec2i!(1,1),
			vec2i!(1,0),vec2i!(2,0),
			Data::GREEN,Data::GREEN,
			Data::GREEN,Data::GREEN)
	}
	pub fn s() -> Mino {
		Mino::new(
			vec2f!(1,1),
			vec2i!(0,0),vec2i!(1,0),
			vec2i!(1,1),vec2i!(2,1),
			Data::PURPLE,Data::PURPLE,
			Data::PURPLE,Data::PURPLE)
	}
	pub fn t() -> Mino {
		Mino::new(
			vec2f!(1,1),
			vec2i!(0,1),vec2i!(1,1),
			vec2i!(2,1),vec2i!(1,0),
			Data::PINK,Data::PINK,
			Data::PINK,Data::PINK)
	}
	pub fn i() -> Mino {
		Mino::new(
			vec2f!(1.5,0.5),
			vec2i!(0,0),vec2i!(1,0),
			vec2i!(2,0),vec2i!(3,0),
			Data::CYAN,Data::CYAN,
			Data::CYAN,Data::CYAN)
	}
}