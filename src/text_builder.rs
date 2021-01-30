use crate::Color;

pub struct TextBuilder {
	text: String,
	color: Color,
	wrap_max_width: Option<u32>,
}

impl TextBuilder {
	pub fn new(text: String, color: Color) -> Self {
		Self {
			text,
			color,
			wrap_max_width: None,
		}
	}
	pub fn with_wrap(mut self, wrap_max_width: u32) -> Self {
		self.wrap_max_width = Some(wrap_max_width);
		self
	}
	pub fn build<'a>(
		self, 
		font: &sdl2::ttf::Font,
		texture_creator: &'a sdl2::render::TextureCreator<sdl2::video::WindowContext>)
		-> sdl2::render::Texture<'a> {
		let surface = font
			.render(self.text.as_str());
		
		let surface = if let Some(wrap_max_width) = self.wrap_max_width {
			surface.blended_wrapped(self.color, wrap_max_width)
		}else{
			surface.blended(self.color)
		};
		
		let surface = surface.unwrap();
		
		let texture = texture_creator
			.create_texture_from_surface(surface)
			.unwrap();
		texture
	}
}