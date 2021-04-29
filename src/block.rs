use std::time::Duration;

use crate::{vec2i,vec2f};
use serde::{Serialize,Deserialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct Data {
	texture_pos: vec2i,
}
impl Data {
	pub const BACKGROUND: Data = Data{texture_pos:vec2i{x:0,y:0}};
	pub const SHADOW: Data = Data{texture_pos:vec2i{x:1,y:0}};
	pub const GRAY: Data = Data{texture_pos:vec2i{x:3,y:0}};
	
	pub const CYAN: Data = Data{texture_pos:vec2i{x:2,y:0}};
	pub const ORANGE: Data = Data{texture_pos:vec2i{x:0,y:1}};
	pub const BLUE: Data = Data{texture_pos:vec2i{x:1,y:1}};
	pub const PINK: Data = Data{texture_pos:vec2i{x:2,y:1}};
	pub const GREEN: Data = Data{texture_pos:vec2i{x:0,y:2}};
	pub const PURPLE: Data = Data{texture_pos:vec2i{x:1,y:2}};
	pub const YELLOW: Data = Data{texture_pos:vec2i{x:2,y:2}};
	
	pub const SENT_LINE: Data = Data{texture_pos:vec2i{x:3,y:1}};
	pub const EMPTY_LINE: Data = Data{texture_pos:vec2i{x:3,y:2}};
	
	pub fn new(x: i32, y: i32) -> Self {
		Self {
			texture_pos: vec2i::new(x,y),
		}
	}
}

use crate::Mino;
use crate::game::Well;
use sdl2::rect::Rect;
use sdl2::render::{Texture, WindowCanvas};

pub struct Canvas<'a> {
	texture: Texture<'a>,
	block_size_tex: u32,
	block_size_draw: u32,
}

impl<'a> Canvas<'a> {
	pub fn new(texture: Texture<'a>, block_size_tex: u32, block_size_draw: u32) -> Canvas<'a> {
		Self {
			texture,
			block_size_tex,
			block_size_draw,
		}
	}
	pub fn draw_block(&self, canvas: &mut WindowCanvas, origin: vec2i, block: &vec2i, data: &Data) {
		let block_size_tex_i32 = self.block_size_tex as i32;
		let block_size_draw_i32 = self.block_size_draw as i32;
		let _ = canvas.copy(
			&self.texture,
			
			Rect::new(
				data.texture_pos.x * block_size_tex_i32,
				data.texture_pos.y * block_size_tex_i32,
				self.block_size_tex, self.block_size_tex),
			
			Rect::new(
				origin.x + block.x * block_size_draw_i32,
				origin.y + block.y * block_size_draw_i32,
				self.block_size_draw, self.block_size_draw)
		);
	}
	fn draw_flash(&self, canvas: &mut WindowCanvas, origin: vec2i, block: &vec2i) {
		let block_size_draw_i32 = self.block_size_draw as i32;
		canvas.set_draw_color(sdl2::pixels::Color::WHITE);
		let _ = canvas.fill_rect(
			Rect::new(
				origin.x + block.x * block_size_draw_i32,
				origin.y + block.y * block_size_draw_i32,
				self.block_size_draw, self.block_size_draw)
		);
	}
	pub fn draw_mino(&self, canvas: &mut WindowCanvas, origin: vec2i, mino: &Mino) {
		for (block, data) in mino.blocks.iter().zip(mino.blocks_data.iter()) {
			self.draw_block(canvas, origin, block, data);
		}
	}
	// over here, the size is in blocks (so not pixels)
	pub fn draw_mino_centered(&self, canvas: &mut WindowCanvas, mut origin: vec2i, mino: &Mino, size: vec2i) {
		let margin = (vec2f::from(size) - vec2f::from(mino.get_size())) / 2f64;
		origin += vec2i::from(margin * self.block_size_draw.into());
		for (block, data) in mino.blocks.iter().zip(mino.blocks_data.iter()) {
			self.draw_block(canvas, origin, block, data);
		}
	}
	pub fn draw_well(
		&mut self, canvas: &mut WindowCanvas, origin: vec2i, well: &Well,
		lc_animation: &Option<crate::unit::LCAnimation>, gol_animation: &Option<crate::unit::GOLAnimation>,
		countdown: Duration,
	) {
		let f = countdown.as_secs_f64() / (if lc_animation.is_some() {*crate::LINE_CLEAR_DURATION} else {*crate::GAME_OF_LIFE_DURATION}).as_secs_f64();
		for y in 0..well.row_len() {
			for x in 0..well.column_len() {
				let p = vec2i!(x as i32, y as i32);
				if let Some(lc_animation) = lc_animation {
					if let Some(data) = well[(x,y)] {
						if lc_animation.animate_line[y] {self.draw_flash(canvas, origin, &p)}
						else {self.draw_block(canvas, origin, &p, &data)}
					}else {self.draw_block(canvas, origin, &p, &Data::BACKGROUND)}
				}else if let Some(gol_animation) = gol_animation {
					if let Some(data) = well[(x,y)] {self.draw_block(canvas, origin, &p, &data)}
					else {self.draw_block(canvas, origin, &p, &Data::BACKGROUND)}
					if let Some(data) = gol_animation.animate_block[(x,y)] {
						self.texture.set_alpha_mod((255f64*f) as u8);
						self.draw_block(canvas, origin, &p, &data);
						self.texture.set_alpha_mod(255);
					}
				}else {
					if let Some(data) = well[(x,y)] {self.draw_block(canvas, origin, &p, &data)}
					else {self.draw_block(canvas, origin, &p, &Data::BACKGROUND)}
				}
			}
		}
	}
}