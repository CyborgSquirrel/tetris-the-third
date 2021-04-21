use sdl2::{render::{Texture, TextureCreator}, ttf::Font, video::WindowContext};
use crate::Color;

pub struct TextCreator<'a, 'b> {
	texture_creator: &'a TextureCreator<WindowContext>,
	font: &'b Font<'b, 'b>,
	big_font: &'b Font<'b, 'b>,
}

impl<'a, 'b> TextCreator<'a, 'b> {
	pub fn new(
		texture_creator: &'a TextureCreator<WindowContext>,
		font: &'b Font<'b,'b>, big_font: &'b Font<'b,'b>) -> Self {
		Self {
			texture_creator,
			font,
			big_font,
		}
	}
	pub fn builder<'c>(&'a self, text: &'c str) -> TextBuilder<'a, 'b, 'c> {
		TextBuilder::new(self, text)
	}
}

#[derive(Clone)]
pub struct TextBuilder<'a, 'b, 'c> {
	text_creator: &'a TextCreator<'a, 'b>,
	text: &'c str,
	color: Color,
	wrap_max_width: Option<u32>,
	big: bool,
}

impl<'a, 'b, 'c> TextBuilder<'a, 'b, 'c> {
	pub fn new(text_creator: &'a TextCreator<'a, 'b>, text: &'c str) -> Self {
		TextBuilder {
			text_creator,
			text,
			color: Color::WHITE,
			wrap_max_width: None,
			big: false,
		}
	}
	pub fn color(mut self, color: Color) -> Self {
		self.color = color;
		self
	}
	pub fn with_wrap(mut self, wrap_max_width: u32) -> Self {
		self.wrap_max_width = Some(wrap_max_width);
		self
	}
	pub fn big(mut self) -> Self {
		self.big = true;
		self
	}
	pub fn build(self: TextBuilder<'a, 'b, 'c>) -> Texture<'a> {
		let TextBuilder{text_creator: TextCreator{texture_creator, font, big_font},
		text, color, wrap_max_width, big} = self;
		
		// I do this because sdl2 outputs an error if you try to render text from an
		// empty string.
		let text = if text.is_empty() {" "} else {text};
		
		let font = if big {big_font} else {font};
		
		let surface = font.render(text);
		
		let surface = if let Some(wrap_max_width) = wrap_max_width {
			surface.blended_wrapped(color, wrap_max_width)
		}else{
			surface.blended(color)
		};
		
		let surface = surface.unwrap();
		
		let texture = texture_creator
			.create_texture_from_surface(surface)
			.unwrap();
		
		texture
	}
}