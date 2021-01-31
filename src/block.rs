use crate::vec2i;
use serde::{Serialize,Deserialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Data {
	texture_pos: vec2i,
}
impl Data {
	pub const BACKGROUND: Data = Data{texture_pos:vec2i{x:0,y:0}};
	pub const SHADOW: Data = Data{texture_pos:vec2i{x:1,y:0}};
	
	pub const CYAN: Data = Data{texture_pos:vec2i{x:2,y:0}};
	pub const ORANGE: Data = Data{texture_pos:vec2i{x:0,y:1}};
	pub const BLUE: Data = Data{texture_pos:vec2i{x:1,y:1}};
	pub const PINK: Data = Data{texture_pos:vec2i{x:2,y:1}};
	pub const GREEN: Data = Data{texture_pos:vec2i{x:0,y:2}};
	pub const PURPLE: Data = Data{texture_pos:vec2i{x:1,y:2}};
	pub const YELLOW: Data = Data{texture_pos:vec2i{x:2,y:2}};
	
	pub fn new(x: i32, y: i32) -> Self {
		Self {
			texture_pos: vec2i::new(x,y),
		}
	}
}

use crate::Mino;
use crate::Well;
use sdl2::rect::Rect;
use sdl2::render::{Texture, WindowCanvas};

pub struct Canvas<'a> {
	texture: &'a Texture<'a>,
	block_size_tex: u32,
	block_size_draw: u32,
}

impl<'a> Canvas<'a> {
	pub fn new(texture: &'a Texture<'a>) -> Canvas<'a> {
		Self {
			texture,
			block_size_tex: 24,
			block_size_draw: 30,
		}
	}
	fn draw_block(&self, canvas: &mut WindowCanvas, origin: vec2i, block: &vec2i, data: &Data) {
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
	pub fn draw_well(&self, canvas: &mut WindowCanvas, origin: vec2i, well: &Well, animate_line: &Vec<bool>) {
		for (y, animate_line) in (0..well.row_len()).zip(animate_line.iter()) {
			for x in 0..well.column_len() {
				if let Some(data) = well[(x,y)] {
					if !animate_line {
						self.draw_block(canvas, origin, &vec2i::new(x as i32, y as i32), &data);
					}else{
						self.draw_flash(canvas, origin, &vec2i::new(x as i32, y as i32));
					}
				}else {
					self.draw_block(canvas, origin, &vec2i::new(x as i32, y as i32), &Data::BACKGROUND);
				}
			}
		}
	}
}