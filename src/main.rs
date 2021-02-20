// #![windows_subsystem = "windows"]

use sdl2::render::{BlendMode, TextureQuery};
use sdl2::render::WindowCanvas;
use sdl2::{event::Event, render::{Texture,TextureCreator}};
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use std::{net::SocketAddr, time::{Duration,Instant}};
use std::thread::sleep;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use rand::{RngCore, SeedableRng, rngs::SmallRng};

use itertools::izip;

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
pub mod unit;
use util::*;
use vec2::vec2i;
use text_builder::TextBuilder;
use config::Config;
use unit::{UnitEvent,Unit,Mode};

use std::net::TcpListener;
use std::net::TcpStream;
use std::net::ToSocketAddrs;

use lenio::LenIO;

use serde::{Serialize,Deserialize};
use bincode::{serialize,deserialize};
use std::io::stdin;

use mino::Mino;

use player::Player;

// #[derive(PartialEq, Eq, Clone)]
enum State {
	Play,
	Start,
	Lobby,
}

fn create_lines_cleared_text<'a>(
	lines_cleared: u32,
	font: &sdl2::ttf::Font,
	texture_creator: &'a TextureCreator<sdl2::video::WindowContext>)
-> Texture<'a> {
	TextBuilder::new(format!("Lines: {}", lines_cleared), Color::WHITE)
		.with_wrap(120)
		.build(font, texture_creator)
}

fn create_level_text<'a>(
	level: u32,
	font: &sdl2::ttf::Font,
	texture_creator: &'a TextureCreator<sdl2::video::WindowContext>)
-> Texture<'a> {
	TextBuilder::new(format!("Level: {}", level), Color::WHITE)
		.with_wrap(120)
		.build(font, texture_creator)
}

fn on_level_changed<'a>(
	level: u32,
	font: &sdl2::ttf::Font,
	texture_creator: &'a TextureCreator<sdl2::video::WindowContext>,
	level_text: &mut Texture<'a>,
	fall_duration: Option<&mut Duration>) {
	*level_text = create_level_text(level, font, texture_creator);
	if let Some(fall_duration) = fall_duration {
		*fall_duration = get_fall_duration(level);
	}
}

fn get_fall_duration(level: u32) -> Duration {
	let base: Duration = Duration::from_secs_f64(0.5);
	base.div_f64(1f64 + (level as f64 / 10f64))
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
	},
	InitEnd,
	
	AddPlayer {
		name: String,
	},
	StartGame,
}

fn apply_unit_event(base: &mut Unit, event: UnitEvent) {
	if let Unit{base: unit::Base{falling_mino,well,can_store_mino,lines_cleared,animate_line,stored_mino,state,..},kind:unit::Kind::Network{rng_queue},..} = base {
		match event {
			UnitEvent::TranslateMino {origin, blocks} => {
				if let Some(falling_mino) = falling_mino {
					falling_mino.origin = origin;
					falling_mino.blocks = blocks;
				} else {panic!()}
			}
			UnitEvent::AddMinoToWell => {
				if let Some(falling_mino) = falling_mino {
					*state = unit::State::LineClear {countdown:Duration::from_secs(0)};
					let (_can_add, clearable_lines, _sendable_lines) = game::mino_adding_system(
						falling_mino, well,
						None,
						animate_line,
						can_store_mino,
						&mut ||rng_queue.pop_back().unwrap());
					*lines_cleared += clearable_lines;
				} else {panic!()}
			}
			UnitEvent::GenerateMino {mino} => {
				rng_queue.push_back(mino);
			}
			UnitEvent::Init => {
				let mut mino = rng_queue.pop_back().unwrap();
				game::center_mino(&mut mino, well);
				*falling_mino = Some(mino);
			}
			UnitEvent::StoreMino => {
				if let Some(falling_mino) = falling_mino {
					game::mino_storage_system(
						falling_mino,
						stored_mino,
						well,
						None,
						&mut true,
						can_store_mino,
						||rng_queue.pop_back().unwrap(),
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

fn draw_menu_select(canvas: &mut WindowCanvas, rect: sdl2::rect::Rect) {
	canvas.set_draw_color(Color::RGBA(255, 255, 0, 127));
	let _ = canvas.draw_rect(rect);
}
	
#[derive(Debug)]
pub enum NetworkState {
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

#[derive(Default)]
struct NetworkInit {
	player_names: Vec<String>,
	units: Vec<Unit>,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum StartSelection {
	Continue,
	NewGame,
	GameMode,	
	NetworkMode,
}

fn darken(canvas: &mut WindowCanvas) {
	canvas.set_draw_color(Color::RGBA(0,0,0,160));
	let _ = canvas.fill_rect(None);
}

fn ask_for_ip(stdin: &std::io::Stdin) -> SocketAddr {
	println!("Write the ip pls:");
	
	let mut addr = String::new();
	stdin.read_line(&mut addr).unwrap();
	addr = addr.trim_end().into();
	
	let default_addr = "127.0.0.1:4141".to_socket_addrs().unwrap().next().unwrap();
	
	let addr = addr.to_socket_addrs().ok()
		.and_then(|mut v|v.next())
		.unwrap_or(default_addr);
	
	println!("{:?}", addr);
	
	addr
}

fn ask_for_name(stdin: &std::io::Stdin) -> String {
	let mut name = String::new();
	println!("Player name:");
	stdin.read_line(&mut name).unwrap();
	name = name.trim_end().into();
	name
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
	
	let window_rect = if let (Some(width), Some(height)) = (config.width, config.height) {
		Rect::new(0, 0, width, height)
	}else {
		video_subsystem.display_bounds(0).unwrap()
	};
	
	let mut window = video_subsystem.window(
			"Tetris part 3",
			window_rect.width(),
			window_rect.height());
	window.position_centered();
	if config.borderless {
		window.borderless();
	}
	let window = window
		.build()
		.expect("Failed to create window");
	
	let mut canvas = window.into_canvas().build()
		.expect("Failed to create canvas");
	canvas.set_blend_mode(BlendMode::Blend);
	
	let mut event_pump = sdl_context.event_pump()
		.expect("Failed to create event pump");
	
	let texture_creator = canvas.texture_creator();
	let texture = texture_creator.load_texture("gfx/block.png")
		.expect("Failed to load block texture");
	
	let font = ttf_context.load_font("gfx/IBMPlexMono-Regular.otf", 32)
		.expect("Failed to load font");
	
	let title = texture_creator.load_texture("gfx/title.png").unwrap();
	
	let mut paused = false;
	let paused_text = TextBuilder::new("Paused press q to save".to_string(), Color::WHITE)
		.with_wrap(120)
		.build(&font, &texture_creator);
	
	let game_over_text = TextBuilder::new("Game over".to_string(), Color::WHITE)
		.with_wrap(10*30)
		.build(&font, &texture_creator);
	let game_won_text = TextBuilder::new("You won".to_string(), Color::WHITE)
		.with_wrap(10*30)
		.build(&font, &texture_creator);
	
	let host_start_text = TextBuilder::new("Press enter to start game".to_string(), Color::WHITE)
		.with_wrap(window_rect.width() as u32)
		.build(&font, &texture_creator);
	
	let local_player_text = TextBuilder::new(" (Local)".to_string(), Color::WHITE)
		.build(&font, &texture_creator);
	let network_player_text = TextBuilder::new(" (Network)".to_string(), Color::WHITE)
		.build(&font, &texture_creator);
	
	let get_player_text = |unit: &Unit| -> &Texture{
		match unit.kind {
			unit::Kind::Local {..} => &local_player_text,
			unit::Kind::Network {..} => &network_player_text,
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
	let get_network_text = |selected_network_state: i32|
		match selected_network_state {
			0 => &offline_text,
			1 => &host_text,
			2 => &client_text,
			_ => panic!(),
		};
	
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
	
	let softdrop_duration = Duration::from_secs_f64(0.05);
	
	let mut units: Vec<Unit> = Vec::new();
	
	let block_canvas = block::Canvas::new(&texture);
	
	let mut state = State::Start;
	
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
	
	let mut player_names = Vec::<String>::new();
	let mut player_names_text = Vec::<Texture>::new();
	
	let mut network_init: Option<NetworkInit> = None;
	
	let mut stopwatch = Duration::from_secs(0);
	
	let mut network_players = 0u32;
	
	let mut start_game = false;
	
	let can_continue_text = TextBuilder::new("Continue".into(), Color::WHITE).build(&font, &texture_creator);
	let cant_continue_text = TextBuilder::new("Continue".into(), Color::GRAY).build(&font, &texture_creator);
	let mut saved_unit: Option<unit::Base> = {
		use std::fs::File;
		use std::io::prelude::*;
		let file = File::open("save");
		file.ok().and_then(|mut file|{
			let mut buf = Vec::<u8>::new();
			file.read_to_end(&mut buf).ok().and_then(|_|{
				deserialize(&buf).ok()
			})
		})
	};
	
	let get_continue_text = |can_continue|{
		if can_continue {&can_continue_text}
		else {&cant_continue_text}
	};
	
	let mut quick_game = true;
	let new_game_text = TextBuilder::new("New Game".into(), Color::WHITE).build(&font, &texture_creator);
	let quick_game_text = TextBuilder::new("Quick Game".into(), Color::WHITE).build(&font, &texture_creator);
	
	let get_game_text = |quick_game|{
		if quick_game {&quick_game_text}
		else {&new_game_text}
	};
	
	let just_saved_text = TextBuilder::new("Saved!".into(), Color::WHITE).build(&font, &texture_creator);
	let mut just_saved = false;
	
	let mut start_selection = StartSelection::Continue;
	
	let mut other_rng = SmallRng::from_entropy();
	
	'running: loop {
		let start = Instant::now();
		
		// @input
		for event in event_pump.poll_iter() {
			match event {
				Event::Quit{..} => break 'running,
				_ => (),
			};
			
			match state {
				State::Play => {
					for unit in units.iter_mut() {
						if let unit::Kind::Local {player,..} = &mut unit.kind {
							player.update(&mut config.players, &event)
						}
					}
					// if is_key_down(&event, Some(Keycode::L)) {
					// 	let unit::Unit{base:unit::Base{well,..},..} = &mut units[0];
					// 	game::try_add_bottom_gap_lines(well, 2, 3);
					// }
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
							=> {paused = !paused;just_saved = false;}
						
						Event::KeyDown{keycode:Some(Keycode::Q), repeat:false, ..}
						if paused => {
							use std::fs::File;
							use std::io::prelude::*;
							let mut file = File::create("save").unwrap();
							file.write_all(&serialize(&units[0].base).unwrap()).unwrap();
							just_saved = true;
						}
						
						_ => ()
					};
				}
				State::Start => {
					let keybinds = &mut config.players[0];
					
					use StartSelection::*;
					if is_key_down(&event, Some(Keycode::S)) {
						start_selection = match start_selection {
							Continue => NewGame,
							NewGame => GameMode,
							GameMode => NetworkMode,
							NetworkMode => Continue,
						}
					}
					if is_key_down(&event, Some(Keycode::W)) {
						start_selection = match start_selection {
							Continue => NetworkMode,
							NewGame => Continue,
							GameMode => NewGame,
							NetworkMode => GameMode,
						}
					}
					
					match start_selection {
						Continue => {
							if is_key_down(&event, Some(Keycode::Return)) {
								if selected_network_state == 0 {
									network_state = NetworkState::Offline;
									state = State::Play;
									
									let player = Player::new(configs.next().unwrap(),None);
									let unit = Unit {base: saved_unit.take().unwrap(), kind: unit::Kind::local(player)};
									lines_cleared_text[0] = create_lines_cleared_text(unit.base.lines_cleared, &font, &texture_creator);
									if let Unit{base:unit::Base{mode:Mode::Marathon{level,..},..},..} = unit {
										level_text[0] = create_level_text(level, &font, &texture_creator);
									}
									units.push(unit);
								}
							}
						},
						NewGame => {
							if is_key_down(&event, Some(Keycode::Return)) {
								if quick_game {
									state = State::Play;
									
									let player = Player::new(configs.next().unwrap(),None);
									let mut unit = Unit::local(Mode::default_marathon(), player);
									if let unit::Kind::Local {rng,..} = &mut unit.kind {
										unit.base.falling_mino.replace(
											rng.next_mino_centered(
												&mut network_state, 0, &unit.base.well));
									}
									units.push(unit);
								}else {
									network_state = match selected_network_state {
										0 => {
											state = State::Lobby;
											
											NetworkState::Offline
										}
										1 => {
											let addr = ask_for_ip(&stdin);
											
											let listener = TcpListener::bind(addr)
												.expect("Couldn't bind listener");
											listener.set_nonblocking(true)
												.expect("Couldn't set listener to be non-blocking");
											
											state = State::Lobby;
											
											NetworkState::Host {
												listener,
												streams: Vec::new(),
											}
										}
										2 => {
											let addr = ask_for_ip(&stdin);
											let name = ask_for_name(&stdin);
											
											let stream = TcpStream::connect(addr)
												.expect("Couldn't connect stream");
											stream.set_nonblocking(true)
												.expect("Couldn't set stream to be non-blocking");
											let mut stream = LenIO::new(stream);
											
											state = State::Lobby;
											units.push(Unit::local(game_mode_ctors[chosen_game_mode](), Player::new(configs.next().unwrap(),None)));
											
											stream.write(&serialize(&NetworkEvent::AddPlayer{name:name.clone()}).unwrap()).unwrap();
											player_names_text.push(
												TextBuilder::new(name.clone(), Color::WHITE)
												.build(&font, &texture_creator));
											player_names.push(name);
											
											NetworkState::Client {
												stream,
											}
										}
										_ => panic!()
									}
								}
							}
							if is_key_down(&event, Some(Keycode::A)) ||
							is_key_down(&event, Some(Keycode::D)) {
								quick_game = !quick_game;
							}
						},
						GameMode => {
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
						},
						NetworkMode => {
							if is_key_down(&event, keybinds.right) ||
							is_key_down(&event, keybinds.right_alt) ||
							is_controlcode_down(&event, &mut keybinds.controller_right, None) {
								selected_network_state = network_states.next().unwrap();
							}
						},
					}
					
					
				}
				State::Lobby => {
					if let NetworkState::Host {..} | NetworkState::Offline = network_state {
						if is_key_down(&event, Some(Keycode::Return)) {
							network_state.broadcast_event(&NetworkEvent::StartGame);
							start_game = true;
						}
						if is_key_down(&event, Some(Keycode::Q)) {
							let name = ask_for_name(&stdin);
							player_names_text.push(
								TextBuilder::new(name.clone(), Color::WHITE)
								.build(&font, &texture_creator));
							
							units.push(Unit::local(Mode::default_marathon(), Player::new(configs.next().unwrap(),None)));
							if let NetworkState::Host{streams,..} = &mut network_state {
								let event = serialize(&NetworkEvent::AddPlayer{name:name.clone()}).unwrap();
								for stream in streams {
									stream.write(&event).unwrap();
								}
							}
							player_names.push(name);
						}
					}
				}
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
							NetworkEvent::AddPlayer {name} => {
								player_names_text.push(
									TextBuilder::new(name.clone(), Color::WHITE)
									.build(&font, &texture_creator));
								player_names.push(name);
								units.push(Unit::network(Mode::default_marathon()))
							}
							NetworkEvent::StartGame => {} //only host gets to start game
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
						NetworkEvent::AddPlayer {name} => {
								network_players += 1;
								player_names_text.push(
									TextBuilder::new(name.clone(), Color::WHITE)
									.build(&font, &texture_creator));
								player_names.push(name);
								units.push(Unit::network(Mode::default_marathon()));
							}
						NetworkEvent::StartGame => {start_game = true}
						NetworkEvent::InitBegin => {
							network_init = Some(NetworkInit::default());
						}
						NetworkEvent::InitPlayer {name} => {
							if let Some(ref mut network_init) = network_init {
								network_players += 1;
								network_init.player_names.push(name);
								network_init.units.push(Unit::network(Mode::default_marathon()));
							}else { panic!(); }
						}
						NetworkEvent::InitEnd => {
							if let Some(mut network_init) = network_init {
								let mut player_names_text_init = Vec::<Texture>::new();
								for name in &network_init.player_names {
									player_names_text_init.push(
										TextBuilder::new(name.clone(), Color::WHITE)
										.build(&font, &texture_creator));
								}
								player_names_text_init.append(&mut player_names_text);
								player_names_text = player_names_text_init;
								
								network_init.player_names.append(&mut player_names);
								player_names = network_init.player_names;
								network_init.units.append(&mut units);
								units = network_init.units;
							}else { panic!(); }
							network_init = None;
						}
					}
				}
			}
		}
		
		if start_game {
			start_game = false;
			state = State::Play;
			for (unit_id, unit) in izip!(0.., &mut units) {
				if let unit::Kind::Local {rng,..} = &mut unit.kind {
					unit.base.falling_mino.replace(
						rng.next_mino_centered(
							&mut network_state, unit_id, &unit.base.well));
					network_state.broadcast_event(
						&NetworkEvent::UnitEvent {
							unit_id,
							event: UnitEvent::Init,
						}
					)
				}
			}
		}
		
		// @update
		match state {
			State::Play => {
				for (unit_id,lines_cleared_text,level_text)
				in izip!(0usize..units.len(),lines_cleared_text.iter_mut(),level_text.iter_mut()) {
					let Unit {base: unit::Base {well, state, stored_mino, can_store_mino, falling_mino, animate_line, lines_cleared, mode}, kind} = &mut units[unit_id];
					match state {
						unit::State::Play if (!paused || network_players > 0) => {
							if let unit::Kind::Local {player,rng} = kind {
								let player::Player {store,fall_countdown,rot_direction,move_direction,move_state,move_repeat_countdown,
								fall_duration,fall_state,..} = player;
								
								if let Some(falling_mino) = falling_mino {
				
									let mino_stored = game::mino_storage_system(
										falling_mino,
										stored_mino,
										well,
										Some(fall_countdown),
										store,
										can_store_mino,
										||{
											rng.next_mino(&mut network_state, unit_id)
										},
										unit_id,
									);
									
									if mino_stored {
										network_state.broadcast_event(
											&NetworkEvent::UnitEvent {
												unit_id,
												event: UnitEvent::StoreMino,
											}
										);
									}
									
									let mut mino_translated = false;
									
									let config::Player {move_prepeat_duration,move_repeat_duration,..} = &config.players[player.config_id];
									
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
											*move_prepeat_duration, *move_repeat_duration,
											dpf);
								
									let (add_mino, mino_translated_while_falling) =
										game::mino_falling_system(
											falling_mino, &well,
											fall_countdown,
											*fall_duration, softdrop_duration,
											fall_state);
									
									mino_translated |= mino_translated_while_falling;
									
									*fall_countdown += dpf;
									
									if mino_translated {
										network_state.broadcast_event(
											&NetworkEvent::UnitEvent{unit_id,event:UnitEvent::TranslateMino{
												origin: falling_mino.origin,
												blocks: falling_mino.blocks.clone()
											}}
										);
									}
									
									// let update_target = true;
									// if update
									
									if add_mino {
										let (can_add, clearable_lines, sendable_lines) = game::mino_adding_system(
											falling_mino, well,
											Some(fall_countdown),
											animate_line,
											can_store_mino,
											||{
												rng.next_mino(&mut network_state, unit_id)
											});
										network_state.broadcast_event(
											&NetworkEvent::UnitEvent {
												unit_id,
												event: UnitEvent::AddMinoToWell
											}
										);
										
										if !can_add {
											*state = unit::State::Over;
										}else {
											if let Mode::Versus {lines_received,..} = mode {
												while !lines_received.is_empty() {
													game::try_add_bottom_line_with_gap(
														well, lines_received.pop_front().unwrap() as usize,
														other_rng.next_u32() as usize % well.num_rows());
												}
											}
											if clearable_lines > 0 {
												*state = unit::State::LineClear{countdown: Duration::from_secs(0)};
												
												*lines_cleared += clearable_lines;
												*lines_cleared_text =
													create_lines_cleared_text(*lines_cleared, &font, &texture_creator);
												
												if let Mode::Marathon {level,lines_before_next_level,..} = mode {
													*lines_before_next_level -= clearable_lines as i32;
													let mut level_changed = false;
													while *lines_before_next_level <= 0 {
														*level += 1;
														*lines_before_next_level += unit::get_lines_before_next_level(*level);
														level_changed = true;
													}
													if level_changed {
														on_level_changed(
															*level,
															&font, &texture_creator, level_text,
															Some(fall_duration));
													}
												}else if let Mode::Versus {target_unit_id: _,..} = mode {
													let target_unit_id = (unit_id+1).rem_euclid(2);//*target_unit_id;
													if let Unit{base:unit::Base{mode:Mode::Versus{lines_received,..},..},..} = &mut units[target_unit_id] {
														lines_received.push_back(sendable_lines);
													}
												}
											}
										}
									}
								}
							}
						}
							
						unit::State::LineClear{countdown} => {
							*countdown += dpf;
							if *countdown >= line_clear_duration {
								*state = unit::State::Play;
								game::line_clearing_system(well, animate_line);
								
								// match mode {
								// 	Mode::Marathon{level,level_target,..} => {
								// 		if *level >= *level_target {
								// 			let _won_text =
								// 				TextBuilder::new(
								// 					format!("You won! Press r to restart.").to_string(),
								// 					Color::WHITE)
								// 				.with_wrap(15 + 4*30 + 15 + 10*30 + 15 + 4*30 + 15)
								// 				.build(&font, &texture_creator);
								// 			*state = unit::State::Win;
								// 		}
								// 	}
								// 	Mode::Sprint{lines_cleared_target} => {
								// 		if *lines_cleared >= *lines_cleared_target {
								// 			let _won_text =
								// 				TextBuilder::new(
								// 					format!("You won in {:.2} seconds! Press r to restart.", stopwatch.as_secs_f64()).to_string(),
								// 					Color::WHITE)
								// 				.with_wrap(15 + 4*30 + 15 + 10*30 + 15 + 4*30 + 15)
								// 				.build(&font, &texture_creator);
								// 			*state = unit::State::Win;
								// 		}
								// 	}
								// }
							}
						}
						
						unit::State::Over => {}
						unit::State::Win => {}
						_ => {}
					}
				}
				
				stopwatch += dpf;
			}
			
			State::Lobby => {
				if let NetworkState::Host {listener, streams} = &mut network_state {
					while let Ok(incoming) = listener.accept() {
						network_players += 1;
						let mut stream = LenIO::new(incoming.0);
						
						stream.write(&serialize(&NetworkEvent::InitBegin).unwrap()).unwrap();
						for name in &player_names {
							stream.write(&serialize(&NetworkEvent::InitPlayer{name:name.clone()}).unwrap()).unwrap();
						}
						stream.write(&serialize(&NetworkEvent::InitEnd).unwrap()).unwrap();
						
						streams.push(stream);
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
			
			let continue_text = get_continue_text(saved_unit.is_some() && selected_network_state == 0);
			
			let TextureQuery {width, height, ..} = continue_text.query();
			let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
			let _ = canvas.copy(
				&continue_text,
				Rect::new(0, 0, width, height),
				rect);
			if start_selection == StartSelection::Continue {
				draw_menu_select(&mut canvas, rect);
			}
			
			layout.row(height as i32);
			layout.row_margin(15);
			
			let game_text = get_game_text(quick_game);
			let TextureQuery {width, height, ..} = game_text.query();
			let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
			let _ = canvas.copy(
				&game_text,
				Rect::new(0, 0, width, height),
				rect);
			if start_selection == StartSelection::NewGame {
				draw_menu_select(&mut canvas, rect);
			}
			
			if !quick_game {
				layout.row(height as i32);
				layout.row_margin(15);
				
				let game_mode_text =
					&game_mode_text[chosen_game_mode];
				let TextureQuery {width, height, ..} = game_mode_text.query();
				let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
				let _ = canvas.copy(
					&game_mode_text,
					Rect::new(0, 0, width, height),
					rect);
				if start_selection == StartSelection::GameMode {
					draw_menu_select(&mut canvas, rect);
				}
				
				layout.row(height as i32);
				layout.row_margin(15);
				
				let network_text = get_network_text(selected_network_state);
				let TextureQuery {width, height, ..} = network_text.query();
				let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
				let _ = canvas.copy(
					&network_text,
					Rect::new(0, 0, width, height),
					rect);
				if start_selection == StartSelection::NetworkMode {
					draw_menu_select(&mut canvas, rect);
				}
			}
			
		}else if let State::Lobby {..} = state {
			
			if let NetworkState::Host {..} | NetworkState::Offline = network_state {
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
				let Unit {base: unit::Base {stored_mino, falling_mino, well, animate_line, state, ..}, ..} = unit;
				
				layout.row_margin(15);
				
				if let Some(ref stored_mino) = stored_mino {
					block_canvas.draw_mino_centered(&mut canvas, layout.as_vec2i(), stored_mino, vec2i!(4,3));
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
				if let unit::Kind::Local {rng: unit::LocalMinoRng {queue,..},..} = &unit.kind {
					for mino in queue.iter() {
						block_canvas.draw_mino_centered(&mut canvas, layout.as_vec2i(), mino, vec2i!(4,3));
						layout.row(3*30);
						layout.row_margin(15);
					}
					
					layout.col(4*30);
					layout.col_margin(15);
				}
				
				match state {
					unit::State::Win => {
						darken(&mut canvas);
				
						let TextureQuery {width, height, ..} = game_won_text.query();
						let _ = canvas.copy(
							&game_won_text,
							Rect::new(0, 0, width, height),
							Rect::new(((window_rect.width()-width)/2) as i32, ((window_rect.height()-height)/2) as i32, width, height));
					}
					unit::State::Over => {
						darken(&mut canvas);
				
						let TextureQuery {width, height, ..} = game_over_text.query();
						let _ = canvas.copy(
							&game_over_text,
							Rect::new(0, 0, width, height),
							Rect::new(((window_rect.width()-width)/2) as i32, ((window_rect.height()-height)/2) as i32, width, height));
					}
					_ => {}
				}
			}
			
			if paused {
				darken(&mut canvas);
				
				let TextureQuery {width, height, ..} = paused_text.query();
				let _ = canvas.copy(
					&paused_text,
					Rect::new(0, 0, width, height),
					Rect::new(((window_rect.width()-width)/2) as i32, ((window_rect.height()-height)/2) as i32, width, height));
				
				if just_saved {
					let TextureQuery {width, height, ..} = just_saved_text.query();
					let _ = canvas.copy(
						&just_saved_text,
						Rect::new(0, 0, width, height),
						Rect::new(((window_rect.width()-width)/2) as i32, ((window_rect.height()-height)/2) as i32 + 100, width, height));
				}
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