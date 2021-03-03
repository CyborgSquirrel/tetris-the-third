// #![windows_subsystem = "windows"]

use sdl2::render::{BlendMode, TextureQuery};
use sdl2::render::WindowCanvas;
use sdl2::{event::Event, render::Texture};
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use std::{net::SocketAddr, time::{Duration, Instant}};
use std::thread::sleep;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use rand::{SeedableRng, rngs::SmallRng};
use slotmap::{DefaultKey, SlotMap};

use itertools::izip;

#[macro_use]
pub mod vec2;
pub mod mino;
pub mod block;
pub mod text;
pub mod config;
pub mod lenio;
pub mod game;
pub mod util;
pub mod mino_controller;
pub mod unit;
pub mod ui;
use util::*;
use vec2::vec2i;
use text::TextCreator;
use config::Config;
use unit::{UnitEvent, Unit, Mode};

use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use lenio::LenIO;

use serde::{Serialize,Deserialize};
use bincode::{serialize,deserialize};
use std::io::stdin;

use mino::Mino;

use mino_controller::MinoController;

use ui::{EnumSelect, Layout, NetworkStateSelection, Pause, PauseSelection, StartLayout, StartSelection};

enum State {
	Play,
	Start,
	Lobby,
}

fn create_lines_cleared_text<'a>(
	lines_cleared: u32,
	text_creator: &'a TextCreator,
) -> Texture<'a> {
	text_creator.builder(&format!("Lines: {}", lines_cleared))
		.with_wrap(120).build()
}

fn create_level_text<'a>(
	level: u32,
	text_creator: &'a TextCreator,
) -> Texture<'a> {
	text_creator.builder(&format!("Level: {}", level))
		.with_wrap(120).build()
}

use vec2::vec2f;

#[derive(Debug, Serialize, Deserialize)]
enum NetworkEvent {
	UnitEvent {
		unit_id: usize,
		event: UnitEvent,
	},
	Init {
		init_players: SlotMap<DefaultKey, Player>,
		init_player_keys: Vec<DefaultKey>,
	},
	AddPlayer {
		name: String,
	},
	StartGame,
	RestartGame,
}

fn draw_select(canvas: &mut WindowCanvas, rect: sdl2::rect::Rect) {
	canvas.set_draw_color(Color::RGBA(255, 255, 0, 127));
	let _ = canvas.draw_rect(rect);
}

fn select(canvas: &mut WindowCanvas, rect: sdl2::rect::Rect, is_selected: bool) {
	if is_selected {
		draw_select(canvas, rect);
	}
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

#[derive(Default,Serialize,Deserialize)]
struct NetworkInit {
	players: SlotMap<DefaultKey, Player>,
	player_keys: Vec<DefaultKey>,
	units: Vec<Unit>,
}

fn darken(canvas: &mut WindowCanvas) {
	canvas.set_draw_color(Color::RGBA(0,0,0,160));
	let _ = canvas.fill_rect(None);
}

fn get_texture_dim(texture: &Texture) -> (u32,u32) {
	let TextureQuery {width, height,..} = texture.query();
	(width, height)
}

fn draw_same_scale(canvas: &mut WindowCanvas, texture: &Texture, rect: sdl2::rect::Rect) {
	let _ = canvas.copy(&texture, Rect::new(0, 0, rect.width(), rect.height()), rect);
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

#[derive(Debug,Clone,Serialize,Deserialize)]
enum PlayerKind {
	Local,
	Network,
}

#[derive(Debug,Clone,Serialize,Deserialize)]
struct Player {
	kind: PlayerKind,
	name: String,
}

impl Player {
	fn local(name: String) -> Player {
		Player {
			kind: PlayerKind::Local,
			name,
		}
	}
	fn network(name: String) -> Player {
		Player {
			kind: PlayerKind::Network,
			name,
		}
	}
}

fn main() {
	let stdin = stdin();
	
	let sdl_context = sdl2::init()
		.expect("Failed to initialize sdl2");
	let video_subsystem = sdl_context.video()
		.expect("Failed to initialize video subsystem");
	let game_controller_subsystem = sdl_context.game_controller()
		.expect("Failed to initialize controller subsystem");
	let ttf_context = sdl2::ttf::init()
		.expect("Failed to initialize ttf");
	
	let mut controllers = Vec::new();
	for i in 0..8usize {
		controllers.push(game_controller_subsystem.open(i as u32).ok());
	}
	
	let mut config = Config::from_file();
	let mut configs = (0..4usize).cycle();
	
	let window_rect = if let (Some(width), Some(height)) = (config.width, config.height) {
		Rect::new(0, 0, width, height)
	}else {
		video_subsystem.display_bounds(0).unwrap()
	};
	
	let mut window = video_subsystem.window(
		"Tetris part 3",window_rect.width(),window_rect.height());
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
	
	let text_creator = TextCreator::new(&texture_creator, &font);
	
	let title = texture_creator.load_texture("gfx/title.png").unwrap();
	
	let paused_text = text_creator.builder("Paused").build();
	
	let game_over_text = text_creator.builder("Game over")
		.with_wrap(10*config.block_size).build();
	let game_won_text = text_creator.builder("You won")
		.with_wrap(10*config.block_size).build();
	
	let host_start_text = text_creator.builder("Press enter to start game")
		.with_wrap(window_rect.width() as u32).build();
	
	let local_player_text = text_creator.builder(" (Local)").build();
	let network_player_text = text_creator.builder(" (Network)").build();
	
	let get_player_text = |unit: &Unit| -> &Texture{
		match unit.kind {
			unit::Kind::Local {..} => &local_player_text,
			unit::Kind::Network {..} => &network_player_text,
		}
	};
	
	let fps: u32 = 60;
	let dpf: Duration = Duration::from_secs(1) / fps;
	
	let mut selected_game_mode: usize = 0;
	let game_mode_text = [
		text_creator.builder("Marathon").build(),
		text_creator.builder("Sprint").build(),
		text_creator.builder("Versus").build(),
	];
	let game_mode_ctors = [
		Mode::default_marathon,
		Mode::default_sprint,
		Mode::default_versus,
	];
	
	let mut selected_network_state = NetworkStateSelection::Offline;
	let mut network_state = NetworkState::Offline;
	
	let offline_text = text_creator.builder("Offline").build();
	let host_text = text_creator.builder("Host").build();
	let client_text = text_creator.builder("Client").build();
	let get_network_text = |ref selected_network_state: &NetworkStateSelection|
		match selected_network_state {
			NetworkStateSelection::Offline => &offline_text,
			NetworkStateSelection::Host => &host_text,
			NetworkStateSelection::Client => &client_text,
		};
	
	let mut lines_cleared_text = [
		create_lines_cleared_text(0, &text_creator),
		create_lines_cleared_text(0, &text_creator),
		create_lines_cleared_text(0, &text_creator),
		create_lines_cleared_text(0, &text_creator),
	];
	
	let mut level_text = [
		create_level_text(1, &text_creator),
		create_level_text(1, &text_creator),
		create_level_text(1, &text_creator),
		create_level_text(1, &text_creator),
	];
	
	let softdrop_duration = Duration::from_secs_f64(0.05);
	
	let mut units: Vec<Unit> = Vec::new();
	
	let block_canvas = block::Canvas::new(&texture, config.block_size);
	
	let mut state = State::Start;
	
	let line_clear_duration = Duration::from_secs_f64(0.1);
	
	let mut stopwatch = Duration::from_secs(0);
	
	let mut start_game = false;
	
	let can_continue_text = text_creator.builder("Continue").build();
	let cant_continue_text = text_creator.builder("Continue").color(Color::GRAY).build();
	let saved_unit: Option<Unit> = {
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
	let new_game_text = text_creator.builder("New Game").build();
	let quick_game_text = text_creator.builder("Quick Game").build();
	
	let get_game_text = |quick_game|{
		if quick_game {&quick_game_text}
		else {&new_game_text}
	};
	
	let just_saved_text = text_creator.builder("Saved").build();
	let mut just_saved = false;
	
	let mut start_selection = StartSelection::Continue;
	
	let mut other_rng = SmallRng::from_entropy();
	
	let resume_text = text_creator.builder("Resume").build();
	let save_text = text_creator.builder("Save").build();
	let quit_to_title_text = text_creator.builder("Quit to title").build();
	let quit_to_desktop_text = text_creator.builder("Quit to desktop").build();
	
	let mut network_players = 0u32;
	let mut players = SlotMap::<DefaultKey, Player>::new();
	let mut player_keys = Vec::<DefaultKey>::new();
	let mut player_names_text = Vec::<Texture>::new();
	
	let mut pause: Option<Pause> = None;
	
	'running: loop {
		let start = Instant::now();
		
		// @input
		for event in event_pump.poll_iter() {
			match event {
				Event::Quit{..} => break 'running,
				Event::ControllerDeviceAdded{which,..} =>
				controllers[which as usize] = game_controller_subsystem.open(which).ok(),
				Event::ControllerDeviceRemapped{which,..} => println!("Remapped {:?}", which),
				Event::ControllerDeviceRemoved{which,..} => println!("Removed {:?}", which),
				// Event::ControllerAxisMotion{..} => println!("MOTION"),
				_ => (),
			};
			
			match state {
				State::Play => {
					
					if let Some(Pause{selection}) = &mut pause {
						if is_any_up_down(&event, Some(0)) {
							*selection = selection.prev_variant();
						}
						if is_any_down_down(&event, Some(0)) {
							*selection = selection.next_variant();
						}
						if is_ok_down(&event, Some(0)) {
							match selection {
								PauseSelection::Resume => pause = None,
								PauseSelection::Save => {
									if let NetworkState::Offline = network_state {
										use std::fs::File;
										use std::io::prelude::*;
										let mut file = File::create("save").unwrap();
										file.write_all(&serialize(&units[0]).unwrap()).unwrap();
										just_saved = true;
									}
								}
								PauseSelection::QuitToTitle => {
									state = State::Start;
									units.clear();
									configs = (0..4usize).cycle();
								}
								PauseSelection::QuitToDesktop => {
									break 'running;
								}
							}
						}
					}else {
						for unit in units.iter_mut() {
							if let unit::Kind::Local {mino_controller,..} = &mut unit.kind {
								mino_controller.update(&mut config.players, &event)
							}
						}
					}
					
					match event{
						// Deliberately not adding custom pause keybind
						Event::KeyDown{keycode: Some(Keycode::Escape),repeat: false,..} |
						Event::ControllerButtonDown{button: sdl2::controller::Button::Start,..} => {
							pause = if pause.is_some() {None} else {Some(Pause::default())};
							just_saved = false;
						}
						
						Event::KeyDown{keycode: Some(Keycode::R),repeat: false,..} => {
							for unit in &mut units {
								unit.base = unit::Base::new(game_mode_ctors[selected_game_mode]());
							}
							network_state.broadcast_event(&NetworkEvent::RestartGame);
							start_game = true;
						}
						
						Event::KeyDown{keycode:Some(Keycode::Q), repeat:false, ..} => {
							if let NetworkState::Offline = network_state {
								if pause.is_some() {
									use std::fs::File;
									use std::io::prelude::*;
									let mut file = File::create("save").unwrap();
									file.write_all(&serialize(&units[0]).unwrap()).unwrap();
									just_saved = true;
								}
							}
						}
						
						_ => ()
					};
				}
				State::Start => {
					let keybinds = &mut config.players[0];
					
					if is_any_up_down(&event, Some(0)) {
						start_selection = start_selection.prev_variant()
					}
					if is_any_down_down(&event, Some(0)) {
						start_selection = start_selection.next_variant()
					}
					
					use StartSelection::*;
					match start_selection {
						Continue => {
							if is_ok_down(&event, Some(0)) {
								if selected_network_state == NetworkStateSelection::Offline {
									network_state = NetworkState::Offline;
									state = State::Play;
									
									let new_controller = MinoController::new(configs.next().unwrap(),Some(0));
									let mut unit = saved_unit.clone().unwrap();
									if let unit::Kind::Local{mino_controller,..} = &mut unit.kind {
										*mino_controller = new_controller;
									}
									lines_cleared_text[0] = create_lines_cleared_text(unit.base.lines_cleared, &text_creator);
									if let Unit{base:unit::Base{mode:Mode::Marathon{level,..},..},..} = unit {
										level_text[0] = create_level_text(level, &text_creator);
									}
									units.push(unit);
								}
							}
						},
						NewGame => {
							if is_ok_down(&event, Some(0)) {
								if quick_game {
									state = State::Play;
									
									let key = players.insert(Player::local("no name".into()));
									player_keys.push(key);
									let player = MinoController::new(configs.next().unwrap(),Some(0));
									let mut unit = Unit::local(Mode::default_marathon(), player);
									if let unit::Kind::Local {rng,..} = &mut unit.kind {
										unit.base.falling_mino.replace(
											rng.next_mino_centered(
												&mut network_state, 0, &unit.base.well));
									}
									units.push(unit);
								}else {
									network_state = match selected_network_state {
										NetworkStateSelection::Offline => {
											state = State::Lobby;
											
											NetworkState::Offline
										}
										NetworkStateSelection::Host => {
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
										NetworkStateSelection::Client => {
											let addr = ask_for_ip(&stdin);
											let name = ask_for_name(&stdin);
											
											let stream = TcpStream::connect(addr)
												.expect("Couldn't connect stream");
											stream.set_nonblocking(true)
												.expect("Couldn't set stream to be non-blocking");
											let mut stream = LenIO::new(stream);
											
											state = State::Lobby;
											units.push(Unit::local(game_mode_ctors[selected_game_mode](), MinoController::new(configs.next().unwrap(),Some(0))));
											
											let key = players.insert(Player::local(name.clone()));
											player_keys.push(key);
											
											stream.write(&serialize(&NetworkEvent::AddPlayer{name:name.clone()}).unwrap()).unwrap();
											player_names_text.push(text_creator.builder(&name).build());
											
											NetworkState::Client {
												stream,
											}
										}
									}
								}
							}
							if is_any_left_down(&event, keybinds, None) ||
							is_any_right_down(&event, keybinds, None) {
								quick_game = !quick_game;
							}
						},
						GameMode => {
							if is_any_left_down(&event, keybinds, None) {
								selected_game_mode = (selected_game_mode as i32 - 1).rem_euclid(3) as usize;
							}
							if is_any_right_down(&event, keybinds, None) {
								selected_game_mode = (selected_game_mode + 1).rem_euclid(3);
							}
						},
						NetworkMode => {
							if is_any_left_down(&event, keybinds, None) {
								selected_network_state = selected_network_state.next_variant();
							}
							if is_any_right_down(&event, keybinds, None) {
								selected_network_state = selected_network_state.prev_variant();
							}
						},
					}
				}
				State::Lobby => {
					if let NetworkState::Host {..} | NetworkState::Offline = network_state {
						if is_ok_down(&event, Some(0)) {
							network_state.broadcast_event(&NetworkEvent::StartGame);
							start_game = true;
						}
						if is_key_down(&event, Some(Keycode::Q)) {
							let name = String::from("salam");//ask_for_name(&stdin); //TODO: put name back
							let key = players.insert(Player::local(name.clone()));
							player_keys.push(key);
							player_names_text.push(text_creator.builder(&name).build());
							
							units.push(Unit::local(game_mode_ctors[selected_game_mode](), MinoController::new(configs.next().unwrap(),Some(0))));
							if let NetworkState::Host{streams,..} = &mut network_state {
								let event = serialize(&NetworkEvent::AddPlayer{name:name.clone()}).unwrap();
								for stream in streams {
									stream.write(&event).unwrap();
								}
							}
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
								unit::update_network(
									unit_id, &mut units, event,
									|lines_cleared|lines_cleared_text[unit_id] =
									create_lines_cleared_text(lines_cleared, &text_creator),
									|level|level_text[unit_id] =
									create_level_text(level, &text_creator),
								);
							}
							NetworkEvent::AddPlayer {name} => {
								network_players += 1;
								let key = players.insert(Player::network(name.clone()));
								player_keys.push(key);
								player_names_text.push(text_creator.builder(&name).build());
								units.push(Unit::network(Mode::default_versus()))
							}
							_ => {}
						}
					}
				}
			}
			NetworkState::Client {ref mut stream} => {
				while let Ok(Ok(event)) = stream.read().map(deserialize::<NetworkEvent>) {
					match event {
						NetworkEvent::UnitEvent {unit_id, event} => {
							unit::update_network(
								unit_id, &mut units, event,
								|lines_cleared|lines_cleared_text[unit_id] =
								create_lines_cleared_text(lines_cleared, &text_creator),
								|level|level_text[unit_id] =
								create_level_text(level, &text_creator),
							);
						}
						NetworkEvent::AddPlayer {name} => {
								network_players += 1;
								let key = players.insert(Player::network(name.clone()));
								player_keys.push(key);
								player_names_text.push(text_creator.builder(&name).build());
								units.push(Unit::network(Mode::default_versus()));
							}
						NetworkEvent::StartGame => {start_game = true}
						NetworkEvent::Init {mut init_players, mut init_player_keys} => {
							network_players += init_players.len() as u32;
							
							let mut init_units = Vec::<Unit>::new();
							let mut init_player_names_text = Vec::<Texture>::new();
							for (_,player) in init_players.iter() {
								init_units.push(Unit::network(Mode::default_versus()));
								init_player_names_text.push(text_creator.builder(&player.name).build());
							}
							init_units.append(&mut units);
							init_player_names_text.append(&mut player_names_text);
							units = init_units;
							player_names_text = init_player_names_text;
							
							for (_,player) in players.drain() {
								let key = init_players.insert(player);
								init_player_keys.push(key);
							}
							
							players = init_players;
							player_keys = init_player_keys;
						}
						NetworkEvent::RestartGame => {
							for unit in &mut units {
								unit.base = unit::Base::new(game_mode_ctors[selected_game_mode]());
							}
							start_game = true;
						}
					}
				}
			}
		}
		
		if start_game {
			start_game = false;
			state = State::Play;
			let units_len = units.len();
			for (unit_id, unit) in izip!(0.., &mut units) {
				let Unit{kind, base: unit::Base{mode,falling_mino,well,..}} = unit;
				
				if let unit::Kind::Local{rng,..} = kind {
					falling_mino.replace(
						rng.next_mino_centered(
							&mut network_state, unit_id, &well));
					network_state.broadcast_event(
						&NetworkEvent::UnitEvent {
							unit_id,
							event: UnitEvent::Init,
						}
					)
				}
				
				if let Mode::Versus {target_unit_id,..} = mode {
					*target_unit_id = (unit_id+1).rem_euclid(units_len);
				}
				
			}
		}
		
		// @update
		match state {
			State::Play => {
				for (unit_id,lines_cleared_text,level_text)
				in izip!(0usize..units.len(),lines_cleared_text.iter_mut(),level_text.iter_mut()) {
					let unit = &mut units[unit_id];
					match &mut unit.base.state {
						unit::State::Play if (!pause.is_some() || network_players > 0) => {
							unit::update_local(
								unit_id,
								&mut units,
								&mut network_state,
								&mut config,
								softdrop_duration,
								dpf,
								&mut other_rng,
								|lines_cleared|*lines_cleared_text =
								create_lines_cleared_text(lines_cleared, &text_creator),
								|level|*level_text =
								create_level_text(level, &text_creator)
							);
						}
							
						unit::State::LineClear{countdown} => {
							*countdown += dpf;
							if *countdown >= line_clear_duration {
								let Unit {base:unit::Base{well,state,animate_line,..},..} = unit;
								*state = unit::State::Play;
								game::line_clearing_system(well, animate_line);
								
								// match mode {
								// 	Mode::Marathon{level,level_target,..} => {
								// 		if *level >= *level_target {
								// 			let _won_text =
								// 				TextBuilder::new(
								// 					format!("You won! Press r to restart.").to_string(),
								// 					Color::WHITE)
								// 				.with_wrap(15 + 4*config.block_size + 15 + 10*config.block_size + 15 + 4*config.block_size + 15)
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
								// 				.with_wrap(15 + 4*config.block_size + 15 + 10*config.block_size + 15 + 4*config.block_size + 15)
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
						
						stream.write(
							&serialize(
								&NetworkEvent::Init {
									init_players: players.clone(),
									init_player_keys: player_keys.clone(),
								}
							).unwrap()
						).unwrap();
						
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
			
			let mut layout = StartLayout {y:0,width:window_rect.width()};
			
			layout.row_margin(15);
			
			let (width, height) = get_texture_dim(&title);
			let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
			draw_same_scale(&mut canvas, &title, rect);
			
			layout.row(height as i32);
			layout.row_margin(30);
			
			let continue_text = get_continue_text(saved_unit.is_some() && selected_network_state == NetworkStateSelection::Offline);
			let (width, height) = get_texture_dim(&continue_text);
			let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
			draw_same_scale(&mut canvas, &continue_text, rect);
			select(&mut canvas, rect, matches!(start_selection, StartSelection::Continue));
			
			layout.row(height as i32);
			layout.row_margin(15);
			
			let game_text = get_game_text(quick_game);
			let (width, height) = get_texture_dim(&game_text);
			let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
			draw_same_scale(&mut canvas, &game_text, rect);
			select(&mut canvas, rect, matches!(start_selection, StartSelection::NewGame));
			
			if !quick_game {
				layout.row(height as i32);
				layout.row_margin(15);
				
				let game_mode_text = &game_mode_text[selected_game_mode];
				let (width, height) = get_texture_dim(&game_mode_text);
				let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
				draw_same_scale(&mut canvas, &game_mode_text, rect);
				select(&mut canvas, rect, matches!(start_selection, StartSelection::GameMode));
				
				layout.row(height as i32);
				layout.row_margin(15);
				
				let network_text = get_network_text(&selected_network_state);
				let (width, height) = get_texture_dim(&network_text);
				let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
				draw_same_scale(&mut canvas, &network_text, rect);
				select(&mut canvas, rect, matches!(start_selection, StartSelection::NetworkMode));
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
					Rect::new(x, config.block_size as i32+i*(32+8), width, height));
				
				x += width as i32;
				
				let TextureQuery {width, height, ..} = player_text.query();
				let _ = canvas.copy(
					&player_text,
					Rect::new(0, 0, width, height),
					Rect::new(x, config.block_size as i32+i*(32+8), width, height));
			}
			
		}else{
			
			let mut layout = Layout {
				x:0,y:0,
				width:window_rect.width() as i32,expected_width:(4*config.block_size as i32+15+10*config.block_size as i32+15+4*config.block_size as i32+15) * units.len() as i32 - 15
			};
			
			for (unit, lines_cleared_text, level_text)
			in izip!(&mut units, &lines_cleared_text, &level_text) {
				let Unit {base: unit::Base {stored_mino, falling_mino, well, animate_line, state, ..}, ..} = unit;
				
				layout.row_margin(15);
				
				if let Some(ref stored_mino) = stored_mino {
					block_canvas.draw_mino_centered(&mut canvas, layout.as_vec2i(), stored_mino, vec2i!(4,3));
				}
				layout.row(3*config.block_size as i32);
				layout.row_margin(config.block_size as i32 / 2);
				
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
				
				layout.col(4*config.block_size as i32);
				layout.col_margin(15);
				
				layout.row_margin(15);
				
				block_canvas.draw_well(&mut canvas, layout.as_vec2i(), &well, animate_line);
				if let Some(falling_mino) = falling_mino {
					let shadow_mino = game::create_shadow_mino(falling_mino, &well);
					block_canvas.draw_mino(&mut canvas, layout.as_vec2i(), &shadow_mino);
					block_canvas.draw_mino(&mut canvas, layout.as_vec2i(), falling_mino);
				}
				
				layout.col(10*config.block_size as i32);
				layout.col_margin(15);
				
				layout.row_margin(15);
				if let unit::Kind::Local {rng: unit::LocalMinoRng {queue,..},..} = &unit.kind {
					for mino in queue.iter() {
						block_canvas.draw_mino_centered(&mut canvas, layout.as_vec2i(), mino, vec2i!(4,3));
						layout.row(3*config.block_size as i32);
						layout.row_margin(config.block_size as i32 / 2);
					}
					
					layout.col(4*config.block_size as i32);
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
			
			if let Some(Pause{selection}) = &mut pause {
				darken(&mut canvas);
				
				let mut layout = StartLayout {y:0,width:window_rect.width()};
				
				layout.row_margin(15);
				
				let (width, height) = get_texture_dim(&paused_text);
				let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
				draw_same_scale(&mut canvas, &paused_text, rect);
				
				layout.row(height as i32);
				layout.row_margin(15);
				
				let (width, height) = get_texture_dim(&resume_text);
				let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
				draw_same_scale(&mut canvas, &resume_text, rect);
				select(&mut canvas, rect, matches!(selection, PauseSelection::Resume));
				
				layout.row(height as i32);
				layout.row_margin(15);
				
				let (width, height) = get_texture_dim(&save_text);
				let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
				draw_same_scale(&mut canvas, &save_text, rect);
				select(&mut canvas, rect, matches!(selection, PauseSelection::Save));
				
				layout.row(height as i32);
				layout.row_margin(15);
				
				let (width, height) = get_texture_dim(&quit_to_title_text);
				let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
				draw_same_scale(&mut canvas, &quit_to_title_text, rect);
				select(&mut canvas, rect, matches!(selection, PauseSelection::QuitToTitle));
				
				layout.row(height as i32);
				layout.row_margin(15);
				
				let (width, height) = get_texture_dim(&quit_to_desktop_text);
				let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
				draw_same_scale(&mut canvas, &quit_to_desktop_text, rect);
				select(&mut canvas, rect, matches!(selection, PauseSelection::QuitToDesktop));
				
				layout.row(height as i32);
				layout.row_margin(15);
				
				if let NetworkState::Offline = network_state {
					if just_saved {
						let TextureQuery {width, height, ..} = just_saved_text.query();
						let _ = canvas.copy(
							&just_saved_text,
							Rect::new(0, 0, width, height),
							Rect::new(((window_rect.width()-width)/2) as i32, ((window_rect.height()-height)/2) as i32 + 100, width, height));
					}
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