// #![windows_subsystem = "windows"]

use sdl2::render::TextureQuery;
use sdl2::event::Event;
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use std::time::{Duration,Instant};
use std::thread::sleep;
use sdl2::pixels::Color;
use sdl2::rect::Rect;

use std::collections::VecDeque;
use std::mem::swap;
use std::cmp::{max,min};

use itertools::izip;

#[macro_use]
pub mod vec2;
pub mod mino;
pub mod block;
pub mod text_builder;
pub mod config;
pub mod lenio;
use vec2::vec2i;
use text_builder::TextBuilder;
use config::Config;

use std::net::TcpListener;
use std::net::TcpStream;

use lenio::LenIO;

use serde::{Serialize,Deserialize};
use bincode::{serialize,deserialize};

type Well = array2d::Array2D<Option<block::Data>>;

use mino::Mino;
use rand::{SeedableRng,Rng,rngs::SmallRng};

enum MinoRng {
	_Hard{rng: SmallRng},
	Fair{rng: SmallRng, stack: Vec<Mino>},
}

impl MinoRng {
	fn generate(&mut self) -> Mino {
		const MINO_CTORS: [fn() -> Mino; 7] =
			[Mino::l,Mino::j,Mino::o,Mino::z,Mino::s,Mino::t,Mino::i];
		match self {
			MinoRng::_Hard {ref mut rng} => {
				MINO_CTORS[rng.gen_range(0..7)]()
			}
			MinoRng::Fair {ref mut rng, ref mut stack} => {
				if stack.is_empty() {
					for i in 0..7 {
						stack.push(MINO_CTORS[i]());
					}
					for i in 0..6 {
						let j = i + rng.gen_range(0..7-i);
						stack.swap(i, j);
					}
				}
				stack.pop().unwrap()
			}
		}
	}
	fn generate_centered(&mut self, well: &Well) -> Mino {
		let mut mino = self.generate();
		center_mino(&mut mino, well);
		mino
	}
	fn fair() -> MinoRng {
		MinoRng::Fair {rng: SmallRng::from_entropy(), stack: Vec::with_capacity(7)}
	}
}

fn get_mino_rect(mino: &Mino) -> (vec2i,vec2i) {
	let mut iter = mino.blocks.iter();
	let mut hi = iter.next().unwrap().clone();
	let mut lo = hi;
	for v in iter {
		hi.x = max(hi.x, v.x);
		hi.y = max(hi.y, v.y);
		lo.x = min(lo.x, v.x);
		lo.y = min(lo.y, v.y);
	}
	(lo,hi)
}

fn get_mino_extents(mino: &Mino) -> vec2i {
	let (lo,hi) = get_mino_rect(mino);
	hi-lo+vec2i!(1,1)
}

fn center_mino(mino: &mut Mino, well: &Well) {
	let ext = get_mino_extents(mino);
	mino.translate(vec2i::RIGHT * (well.num_rows() as i32-ext.x)/2);
}

fn reset_mino(mino: &mut Mino) {
	for _ in 0..mino.rotation.rem_euclid(4) {
		mino.rotl();
	}
	let (lo,_) = get_mino_rect(mino);
	mino.translate(-lo);
}

fn check_block_in_bounds(block: &vec2i, dim: &vec2i) -> bool {
	block.x >= 0 && block.x < dim.x && block.y < dim.y
}

fn check_mino_well_collision(mino: &Mino, well: &Well) -> bool {
	let dim = vec2i::from((well.column_len(),well.row_len()));
	for block in mino.blocks.iter() {
		if block.y < 0 {continue;}
		if !check_block_in_bounds(block, &dim) {
			return true;
		}
		if well[(block.x as usize, block.y as usize)].is_some() {
			return true;
		}
	}
	false
}

fn mino_fits_in_well(mino: &Mino, well: &Well) -> bool {
	for block in mino.blocks.iter() {
		if block.y < 0 || well[(block.x as usize, block.y as usize)].is_some() {
			return false;
		}
	}
	true
}

fn add_mino_to_well(mino: &Mino, well: &mut Well) {
	for (block, data) in mino.blocks.iter().zip(mino.blocks_data.iter()) {
		assert!(block.y >= 0 && well[(block.x as usize, block.y as usize)].is_none());
		well[(block.x as usize, block.y as usize)] = Some(*data);
	}
}

fn mino_falling_system(
	falling_mino: &mut Mino,
	well: &Well,
	fall_countdown: &mut Duration,
	fall_duration: Duration,
	softdrop_duration: Duration,
	fall_state: &mut FallState)
-> (bool, bool) {
	let fall_duration = match fall_state {
		FallState::Fall => fall_duration,
		FallState::Softdrop => softdrop_duration,
		FallState::Harddrop => Duration::from_secs(0),
	};
	
	if FallState::Softdrop == *fall_state {
		*fall_countdown = std::cmp::min(*fall_countdown, softdrop_duration);
	}
	
	if FallState::Harddrop == *fall_state {
		*fall_state = FallState::Fall;
		*fall_countdown = Duration::from_secs(0);
	}
	
	let mut mino_translated = false;
	
	while *fall_countdown >= fall_duration {
		if try_down_mino(falling_mino, well) {
			mino_translated = true;
			*fall_countdown -= fall_duration;
		}else{
			return (true, mino_translated);
		}
	}
	
	(false, mino_translated)
}

fn try_mutate_mino<F>(mino: &mut Mino, well: &Well, f: F) -> bool where F: Fn(&mut Mino) {
	let mut mutated_mino = mino.clone();
	f(&mut mutated_mino);
	if !check_mino_well_collision(&mutated_mino, &well) {
		*mino = mutated_mino;
		return true;
	}
	false
}

fn try_rotl_mino(mino: &mut Mino, well: &Well) -> bool{
	try_mutate_mino(mino, well, |mino|mino.rotl())
}
fn try_rotr_mino(mino: &mut Mino, well: &Well) -> bool{
	try_mutate_mino(mino, well, |mino|mino.rotr())
}
fn try_left_mino(mino: &mut Mino, well: &Well) -> bool{
	try_mutate_mino(mino, well, |mino|mino.left())
}
fn try_right_mino(mino: &mut Mino, well: &Well) -> bool{
	try_mutate_mino(mino, well, |mino|mino.right())
}
fn try_down_mino(mino: &mut Mino, well: &Well) -> bool{
	try_mutate_mino(mino, well, |mino|mino.down())
}

fn try_clear_lines(well: &mut Well) {
	let mut dy: usize = 0;
	for y in (0..well.row_len()).rev() {
		let mut count = 0;
		for x in 0..well.column_len() {
			count += well[(x,y)].is_some() as usize;
			if dy != 0 {
				well[(x,y+dy)] = well[(x,y)];
				well[(x,y)] = None;
			}
		}
		if count == well.column_len() {
			dy += 1;
		}
	}
}

fn mark_clearable_lines(well: &Well, clearable: &mut Vec<bool>, clearable_count: &mut u32) {
	for (row,clearable) in (well.columns_iter()).zip(clearable.iter_mut()) {
		let mut count = 0;
		for block in row {
			count += block.is_some() as u32;
		}
		if count as usize == well.column_len() {
			*clearable_count += 1;
			*clearable = true;
		}
	}
}

fn create_shadow_mino(mino: &Mino, well: &Well) -> Mino {
	let mut shadow_mino = mino.clone();
	shadow_mino.make_shadow();
	while try_down_mino(&mut shadow_mino, &well) {}
	shadow_mino
}

enum RotDirection {
	None,
	Left,
	Right,
}

#[derive(PartialEq, Eq)]
enum MoveState {
	Still,
	Instant,
	Prepeat,
	Repeat,
}

#[derive(PartialEq, Eq, Debug)]
enum MoveDirection {
	None,
	Left,
	Right,
}

#[derive(PartialEq, Eq)]
enum FallState {
	Fall,
	Softdrop,
	Harddrop,
}

// #[derive(PartialEq, Eq, Clone)]
enum State {
	Play,
	Pause,
	Over,
	Start,
	LobbyHost,
	LobbyClient,
}

enum UnitState {
	Play,
	LineClear{countdown: Duration},
	Over,
	Win,
}

#[derive(PartialEq, Eq)]
enum Mode {
	Marathon{level: u32, level_target: u32, lines_before_next_level: i32},
	Sprint{lines_cleared_target: u32},
}

struct Unit {
	well: Well,
	animate_line: Vec<bool>,
	state: UnitState,
	queue: VecDeque<Mino>,
	rng: MinoRng,
	player: Player,
	
	falling_mino: Mino,
	
	lines_cleared: u32,
	mode: Mode,
	
	can_store_mino: bool,
	stored_mino: Option<Mino>,
}

impl Unit {
	fn new(mode: Mode, player: Player) -> Self {
		let mut rng = MinoRng::fair();
		let well = Well::filled_with(None, 10, 20);
	    Unit {
	    	animate_line: vec![false; 20],
	    	state: UnitState::Play,
	    	player,
	    	queue: {
	    		let mut queue = VecDeque::with_capacity(5);
	    		for _ in 0..5 {
	    			queue.push_back(rng.generate());
	    		}
	    		queue
	    	},
	    	falling_mino: rng.generate_centered(&well),
	    	// fall_duration: get_fall_duration(1),
	    	can_store_mino: true,
	    	stored_mino: None,
	    	well,
	    	rng,
	    	lines_cleared: 0,
	    	mode,
	    }
	}
}

impl Mode {
	fn default_marathon() -> Mode {
		Mode::Marathon{
			level_target: 11, level: 1,
			lines_before_next_level: get_lines_before_next_level(1),
		}
	}
	fn default_sprint() -> Mode {
		Mode::Sprint{lines_cleared_target: 40}
	}
}

fn create_lines_cleared_text<'a>(
	lines_cleared: u32,
	font: &sdl2::ttf::Font,
	texture_creator: &'a sdl2::render::TextureCreator<sdl2::video::WindowContext>)
	-> sdl2::render::Texture<'a> {
	TextBuilder::new(format!("Lines: {}", lines_cleared), Color::WHITE)
		.with_wrap(120)
		.build(font, texture_creator)
}

fn create_level_text<'a>(
	level: u32,
	font: &sdl2::ttf::Font,
	texture_creator: &'a sdl2::render::TextureCreator<sdl2::video::WindowContext>)
	-> sdl2::render::Texture<'a> {
	TextBuilder::new(format!("Level: {}", level), Color::WHITE)
		.with_wrap(120)
		.build(font, texture_creator)
}

fn create_score_text<'a>(
	score: u32,
	font: &sdl2::ttf::Font,
	texture_creator: &'a sdl2::render::TextureCreator<sdl2::video::WindowContext>)
	-> sdl2::render::Texture<'a> {
	TextBuilder::new(format!("Score: {}", score), Color::WHITE)
		.with_wrap(120)
		.build(font, texture_creator)
}

fn get_fall_duration(level: u32) -> Duration {
	let base: Duration = Duration::from_secs_f64(0.5);
	base.div_f64(1f64 + (level as f64 / 10f64))
}

fn get_lines_before_next_level(level: u32) -> i32 {
	10 * (level as i32)
}

enum Player {
	Local {
		move_direction: MoveDirection,
		move_state: MoveState,
		rot_direction: RotDirection,
		fall_state: FallState,
		
		store: bool,
		
		fall_countdown: Duration,
		move_repeat_countdown: Duration,
		
		fall_duration: Duration,
		
		joystick_id: Option<u32>,
		config_id: usize,
	},
	Network,
}

impl Player {
	fn local(config_id: usize, joystick_id: Option<u32>) -> Self {
	    Player::Local {
			move_direction: MoveDirection::None,
			move_state: MoveState::Still,
			rot_direction: RotDirection::None,
			fall_state: FallState::Fall,
			
			store: false,
			
			fall_countdown: Duration::from_secs(0),
			move_repeat_countdown: Duration::from_secs(0),
			
			fall_duration: get_fall_duration(1),
			
			joystick_id,
			config_id,
	    }
	}
	fn network() -> Self {
		Player::Network
	}
	fn update_local(&mut self, keybinds: &mut [config::Player;4], event: &Event) {
		if let Player::Local{
			move_direction,
			move_state,
			rot_direction,
			fall_state,
			store,
			joystick_id,
			config_id,
			..
		} = self {
			let keybinds = &mut keybinds[*config_id];
			
			if is_key_down(event, keybinds.left) ||
			is_key_down(event, keybinds.left_alt) ||
			is_controlcode_down(event, &mut keybinds.controller_left, *joystick_id) {
				*move_direction = MoveDirection::Left;
				*move_state = MoveState::Instant;
			}
			
			if is_key_down(event, keybinds.right) ||
			is_key_down(event, keybinds.right_alt) ||
			is_controlcode_down(event, &mut keybinds.controller_right, *joystick_id) {
				*move_direction = MoveDirection::Right;
				*move_state = MoveState::Instant;
			}
			
			if is_key_up(event, keybinds.left) ||
			is_key_up(event, keybinds.left_alt) ||
			is_controlcode_up(event, &mut keybinds.controller_left, *joystick_id) {
				if *move_direction == MoveDirection::Left {
					*move_direction = MoveDirection::None;
					*move_state = MoveState::Still;
				}
			}
			
			if is_key_up(event, keybinds.right) ||
			is_key_up(event, keybinds.right_alt) ||
			is_controlcode_up(event, &mut keybinds.controller_right, *joystick_id) {
				if *move_direction == MoveDirection::Right {
					*move_direction = MoveDirection::None;
					*move_state = MoveState::Still;
				}
			}
			
			if is_key_down(event, keybinds.rot_left) ||
			is_controlcode_down(event, &mut keybinds.controller_rot_left, *joystick_id) {
				*rot_direction = RotDirection::Left
			}
			
			if is_key_down(event, keybinds.rot_right) ||
			is_key_down(event, keybinds.rot_right_alt) ||
			is_controlcode_down(event, &mut keybinds.controller_rot_right, *joystick_id) {
				*rot_direction = RotDirection::Right
			}
			
			if is_key_down(event, keybinds.softdrop) ||
			is_key_down(event, keybinds.softdrop_alt) ||
			is_controlcode_down(event, &mut keybinds.controller_softdrop, *joystick_id) {
				*fall_state = FallState::Softdrop;
			}
			
			if is_key_up(event, keybinds.softdrop) ||
			is_key_up(event, keybinds.softdrop_alt) ||
			is_controlcode_up(event, &mut keybinds.controller_softdrop, *joystick_id) {
				*fall_state = FallState::Fall
			}
			
			if is_key_down(event, keybinds.harddrop) ||
			is_controlcode_down(event, &mut keybinds.controller_harddrop, *joystick_id) {
				*fall_state = FallState::Harddrop;
			}
			
			if is_key_down(event, keybinds.store) ||
			is_controlcode_down(event, &mut keybinds.controller_store, *joystick_id) {
				*store = true;
			}
		}
	}
}

fn is_key_down(event: &Event, key: Option<Keycode>) -> bool {
	if let Some(key) = key {
		if let Event::KeyDown{keycode: Some(event_key),repeat: false,..} = event {
			key == *event_key
		}else {
			false
		}
	}else {false}
}

fn is_key_up(event: &Event, key: Option<Keycode>) -> bool {
	if let Some(key) = key {
		if let Event::KeyUp{keycode: Some(event_key),repeat: false,..} = event {
			key == *event_key
		}else {
			false
		}
	}else {false}
}

fn is_controlcode_down(
	event: &Event,
	controlcode: &mut Option<config::Controlcode>,
	joystick_id: Option<u32>)
-> bool {
	if let Some(joystick_id) = joystick_id {
		if let Some(controlcode) = controlcode {
			match (controlcode, event) {
				(config::Controlcode::Button(button),
				Event::ControllerButtonDown{button: event_button,which,..})
				if joystick_id == *which => {
					button == event_button
				}
				
				(config::Controlcode::Axis(axis, ref mut down),
				Event::ControllerAxisMotion{axis:event_axis,value,which,..})
				if joystick_id == *which && axis == event_axis => {
					if !*down && *value >= 4096i16 {
						*down = true;
						true
					}else if *down && *value < 4096 {
						*down = false;
						false
					}else {false}
				}
				
				(_,_) => false
			}
		}else {false}
	}else {false}
}

fn is_controlcode_up(
	event: &Event,
	controlcode: &mut Option<config::Controlcode>,
	joystick_id: Option<u32>)
-> bool {
	if let Some(joystick_id) = joystick_id {
		if let Some(controlcode) = controlcode {
			match (controlcode, event) {
				(config::Controlcode::Button(button),
				Event::ControllerButtonUp{button: event_button,which,..})
				if joystick_id == *which => {
					button == event_button
				}
				
				(config::Controlcode::Axis(axis, ref mut down),
				Event::ControllerAxisMotion{axis:event_axis,value,which,..})
				if joystick_id == *which && axis == event_axis => {
					if !*down && *value >= 4096i16 {
						*down = true;
						false
					}else if *down && *value < 4096 {
						*down = false;
						true
					}else {false}
				}
				
				(_,_) => false
			}
		}else {false}
	}else {false}
}

#[derive(Default)]
struct Layout {
	x: i32,
	y: i32,
	width: i32,
	expected_width: i32,
}

impl Layout {
	fn centered_x(&self) -> i32 {
		((self.width-self.expected_width) / 2) as i32
	}
	fn x(&self) -> i32 {
		return self.centered_x()+self.x;
	}
	fn y(&self) -> i32 {
		return self.y;
	}
	fn as_vec2i(&self) -> vec2i {
		vec2i!(self.x(),self.y())
	}
	fn row(&mut self, y: i32) {
		self.y += y;
	}
	fn row_margin(&mut self, y: i32) {
		self.y += y;
	}
	fn col(&mut self, x: i32) {
		self.y = 0;
		self.x += x;
	}
	fn col_margin(&mut self, x: i32) {
		self.y = 0;
		self.x += x;
	}
}
struct StartLayout {
	x: i32,
	y: i32,
	width: u32
}

use vec2::vec2f;

#[derive(Debug, Serialize, Deserialize)]
enum NetworkEvent {
	UnitEvent {
		unit_id: usize,
		event: UnitEvent,
	},
	InitPlayers {
		count: usize,
	},
	AddPlayer,
	StartGame,
}

#[derive(Debug, Serialize, Deserialize)]
enum UnitEvent {
	TranslateMino {
		origin: vec2f,
		blocks: [vec2i; 4],
	},
	AddMinoToWell,
	GenerateMino {
		mino: Mino,
	},
	StoreMino {
		generated_mino: Option<Mino>,
	},
}

impl StartLayout {
	fn centered_x(&self, obj_width: u32) -> i32 {
		((self.width-obj_width) / 2) as i32
	}
	fn row(&mut self, y: i32) {
		self.y += y;
	}
	fn row_margin(&mut self, y: i32) {
		self.y += y;
	}
}

fn main() {
	let sdl_context = sdl2::init()
		.expect("Failed to initialize sdl2");
	let video_subsystem = sdl_context.video()
		.expect("Failed to initialize video subsystem");
	let ttf_context = sdl2::ttf::init()
		.expect("Failed to initialize ttf");
	let game_controller_subsystem = sdl_context.game_controller()
		.expect("Failed to initialize controller subsystem");
	
	let available = game_controller_subsystem
		.num_joysticks()
		.expect("Failed to enumerate joysticks");
	
	let mut controllers = Vec::new();
	
	for i in 0..available {
		if let Ok(controller) = game_controller_subsystem.open(i) {
			controllers.push(controller);
		}
	}
	
	let mut config = Config::from_file();
	let mut configs = (0..4usize).cycle();
	
	let window_rect = video_subsystem.display_bounds(0).unwrap();
	
	let window = video_subsystem.window(
			"Tetris part 3",
			window_rect.width(),
			window_rect.height())
		.position_centered()
		.borderless()
		.build()
		.expect("Failed to create window");
	
	let mut canvas = window.into_canvas().build()
		.expect("Failed to create canvas");
	canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
	
	let mut event_pump = sdl_context.event_pump()
		.expect("Failed to create event pump");
	
	let texture_creator = canvas.texture_creator();
	let texture = texture_creator.load_texture("gfx/block.png")
		.expect("Failed to load block texture");
	
	let font = ttf_context.load_font("gfx/IBMPlexMono-Regular.otf", 32)
		.expect("Failed to load font");
	
	let title = texture_creator.load_texture("gfx/title.png").unwrap();
	
	let paused_text = TextBuilder::new("Paused".to_string(), Color::WHITE)
		.build(&font, &texture_creator);
	
	let game_over_text = TextBuilder::new("Game over press r to restart".to_string(), Color::WHITE)
		.with_wrap(10*30)
		.build(&font, &texture_creator);
	
	let host_start_text = TextBuilder::new("Press enter to start game".to_string(), Color::WHITE)
		.with_wrap(window_rect.width() as u32)
		.build(&font, &texture_creator);
	
	let local_player_text = TextBuilder::new("Local player".to_string(), Color::WHITE)
		.build(&font, &texture_creator);
	let network_player_text = TextBuilder::new("Network player".to_string(), Color::WHITE)
		.build(&font, &texture_creator);
	
	let get_player_text = |player: &Player| -> &sdl2::render::Texture{
		match player {
			Player::Local{..} => &local_player_text,
			Player::Network => &network_player_text,
		}
	};
	
	let fps: u32 = 60;
	let dpf: Duration = Duration::from_secs(1) / fps;
	
	let mut chosen_game_mode: usize = 0;
	let game_mode_text = [
		TextBuilder::new("Marathon".to_string(),Color::WHITE).build(&font, &texture_creator),
		TextBuilder::new("Sprint".to_string(), Color::WHITE).build(&font, &texture_creator),
	];
	
	#[derive(Debug)]
	enum NetworkState {
		Offline,
		Host {
			listener: TcpListener,
			streams: Vec<LenIO<TcpStream>>,
		},
		Client {
			stream: LenIO<TcpStream>,
		},
	}
	
	impl NetworkState {
		fn broadcast_event(&mut self, event: &NetworkEvent) {
			use NetworkState::*;
			match self {
				Offline => {},
				Host {streams,..} => {
					let event = &serialize(event).unwrap();
					for stream in streams {
						stream.write(event).unwrap();
					}
				}
				Client {stream} => {
					stream.write(&serialize(event).unwrap()).unwrap();
				}
			}
		}
	}
	
	let mut network_states = (0..3i32).cycle();
	let mut selected_network_state = network_states.next().unwrap();
	let mut network_state = NetworkState::Offline;
	
	let offline_text = TextBuilder::new("Offline".to_string(), Color::WHITE).build(&font, &texture_creator);
	let host_text = TextBuilder::new("Host".to_string(),Color::WHITE).build(&font, &texture_creator);
	let client_text = TextBuilder::new("Client".to_string(),Color::WHITE).build(&font, &texture_creator);
	
	let mut lines_cleared_text = [
		create_lines_cleared_text(0, &font, &texture_creator),
		create_lines_cleared_text(0, &font, &texture_creator),
		create_lines_cleared_text(0, &font, &texture_creator),
		create_lines_cleared_text(0, &font, &texture_creator),
	];
	
	let mut level_text = [
		create_level_text(1, &font, &texture_creator),
		create_level_text(1, &font, &texture_creator),
		create_level_text(1, &font, &texture_creator),
		create_level_text(1, &font, &texture_creator),
	];
	
	let mut _score = 0;
	let mut _score_text =
		create_score_text(_score, &font, &texture_creator);
	
	let softdrop_duration = Duration::from_secs_f64(0.05);
	
	let move_prepeat_duration = Duration::from_secs_f64(0.15);
	let move_repeat_duration = Duration::from_secs_f64(0.05);
	
	let mut players = Vec::<Player>::new();
	
	let mut units: Vec<Unit> = Vec::new();
	
	let block_canvas = block::Canvas::new(&texture);
	
	let mut state = State::Start;
	let mut prev_state: Option<State> = None;
	
	let start_text = TextBuilder::new(
			"Welcome to tetris! Please choose a game mode, and press enter.".to_string(), 
			Color::WHITE)
		.with_wrap(15 + 4*30 + 15 + 10*30 + 15 + 4*30 + 15)
		.build(&font, &texture_creator);
	let game_mode_ctors = [
		Mode::default_marathon,
		Mode::default_sprint,
	];
	
	let line_clear_duration = Duration::from_secs_f64(0.1);
	
	let mut stopwatch = Duration::from_secs(0);
	'running: loop {
		let start = Instant::now();
		
		// @input
		for event in event_pump.poll_iter() {
			match event {
				Event::Quit{..} => break 'running,
				_ => (),
			};
			
			match state {
				State::Play | State::Pause => {
					if let State::Play = state {
						for Unit{player,..} in units.iter_mut() {
							player.update_local(&mut config.players, &event)
						}
					}
					match event{
						// Not adding restart keybind for now
						Event::KeyDown{keycode: Some(Keycode::R),repeat: false,..}
							=> if let State::Pause | State::Over = state {
							lines_cleared_text = [
								create_lines_cleared_text(0, &font, &texture_creator),
								create_lines_cleared_text(0, &font, &texture_creator),
								create_lines_cleared_text(0, &font, &texture_creator),
								create_lines_cleared_text(0, &font, &texture_creator),
							];
							
			
							level_text = [
								create_level_text(1, &font, &texture_creator),
								create_level_text(1, &font, &texture_creator),
								create_level_text(1, &font, &texture_creator),
								create_level_text(1, &font, &texture_creator),
							];
							
							units.clear();
							for player in players.drain(..) {
								units.push(Unit::new(game_mode_ctors[chosen_game_mode](), player));
							}
							
							state = State::Play;
							prev_state = None;
							
							stopwatch = Duration::from_secs(0);
						}
						
						// Deliberately not adding custom pause keybind
						Event::KeyDown{keycode: Some(Keycode::Escape),repeat: false,..} |
						Event::ControllerButtonDown{button: sdl2::controller::Button::Start,..}
							=> match state {
								State::Pause => {state = prev_state.unwrap_or(State::Play); prev_state = None;},
								State::Over => (),
								other => {state = State::Pause;prev_state = Some(other);},
							}
						
						_ => ()
					};
				}
				State::Start => {
					let keybinds = &mut config.players[0];
					
					if is_key_down(&event, keybinds.left) ||
					is_key_down(&event, keybinds.left_alt) ||
					is_controlcode_down(&event, &mut keybinds.controller_left, None) {
						chosen_game_mode = (chosen_game_mode as i32 - 1).rem_euclid(2) as usize;
					}
					
					if is_key_down(&event, keybinds.right) ||
					is_key_down(&event, keybinds.right_alt) ||
					is_controlcode_down(&event, &mut keybinds.controller_right, None) {
						chosen_game_mode = (chosen_game_mode + 1).rem_euclid(2);
					}
					
					if is_key_down(&event, Some(Keycode::Q)) {
						selected_network_state = network_states.next().unwrap();
					}
					
					if is_key_down(&event, Some(Keycode::Return)) {
						network_state = match selected_network_state {
							0 => {
								state = State::Play;
								units.push(Unit::new(game_mode_ctors[chosen_game_mode](), Player::local(configs.next().unwrap(),None)));
								NetworkState::Offline
							}
							1 => {
								let listener = TcpListener::bind("127.0.0.1:4141")
									.expect("Couldn't bind listener");
								listener.set_nonblocking(true)
									.expect("Couldn't set listener to be non-blocking");
								
								state = State::LobbyHost;
								
								NetworkState::Host {
									listener,
									streams: Vec::new(),
								}
							}
							2 => {
								let stream = TcpStream::connect("127.0.0.1:4141")
									.expect("Couldn't connect stream");
								stream.set_nonblocking(true)
									.expect("Couldn't set stream to be non-blocking");
								let mut stream = LenIO::new(stream);
								
								state = State::LobbyClient;
								units.push(Unit::new(game_mode_ctors[chosen_game_mode](), Player::local(configs.next().unwrap(),None)));
								stream.write(&serialize(&NetworkEvent::AddPlayer).unwrap()).unwrap();
								
								NetworkState::Client {
									stream,
								}
							}
							_ => {panic!()}
						}
					}
				}
				State::LobbyHost => {
					if is_key_down(&event, Some(Keycode::Return)) {
						network_state.broadcast_event(&NetworkEvent::StartGame);
						state = State::Play;
					}
					if is_key_down(&event, Some(Keycode::Q)) {
						units.push(Unit::new(Mode::default_marathon(), Player::local(configs.next().unwrap(),None)));
						if let NetworkState::Host{streams,..} = &mut network_state {
							let event = serialize(&NetworkEvent::AddPlayer).unwrap();
							for stream in streams {
								stream.write(&event).unwrap();
							}
						}
					}
				}
				_ => {}
			}
		}
		
		// @network
		match network_state {
			NetworkState::Offline => {}
			NetworkState::Host {ref mut streams,..} => {
				for i in 0..streams.len() {
					while let Ok(Ok(event)) = streams[i].read().map(deserialize::<NetworkEvent>) {
						for j in 0..streams.len() {
							if i == j {continue;}
							streams[j].write(&serialize(&event).unwrap()).unwrap();
						}
						match event {
							NetworkEvent::UnitEvent {unit_id, event} => {
								let Unit{falling_mino,player,well,can_store_mino,state,lines_cleared,animate_line,stored_mino,..} = &mut units[unit_id];
								match event {
									UnitEvent::TranslateMino {origin, blocks} => {
										falling_mino.origin = origin;
										falling_mino.blocks = blocks;
									}
									UnitEvent::AddMinoToWell => {
										let can_add = mino_fits_in_well(&falling_mino, &well);
										if !can_add {
											*state = UnitState::Over;
										}else{
											*can_store_mino = true;
											add_mino_to_well(&falling_mino, well);
											
											let mut clearable_lines = 0;
											mark_clearable_lines(&well, animate_line, &mut clearable_lines);
											
											if clearable_lines != 0 {
												*state = UnitState::LineClear{countdown: Duration::from_secs(0)};
												
												*lines_cleared += clearable_lines;
												
											}
										}
									}
									UnitEvent::GenerateMino {mino} => {
										*falling_mino = mino;
										center_mino(falling_mino, &well);
									}
									UnitEvent::StoreMino {generated_mino} => {
										if *can_store_mino {
											*can_store_mino = false;
											reset_mino(falling_mino);
											if let Some(stored_mino) = stored_mino {
												swap(stored_mino, falling_mino);
											}else if let Some(mut generated_mino) = generated_mino{
												swap(&mut generated_mino, falling_mino);
												*stored_mino = Some(generated_mino);
											}
											center_mino(falling_mino, &well);
										}
									}
								}
							}
							NetworkEvent::AddPlayer => {units.push(Unit::new(Mode::default_marathon(),Player::network()))}
							NetworkEvent::StartGame => {} //only host gets to start game
							NetworkEvent::InitPlayers {..} => {} //host already has players initted
						}
					}
				}
			}
			NetworkState::Client {ref mut stream} => {
				while let Ok(Ok(event)) = stream.read().map(deserialize::<NetworkEvent>) {
					match event {
						NetworkEvent::UnitEvent {unit_id, event} => {
							let Unit{falling_mino,player,well,can_store_mino,state,lines_cleared,animate_line,stored_mino,..} = &mut units[unit_id];
							match event {
								UnitEvent::TranslateMino {origin, blocks} => {
									falling_mino.origin = origin;
									falling_mino.blocks = blocks;
								}
								UnitEvent::AddMinoToWell => {
									let can_add = mino_fits_in_well(&falling_mino, &well);
									if !can_add {
										*state = UnitState::Over;
									}else{
										*can_store_mino = true;
										add_mino_to_well(&falling_mino, well);
										
										let mut clearable_lines = 0;
										mark_clearable_lines(&well, animate_line, &mut clearable_lines);
										
										if clearable_lines != 0 {
											*state = UnitState::LineClear{countdown: Duration::from_secs(0)};
											
											*lines_cleared += clearable_lines;
											
										}
									}
								}
								UnitEvent::GenerateMino {mino} => {
									*falling_mino = mino;
									center_mino(falling_mino, &well);
								}
								UnitEvent::StoreMino {generated_mino} => {
									if *can_store_mino {
										*can_store_mino = false;
										reset_mino(falling_mino);
										if let Some(stored_mino) = stored_mino {
											swap(stored_mino, falling_mino);
										}else if let Some(mut generated_mino) = generated_mino{
											swap(&mut generated_mino, falling_mino);
											*stored_mino = Some(generated_mino);
										}
										center_mino(falling_mino, &well);
									}
								}
							}
						}
						NetworkEvent::AddPlayer => {units.push(Unit::new(Mode::default_marathon(),Player::network()))}
						NetworkEvent::StartGame => {state = State::Play}
						NetworkEvent::InitPlayers {count} => {
							let mut inited_units = Vec::new();
							for _ in 0..count {
								inited_units.push(Unit::new(Mode::default_marathon(), Player::network()));
							}
							inited_units.append(&mut units);
							units = inited_units;
						}
					}
				}
			}
		}
		
		// @update
		match state {
			State::Play => {
				for (unit_id,Unit{well,state,queue,rng,animate_line,player,lines_cleared,mode,
					stored_mino, can_store_mino, falling_mino},lines_cleared_text,level_text) in
					izip!(0usize..,units.iter_mut(),lines_cleared_text.iter_mut(),level_text.iter_mut()) {
					match state {
						UnitState::Play => {
							
							match player {
								Player::Local{store,fall_countdown,rot_direction,move_direction,move_state,move_repeat_countdown,
									fall_duration,fall_state,..} => {
							
									if *store && *can_store_mino {
										
										*can_store_mino = false;
										*store = false;
										*fall_countdown = Duration::from_secs(0);
										reset_mino(falling_mino);
										if let Some(stored_mino) = stored_mino {
											swap(stored_mino, falling_mino);
											network_state.broadcast_event(
												&NetworkEvent::UnitEvent{unit_id,event:UnitEvent::StoreMino{generated_mino:None}}
											);
										}else{
											let mut next_mino = queue.pop_front().unwrap();
											swap(&mut next_mino, falling_mino);
											*stored_mino = Some(next_mino);
											queue.push_back(rng.generate());
											network_state.broadcast_event(
												&NetworkEvent::UnitEvent{unit_id,event:UnitEvent::StoreMino{generated_mino:Some(falling_mino.clone())}}
											);
										}
										center_mino(falling_mino, &well);
									}
									
									let mut mino_translated = false;
									
									mino_translated |= match rot_direction {
										RotDirection::Left => try_rotl_mino(falling_mino, &well),
										RotDirection::Right => try_rotr_mino(falling_mino, &well),
										RotDirection::None => false,
									};
									*rot_direction = RotDirection::None;
									
									if MoveState::Instant == *move_state {
										mino_translated |= match move_direction{
											MoveDirection::Left => try_left_mino(falling_mino, &well),
											MoveDirection::Right => try_right_mino(falling_mino, &well),
											_ => false, // oh no
										};
										*move_repeat_countdown = Duration::from_secs(0);
										*move_state = MoveState::Prepeat;
									}
									if MoveState::Prepeat == *move_state {
										if *move_repeat_countdown >= move_prepeat_duration {
											*move_repeat_countdown -= move_prepeat_duration;
											mino_translated |= match move_direction{
												MoveDirection::Left => try_left_mino(falling_mino, &well),
												MoveDirection::Right => try_right_mino(falling_mino, &well),
												_ => false, // oh no
											};
											*move_state = MoveState::Repeat;
										}
									}
									if MoveState::Repeat == *move_state {
										while *move_repeat_countdown >= move_repeat_duration {
											*move_repeat_countdown -= move_repeat_duration;
											mino_translated |= match move_direction{
												MoveDirection::Left => try_left_mino(falling_mino, &well),
												MoveDirection::Right => try_right_mino(falling_mino, &well),
												_ => false, // oh no
											};
										}
									}
								
									let (add_mino, mino_translated_while_falling) = mino_falling_system(
										falling_mino, &well,
										fall_countdown,
										*fall_duration, softdrop_duration,
										fall_state);
									
									mino_translated |= mino_translated_while_falling;
									
									if mino_translated {
										network_state.broadcast_event(
											&NetworkEvent::UnitEvent{unit_id,event:UnitEvent::TranslateMino{
												origin: falling_mino.origin,
												blocks: falling_mino.blocks,
											}}
										);
									}
									
									if add_mino {
										
										network_state.broadcast_event(
											&NetworkEvent::UnitEvent{unit_id,event:UnitEvent::AddMinoToWell}
										);
										
										let can_add = mino_fits_in_well(&falling_mino, &well);
										if !can_add {
											*state = UnitState::Over;
										}else{
											*can_store_mino = true;
											add_mino_to_well(&falling_mino, well);
											*fall_countdown = Duration::from_secs(0);
											
											let mut clearable_lines = 0;
											mark_clearable_lines(&well, animate_line, &mut clearable_lines);
											
											if clearable_lines != 0 {
												*state = UnitState::LineClear{countdown: Duration::from_secs(0)};
												
												*lines_cleared += clearable_lines;
												*lines_cleared_text =
													create_lines_cleared_text(*lines_cleared, &font, &texture_creator);
												
												if let Mode::Marathon{level,lines_before_next_level,..} = mode {
													*lines_before_next_level -= clearable_lines as i32;
													let level_changed = *lines_before_next_level <= 0;
													while *lines_before_next_level <= 0 {
														*level += 1;
														*lines_before_next_level +=
															get_lines_before_next_level(*level) as i32;
													}
												
													if level_changed {
														*level_text =
															create_level_text(*level, &font, &texture_creator);
														*fall_duration = get_fall_duration(*level);
													}
												}
											}
											
											*falling_mino = queue.pop_front().unwrap();
											network_state.broadcast_event(
												&NetworkEvent::UnitEvent{unit_id,event:UnitEvent::GenerateMino{
													mino: falling_mino.clone(),
												}}
											);
											center_mino(falling_mino, &well);
											
											queue.push_back(rng.generate());
										}
									}
									
									*fall_countdown += dpf;
									if MoveState::Still != *move_state {
										*move_repeat_countdown += dpf;
									}
								}
								Player::Network => {}
							}
						}
						
						UnitState::LineClear{countdown} => {
							*countdown += dpf;
							if *countdown >= line_clear_duration {
								*state = UnitState::Play;
								for line in animate_line.iter_mut() {
									*line = false;
								}
								try_clear_lines(well);
								
								match mode {
									Mode::Marathon{level,level_target,..} => {
										if *level >= *level_target {
											let _won_text =
												TextBuilder::new(
													format!("You won! Press r to restart.").to_string(),
													Color::WHITE)
												.with_wrap(15 + 4*30 + 15 + 10*30 + 15 + 4*30 + 15)
												.build(&font, &texture_creator);
											*state = UnitState::Win;
										}
									}
									Mode::Sprint{lines_cleared_target} => {
										if *lines_cleared >= *lines_cleared_target {
											let _won_text =
												TextBuilder::new(
													format!("You won in {:.2} seconds! Press r to restart.", stopwatch.as_secs_f64()).to_string(),
													Color::WHITE)
												.with_wrap(15 + 4*30 + 15 + 10*30 + 15 + 4*30 + 15)
												.build(&font, &texture_creator);
											*state = UnitState::Win;
										}
									}
								}
							}
						}
						
						UnitState::Over => (),
						UnitState::Win => ()
					}
				}
				
				stopwatch += dpf;
			}
			
			State::LobbyHost => {
				if let NetworkState::Host {listener, streams} = &mut network_state {
					while let Ok(incoming) = listener.accept() {
						let mut stream = LenIO::new(incoming.0);
						stream.write(&serialize(&NetworkEvent::InitPlayers{count:units.len()}).unwrap()).unwrap();
						streams.push(stream);
						println!("{:?}", incoming.1);
						println!("Connection established");
					}
				}else {
					panic!();
				}
			}
			
			State::LobbyClient => {
				
			}
			
			_ => ()
			
		}
		
		
		// @draw
		
		canvas.set_draw_color(Color::BLACK);
		canvas.clear();
		if let State::Start = state {
			
			let mut layout = StartLayout {x:0,y:0,width:window_rect.width()};
			
			layout.row_margin(15);
			
			let TextureQuery {width, height, ..} = title.query();
			let _ = canvas.copy(
				&title,
				Rect::new(0, 0, width, height),
				Rect::new(layout.centered_x(width), layout.y, width, height));
			
			layout.row(height as i32);
			layout.row_margin(15);
			
			let TextureQuery {width, height, ..} = start_text.query();
			let _ = canvas.copy(
				&start_text,
				Rect::new(0, 0, width, height),
				Rect::new(layout.centered_x(width), layout.y, width, height));
			
			layout.row(height as i32);
			layout.row_margin(15);
			
			let game_mode_text =
				&game_mode_text[chosen_game_mode];
			let TextureQuery {width, height, ..} = game_mode_text.query();
			let _ = canvas.copy(
				&game_mode_text,
				Rect::new(0, 0, width, height),
				Rect::new(layout.centered_x(width), layout.y, width, height));
			
			layout.row(height as i32);
			layout.row_margin(15);
			
			let network_text = match selected_network_state {
				0 => &offline_text,
				1 => &host_text,
				2 => &client_text,
				_ => panic!(),
			};
			let TextureQuery {width, height, ..} = network_text.query();
			let _ = canvas.copy(
				&network_text,
				Rect::new(0, 0, width, height),
				Rect::new(layout.centered_x(width), layout.y, width, height));
			
		}else if let State::LobbyHost {..} = state {
			
			let TextureQuery {width, height, ..} = host_start_text.query();
			let _ = canvas.copy(
				&host_start_text,
				Rect::new(0, 0, width, height),
				Rect::new(0, 0, width, height));
			
			for (i, Unit{player,..}) in (0..).zip(&units) {
				let player_text = get_player_text(player);
				let TextureQuery {width, height, ..} = player_text.query();
				let _ = canvas.copy(
					&player_text,
					Rect::new(0, 0, width, height),
					Rect::new(0, 30+i*(32+8), width, height));
			}
			
		}else if let State::LobbyClient {..} = state {
			for (i, Unit{player,..}) in (0..).zip(&units) {
				let player_text = get_player_text(player);
				let TextureQuery {width, height, ..} = player_text.query();
				let _ = canvas.copy(
					&player_text,
					Rect::new(0, 0, width, height),
					Rect::new(0, 30+i*(32+8), width, height));
			}
		}else{
			
			let mut layout = Layout {
				x:0,y:0,
				width:window_rect.width() as i32,expected_width:(4*30+15+10*30+15+4*30+15) * units.len() as i32 - 15
			};
			
			for ((Unit{well,queue,animate_line,stored_mino,state,falling_mino,..},lines_cleared_text),level_text)
			in units.iter().zip(lines_cleared_text.iter()).zip(level_text.iter_mut()) {
				layout.row_margin(15);
				
				if let Some(ref stored_mino) = stored_mino {
					block_canvas.draw_mino(&mut canvas, layout.as_vec2i(), stored_mino);
				}
				layout.row(3*30);
				layout.row_margin(15);
				
				let TextureQuery {width, height, ..} = lines_cleared_text.query();
				let _ = canvas.copy(
					&lines_cleared_text,
					Rect::new(0, 0, width, height),
					Rect::new(layout.x(), layout.y(), width, height));
				
				layout.row(32*2);
				layout.row_margin(15);
				
				let TextureQuery {width, height, ..} = level_text.query();
				let _ = canvas.copy(
					&level_text,
					Rect::new(0, 0, width, height),
					Rect::new(layout.x(), layout.y(), width, height));
				
				layout.col(4*30);
				layout.col_margin(15);
				
				layout.row_margin(15);
				
				block_canvas.draw_well(&mut canvas, layout.as_vec2i(), &well, animate_line);
				let shadow_mino = create_shadow_mino(falling_mino, &well);
				block_canvas.draw_mino(&mut canvas, layout.as_vec2i(), &shadow_mino);
				block_canvas.draw_mino(&mut canvas, layout.as_vec2i(), falling_mino);
				
				layout.col(10*30);
				layout.col_margin(15);
				
				layout.row_margin(15);
				for mino in queue.iter() {
					block_canvas.draw_mino(&mut canvas, layout.as_vec2i(), mino);
					layout.row(3*30);
					layout.row_margin(15);
				}
				
				layout.col(4*30);
				layout.col_margin(15);
				
				if let UnitState::Win = state {
					
				}
			}
			
			match state {
				State::Pause => {
					canvas.set_draw_color(Color::RGBA(0,0,0,160));
					let _ = canvas.fill_rect(None);
					
					let TextureQuery {width, height, ..} = paused_text.query();
					let _ = canvas.copy(
						&paused_text,
						Rect::new(0, 0, width, height),
						Rect::new(((window_rect.width()-width)/2) as i32, ((window_rect.height()-height)/2) as i32, width, height));
				}
				State::Over => {
					canvas.set_draw_color(Color::RGBA(0,0,0,160));
					let _ = canvas.fill_rect(None);
					
					let TextureQuery {width, height, ..} = game_over_text.query();
					let _ = canvas.copy(
						&game_over_text,
						Rect::new(0, 0, width, height),
						Rect::new(0, 0, width, height));
				}
				// State::Won{ref won_text} => {
				// 	canvas.set_draw_color(Color::RGBA(0,0,0,160));
				// 	let _ = canvas.fill_rect(None);
					
				// 	let TextureQuery {width, height, ..} = won_text.query();
				// 	let _ = canvas.copy(
				// 		&won_text,
				// 		Rect::new(0, 0, width, height),
				// 		Rect::new(0, 0, width, height));
				// }
				_ => ()
			}
			
		}
		
		canvas.present();
		
		
		// TIMEKEEPING
		let duration = start.elapsed();
		let difference = match dpf.checked_sub(duration) {
			Some(difference) => difference,
			None => Duration::from_secs(0),
		};
		
		sleep(difference);
	}
}