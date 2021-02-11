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

use itertools::izip;
use rand::RngCore;

#[macro_use]
pub mod vec2;
pub mod mino;
pub mod block;
pub mod text_builder;
pub mod config;
pub mod lenio;
pub mod game;
pub mod util;
pub mod player;
use util::*;
use vec2::vec2i;
use text_builder::TextBuilder;
use config::Config;

use std::net::TcpListener;
use std::net::TcpStream;
use std::net::ToSocketAddrs;

use lenio::LenIO;

use serde::{Serialize,Deserialize};
use bincode::{serialize,deserialize};
use std::io::stdin;

use mino::Mino;
use rand::{SeedableRng,rngs::SmallRng};

use player::Player;


// #[derive(PartialEq, Eq, Clone)]
enum State {
	Play,
	Pause,
	Over,
	Start,
	Lobby,
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
	well: game::Well,
	animate_line: Vec<bool>,
	state: UnitState,
	
	lines_cleared: u32,
	mode: Mode,
	
	falling_mino: Option<Mino>,
	can_store_mino: bool,
	stored_mino: Option<Mino>,
	
	kind: UnitKind
}

enum UnitKind {
	Local {
		queue: VecDeque<Mino>,
		rng: game::MinoRng,
		player: player::Player,
	},
	Network {
		rng_queue: VecDeque<Mino>,
	}
}

impl UnitKind {
	fn next_mino(&mut self, network_state: Option<&mut NetworkState>, unit_id: usize) -> Mino {
		match self {
			UnitKind::Local {queue, rng, ..} => {
				let mino = queue.pop_front().unwrap();
				queue.push_back(rng.generate());
				let network_state = network_state.unwrap();
				network_state.broadcast_event(
					&NetworkEvent::UnitEvent {
						unit_id,
						event: UnitEvent::GenerateMino {mino: mino.clone()}
					}
				);
				mino
			}
			UnitKind::Network {rng_queue} => {
				rng_queue.pop_front().unwrap()
			}
		}
	}
	fn next_mino_centered(&mut self, network_state: Option<&mut NetworkState>, unit_id: usize, well: &game::Well) -> Mino {
		let mut mino = self.next_mino(network_state, unit_id);
		game::center_mino(&mut mino, well);
		mino
	}
}

impl Unit {
	fn local(mode: Mode, player: player::Player) -> Unit {
		let mut rng = game::MinoRng::fair();
		let well = game::Well::filled_with(None, 10, 20);
		let mut kind = UnitKind::Local {
			player,
			queue: {
				let mut queue = VecDeque::with_capacity(5);
				for _ in 0..5 {
					queue.push_back(rng.generate());
				}
				queue
			},
			rng,
		};
		Unit {
			animate_line: vec![false; 20],
			state: UnitState::Play,
			lines_cleared: 0,
			mode,
			can_store_mino: true,
			stored_mino: None,
			falling_mino: None,
			well,
			kind,
		}
	}
	fn network(mode: Mode) -> Unit {
		let well = game::Well::filled_with(None, 10, 20);
		Unit {
			animate_line: vec![false; 20],
			state: UnitState::Play,
			lines_cleared: 0,
			mode,
			can_store_mino: true,
			stored_mino: None,
			falling_mino: None,
			well,
			kind: UnitKind::Network {
				rng_queue: VecDeque::new(),
			}
		}
	}
}

impl Mode {
	fn default_marathon() -> Mode {
		Mode::Marathon{
			level_target: 10, level: 1,
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
	InitBegin,
	InitPlayer {
		name: String,
		seed: u64,
	},
	InitEnd,
	
	AddPlayer {
		name: String,
		seed: u64,
	},
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
	Init,
}

fn apply_unit_event(base: &mut Unit, event: UnitEvent) {
	if let Unit{falling_mino,well,can_store_mino,lines_cleared,animate_line,stored_mino,kind:UnitKind::Network{rng_queue},..} = base {
		match event {
			UnitEvent::TranslateMino {origin, blocks} => {
				if let Some(falling_mino) = falling_mino {
					falling_mino.origin = origin;
					falling_mino.blocks = blocks;
				} else {panic!()}
			}
			UnitEvent::AddMinoToWell => {
				if let Some(falling_mino) = falling_mino {
					let (_can_add, clearable_lines) = game::mino_adding_system(
						falling_mino, well,
						None,
						animate_line,
						can_store_mino,
						&mut ||rng_queue.pop_back().unwrap()
						);
					*lines_cleared += clearable_lines;
				} else {panic!()}
			}
			UnitEvent::GenerateMino {mino} => {
				rng_queue.push_back(mino);
			}
			UnitEvent::Init => {
				*falling_mino = Some(rng_queue.pop_back().unwrap());
			}
			UnitEvent::StoreMino {generated_mino} => {
				if let Some(falling_mino) = falling_mino {
					game::mino_storage_system(
						falling_mino,
						stored_mino,
						well,
						None,
						&mut true,
						can_store_mino,
						||generated_mino.unwrap(),
						None,
						0);
				}
			}
		}
	}
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
	
#[derive(Debug)]
pub enum NetworkState {
	Offline,
	Host {
		listener: TcpListener,
		streams: Vec<LenIO<TcpStream>>,
		readers: Vec<LenIO<TcpStream>>,
		writers: Vec<LenIO<TcpStream>>,
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

#[derive(Debug, Default)]
struct NetworkInit {
	player_names: Vec<String>,
	player_seeds: Vec<u64>,
}

fn main() {
	let stdin = stdin();
	
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
	
	let local_player_text = TextBuilder::new(" (Local)".to_string(), Color::WHITE)
		.build(&font, &texture_creator);
	let network_player_text = TextBuilder::new(" (Network)".to_string(), Color::WHITE)
		.build(&font, &texture_creator);
	
	let get_player_text = |unit: &Unit| -> &sdl2::render::Texture{
		match unit.kind {
			UnitKind::Local {..} => &local_player_text,
			UnitKind::Network {..} => &network_player_text,
		}
	};
	
	let fps: u32 = 60;
	let dpf: Duration = Duration::from_secs(1) / fps;
	
	let mut chosen_game_mode: usize = 0;
	let game_mode_text = [
		TextBuilder::new("Marathon".to_string(),Color::WHITE).build(&font, &texture_creator),
		TextBuilder::new("Sprint".to_string(), Color::WHITE).build(&font, &texture_creator),
	];
	
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
	
	let mut seeder = SmallRng::from_entropy();
	
	let mut player_names = Vec::<String>::new();
	let mut player_names_text = Vec::<sdl2::render::Texture>::new();
	
	let mut player_seeds = Vec::<u64>::new();
	
	let mut network_init: Option<NetworkInit> = None;
	
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
						for unit in units.iter_mut() {
							if let UnitKind::Local {player,..} = &mut unit.kind {
								player.update_local(&mut config.players, &event)
							}
						}
					}
					match event{
						// Not adding restart keybind for now
						// Event::KeyDown{keycode: Some(Keycode::R),repeat: false,..}
						// 	=> if let State::Pause | State::Over = state {
						// 	lines_cleared_text = [
						// 		create_lines_cleared_text(0, &font, &texture_creator),
						// 		create_lines_cleared_text(0, &font, &texture_creator),
						// 		create_lines_cleared_text(0, &font, &texture_creator),
						// 		create_lines_cleared_text(0, &font, &texture_creator),
						// 	];
							
			
						// 	level_text = [
						// 		create_level_text(1, &font, &texture_creator),
						// 		create_level_text(1, &font, &texture_creator),
						// 		create_level_text(1, &font, &texture_creator),
						// 		create_level_text(1, &font, &texture_creator),
						// 	];
							
						// 	units.clear();
						// 	for player in players.drain(..) {
						// 		units.push(Unit::new(game_mode_ctors[chosen_game_mode](), player));
						// 	}
							
						// 	state = State::Play;
						// 	prev_state = None;
							
						// 	stopwatch = Duration::from_secs(0);
						// }
						
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
								let mut unit = Unit::local(
									game_mode_ctors[chosen_game_mode](),
									Player::local(configs.next().unwrap(),None));
								unit.falling_mino.replace(
									unit.kind.next_mino_centered(
										Some(&mut network_state), 0, &unit.well));
								units.push(unit);
								
								NetworkState::Offline
							}
							1 => {
								println!("Write the ip pls:");
								
								let mut addr = String::new();
								stdin.read_line(&mut addr).unwrap();
								addr = addr.trim_end().into();
								
								let default_addr = "127.0.0.1:4141".to_socket_addrs().unwrap().next().unwrap();
								
								let addr = addr.to_socket_addrs().ok()
									.and_then(|mut v|v.next())
									.unwrap_or(default_addr);
								
								println!("{:?}", addr);
								
								let listener = TcpListener::bind(addr)
									.expect("Couldn't bind listener");
								listener.set_nonblocking(true)
									.expect("Couldn't set listener to be non-blocking");
								
								state = State::Lobby;
								
								NetworkState::Host {
									listener,
									streams: Vec::new(),
									readers: Vec::new(),
									writers: Vec::new(),
								}
							}
							2 => {
								println!("Write the ip pls:");
								
								let mut addr = String::new();
								stdin.read_line(&mut addr).unwrap();
								addr = addr.trim_end().into();
								
								let default_addr = "127.0.0.1:4141".to_socket_addrs().unwrap().next().unwrap();
								
								let addr = addr.to_socket_addrs().ok()
									.and_then(|mut v|v.next())
									.unwrap_or(default_addr);
								
								println!("{:?}", addr);
								
								let mut name = String::new();
								println!("Player name:");
								stdin.read_line(&mut name).unwrap();
								name = name.trim_end().into();
								
								let stream = TcpStream::connect(addr)
									.expect("Couldn't connect stream");
								stream.set_nonblocking(true)
									.expect("Couldn't set stream to be non-blocking");
								let mut stream = LenIO::new(stream);
								
								let seed = seeder.next_u64();
								player_seeds.push(seed);
								
								state = State::Lobby;
								units.push(Unit::local(game_mode_ctors[chosen_game_mode](), Player::local(configs.next().unwrap(),None)));
								
								stream.write(&serialize(&NetworkEvent::AddPlayer{name:name.clone(),seed}).unwrap()).unwrap();
								player_names_text.push(
									TextBuilder::new(name.clone(), Color::WHITE)
									.build(&font, &texture_creator));
								player_names.push(name);
								
								NetworkState::Client {
									stream,
								}
							}
							_ => {panic!()}
						}
					}
				}
				State::Lobby => {
					if let NetworkState::Host {..} = network_state {
						if is_key_down(&event, Some(Keycode::Return)) {
							network_state.broadcast_event(&NetworkEvent::StartGame);
							state = State::Play;
						}
						if is_key_down(&event, Some(Keycode::Q)) {
							let mut name = String::new();
							println!("Player name:");
							stdin.read_line(&mut name).unwrap();
							name = name.trim_end().into();
							player_names_text.push(
								TextBuilder::new(name.clone(), Color::WHITE)
								.build(&font, &texture_creator));
							
							let seed = seeder.next_u64();
							
							units.push(Unit::local(Mode::default_marathon(), Player::local(configs.next().unwrap(),None)));
							if let NetworkState::Host{streams,..} = &mut network_state {
								let event = serialize(&NetworkEvent::AddPlayer{name:name.clone(),seed}).unwrap();
								for stream in streams {
									stream.write(&event).unwrap();
								}
							}
							player_names.push(name);
							player_seeds.push(seed);
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
								apply_unit_event(&mut units[unit_id], event);
							}
							NetworkEvent::AddPlayer {name, seed} => {
								player_names_text.push(
									TextBuilder::new(name.clone(), Color::WHITE)
									.build(&font, &texture_creator));
								player_names.push(name);
								player_seeds.push(seed);
								units.push(Unit::network(Mode::default_marathon()))
							}
							NetworkEvent::StartGame => {
								for unit in &mut units {
									if let UnitKind::Local {..} = unit.kind {
										// unit.falling_mino.replace(
										// 	unit.kind.next_mino_centered(
										// 		Some(&mut network_state), 0, &unit.well));
									}
								}
							} //only host gets to start game
							NetworkEvent::InitBegin | NetworkEvent::InitEnd | NetworkEvent::InitPlayer {..}
							=> {} //host already has players initted
						}
					}
				}
			}
			NetworkState::Client {ref mut stream} => {
				while let Ok(Ok(event)) = stream.read().map(deserialize::<NetworkEvent>) {
					match event {
						NetworkEvent::UnitEvent {unit_id, event} => {
							apply_unit_event(&mut units[unit_id], event);
						}
						NetworkEvent::AddPlayer {name, seed} => {
								player_names_text.push(
									TextBuilder::new(name.clone(), Color::WHITE)
									.build(&font, &texture_creator));
								player_names.push(name);
								player_seeds.push(seed);
								// units.push(Unit::new(Mode::default_marathon(),Player::network(),seed))
							}
						NetworkEvent::StartGame => {state = State::Play}
						NetworkEvent::InitBegin => {
							network_init = Some(NetworkInit::default());
						}
						NetworkEvent::InitPlayer {name, seed} => {
							if let Some(ref mut network_init) = network_init {
								network_init.player_names.push(name);
								network_init.player_seeds.push(seed);
							}else { panic!(); }
						}
						NetworkEvent::InitEnd => {
							if let Some(mut network_init) = network_init {
								network_init.player_names.append(&mut player_names);
								player_names = network_init.player_names;
								network_init.player_seeds.append(&mut player_seeds);
								player_seeds = network_init.player_seeds;
							}else { panic!(); }
							network_init = None;
						}
					}
				}
			}
		}
		
		// @update
		match state {
			State::Play => {
				for (unit_id,unit,lines_cleared_text,level_text)
				in izip!(0usize..,units.iter_mut(),lines_cleared_text.iter_mut(),level_text.iter_mut()) {
					if let Unit {well, state, stored_mino, can_store_mino, falling_mino, animate_line, lines_cleared, mode, kind: UnitKind::Local {queue,rng,player}} = unit {
						match state {
							UnitState::Play => {
								match player {
									player::Player::Local{store,fall_countdown,rot_direction,move_direction,move_state,move_repeat_countdown,
										fall_duration,fall_state,..} => {
										
										if let Some(falling_mino) = falling_mino {
						
											game::mino_storage_system(
												falling_mino,
												stored_mino,
												well,
												Some(fall_countdown),
												store,
												can_store_mino,
												||{
													let mino = queue.pop_front().unwrap();
													queue.push_back(rng.generate());
													mino
												},
												Some(&mut network_state),
												unit_id,
											);
											
											let mut mino_translated = false;
											
											mino_translated |= 
												game::mino_rotation_system(
													falling_mino,
													&well,
													rot_direction);
											
											mino_translated |=
												game::mino_movement_system(
													falling_mino,
													&well,
													move_state, move_direction,
													move_repeat_countdown,
													move_prepeat_duration, move_repeat_duration,
													dpf);
										
											let (add_mino, mino_translated_while_falling) =
												game::mino_falling_system(
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
												let (can_add, clearable_lines) = game::mino_adding_system(
													falling_mino, well,
													Some(fall_countdown),
													animate_line,
													can_store_mino,
													&mut ||{
														let mino = queue.pop_front().unwrap();
														network_state.broadcast_event(
															&NetworkEvent::UnitEvent{unit_id,event:UnitEvent::GenerateMino{
																mino: mino.clone(),
															}}
														);
														queue.push_back(rng.generate());
														mino
													});
												
												if !can_add {
													*state = UnitState::Over;
												}else {
													*state = UnitState::LineClear{countdown: Duration::from_secs(0)};
													
													*lines_cleared += clearable_lines;
													*lines_cleared_text =
														create_lines_cleared_text(*lines_cleared, &font, &texture_creator);
												}
											}
											
											*fall_countdown += dpf;
										}
									}
									player::Player::Network => {}
								}
							}
							
							UnitState::LineClear{countdown} => {
								*countdown += dpf;
								if *countdown >= line_clear_duration {
									*state = UnitState::Play;
									game::line_clearing_system(well, animate_line);
									
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
				}
				
				stopwatch += dpf;
			}
			
			State::Lobby => {
				if let NetworkState::Host {listener, streams, readers, writers} = &mut network_state {
					while let Ok(incoming) = listener.accept() {
						let reader = incoming.0;
						let writer = reader.try_clone().unwrap();
						
						let reader = LenIO::new(reader);
						let mut writer = LenIO::new(writer);
						// let mut stream = LenIO::new(incoming.0);
						
						writer.write(&serialize(&NetworkEvent::InitBegin).unwrap()).unwrap();
						for (name, seed) in izip!(&player_names, &player_seeds) {
							writer.write(&serialize(&NetworkEvent::InitPlayer{name:name.clone(),seed:*seed}).unwrap()).unwrap();
						}
						writer.write(&serialize(&NetworkEvent::InitEnd).unwrap()).unwrap();
						
						readers.push(reader);
						writers.push(writer);
						println!("{:?}", incoming.1);
						println!("Connection established");
					}
				}
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
			
		}else if let State::Lobby {..} = state {
			
			if let NetworkState::Host {..} = network_state {
				let TextureQuery {width, height, ..} = host_start_text.query();
				let _ = canvas.copy(
					&host_start_text,
					Rect::new(0, 0, width, height),
					Rect::new(0, 0, width, height));
			}
			
			for (i, unit, name_text) in izip!(0..,&units,&player_names_text) {
				let player_text = get_player_text(unit);
				
				let mut x = 0;
				
				let TextureQuery {width, height, ..} = name_text.query();
				let _ = canvas.copy(
					&name_text,
					Rect::new(0, 0, width, height),
					Rect::new(x, 30+i*(32+8), width, height));
				
				x += width as i32;
				
				let TextureQuery {width, height, ..} = player_text.query();
				let _ = canvas.copy(
					&player_text,
					Rect::new(0, 0, width, height),
					Rect::new(x, 30+i*(32+8), width, height));
			}
			
		}else{
			
			let mut layout = Layout {
				x:0,y:0,
				width:window_rect.width() as i32,expected_width:(4*30+15+10*30+15+4*30+15) * units.len() as i32 - 15
			};
			
			for (unit, lines_cleared_text, level_text)
			in izip!(&mut units, &lines_cleared_text, &level_text) {
				let Unit {stored_mino, falling_mino, well, animate_line, ..} = unit;
				
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
				if let Some(falling_mino) = falling_mino {
					let shadow_mino = game::create_shadow_mino(falling_mino, &well);
					block_canvas.draw_mino(&mut canvas, layout.as_vec2i(), &shadow_mino);
					block_canvas.draw_mino(&mut canvas, layout.as_vec2i(), falling_mino);
				}
				
				layout.col(10*30);
				layout.col_margin(15);
				
				layout.row_margin(15);
				if let UnitKind::Local {queue,..} = &unit.kind {
					for mino in queue.iter() {
						block_canvas.draw_mino(&mut canvas, layout.as_vec2i(), mino);
						layout.row(3*30);
						layout.row_margin(15);
					}
					
					layout.col(4*30);
					layout.col_margin(15);
				}
				
				// if let UnitState::Win = state {
					
				// }
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