#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::room::Room;
use crate::room::RoomCommand;
use sdl2::{controller::{Axis, Button}, image::LoadSurface, render::{BlendMode, TextureQuery}, surface::Surface};
use sdl2::render::WindowCanvas;
use sdl2::{event::Event, render::Texture};
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use std::{collections::{BTreeSet, VecDeque, BTreeMap}, iter, net::SocketAddr, time::{Duration, Instant}};
use std::thread::sleep;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use config::{InputMethod,Bind,MenuBinds};
use lazy_static::lazy_static;
use network::{NetworkState,NetworkCommand};
use command::Command;

use itertools::izip;

#[macro_use]
pub mod vec2;
pub mod mino;
pub mod block;
pub mod text;
pub mod config;
pub mod lenio;
pub mod game;
pub mod mino_controller;
pub mod unit;
pub mod ui;
pub mod network;
pub mod room;
pub mod command;
pub mod myevents;
use vec2::{vec2i,vec2f};
use text::TextCreator;
use config::Config;
use unit::{Unit, Mode};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use lenio::LenIO;
use serde::{Serialize, Deserialize};
use bincode::{serialize, deserialize};
use mino::Mino;
use mino_controller::MinoController;
use ui::{EnumSelect, GameModeSelection, GameLayout, NetworkStateSelection, Pause, PauseSelection, CenteredLayout, TitleSelection};

pub enum State {
	Play {
		players_won: u32,
		players_done: u32,
		players_lost: u32,
		over: bool,
		winner: Option<String>,
		pause: Option<Pause>,
	},
	Title,
	PreLobby,
	Lobby,
}

impl State {
	fn play() -> Self {
		State::Play {
			players_won: 0,
			players_done: 0,
			players_lost: 0,
			over: false,
			winner: None,
			pause: None,
		}
	}
}

struct LinesClearedText<'a>(Texture<'a>, &'a TextCreator<'a,'a>, u32);
impl<'a> LinesClearedText<'a> {
	fn new(text_creator: &'a TextCreator, block_size: u32) -> Self {
		LinesClearedText(text_creator.builder("").build(), text_creator, block_size)
	}
	fn update(&mut self, lines_cleared: u32) {
		self.0 = self.1.builder(&format!("Lines: {}", lines_cleared)).game().with_wrap(self.2*4).build()
	}
}

struct LevelText<'a>(Texture<'a>, &'a TextCreator<'a,'a>, u32);
impl<'a> LevelText<'a> {
	fn new(text_creator: &'a TextCreator, block_size: u32) -> Self {
		LevelText(text_creator.builder("").build(), text_creator, block_size)
	}
	fn update(&mut self, level: u32) {
		self.0 = self.1.builder(&format!("Level: {}", level)).game().with_wrap(self.2*4).build()
	}
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

fn darken(canvas: &mut WindowCanvas, rect: Option<Rect>) {
	canvas.set_draw_color(Color::RGBA(0,0,0,160));
	let _ = canvas.fill_rect(rect);
}

fn get_texture_dim(texture: &Texture) -> (u32,u32) {
	let TextureQuery {width, height,..} = texture.query();
	(width, height)
}

fn draw_same_scale(canvas: &mut WindowCanvas, texture: &Texture, rect: Rect) {
	let _ = canvas.copy(&texture, Rect::new(0, 0, rect.width(), rect.height()), rect);
}

fn draw_centered(canvas: &mut WindowCanvas, texture: &Texture, centering_rect: Rect) {
	let TextureQuery {width, height,..} = texture.query();
	let _ = canvas.copy(
		&texture,
		Rect::new(0, 0, width, height),
		Rect::new(
			centering_rect.x() + ((centering_rect.width()-width)/2) as i32,
			centering_rect.y() + ((centering_rect.height()-height)/2) as i32,
			width, height));
}

fn string_to_addr(addr: String) -> SocketAddr {
	let default_addr = "127.0.0.1:4141".to_socket_addrs().unwrap().next().unwrap();
	
	let addr = addr.to_socket_addrs().ok()
		.and_then(|mut v|v.next())
		.unwrap_or(default_addr);
	
	addr
}

struct Prompt<'a> {
	text: String,
	label: Texture<'a>,
	texture: Texture<'a>,
	creator: &'a TextCreator<'a,'a>,
}
impl<'a> Prompt<'a> {
	fn new(creator: &'a TextCreator, label: &str) -> Self {
		let text = String::from("");
		let label = creator.builder(label).big().build();
		let texture = creator.builder(text.as_str()).big().build();
		Prompt {text, label, texture, creator}
	}
	fn update(&mut self, text: String) {
		if text != self.text {
			self.text = text;
			self.texture = self.creator.builder(self.text.as_str()).big().build();
		}
	}
	fn input(&mut self, event: &Event) {
		match event {
			Event::TextInput {text, ..} => {
				self.update(self.text.clone()+text);
			}
			Event::KeyDown {keycode: Some(Keycode::Backspace), ..} => {
				let mut text = self.text.clone();
				text.pop();
				self.update(text);
			}
			_ => {}
		}
	}
	fn draw(&self, canvas: &mut WindowCanvas) {
		let mut y = 0;
		
		let (width, height) = get_texture_dim(&self.label);
		let rect = Rect::new(0, 0, width, height);
		draw_same_scale(canvas, &self.label, rect);
		
		y += height as i32;
		
		let (width, height) = get_texture_dim(&self.texture);
		let rect = Rect::new(0, y, width, height);
		draw_same_scale(canvas, &self.texture, rect);
	}
}

#[derive(Debug,Clone,Serialize,Deserialize)]
enum PlayerKind {Local(InputMethod), Network}

impl Default for PlayerKind {fn default() -> Self {PlayerKind::Network}}

#[derive(Debug,Clone,Serialize,Deserialize,Default)]
pub struct Player {
	#[serde(skip)]kind: PlayerKind,
	name: String,
}

impl Player {
	fn new(name: String, input: InputMethod) -> Player {
		Player {
			kind: PlayerKind::Local(input),
			name,
		}
	}
}

fn prev_next_variant<T: EnumSelect>(mut value: T, prev: &Bind, next: &Bind, event: &Event, input_method: &InputMethod) -> T {
	if prev.is_down(event, input_method) {value = value.prev_variant()}
	if next.is_down(event, input_method) {value = value.next_variant()}
	value
}

fn axis_to_usize(axis: Axis) -> usize {
	match axis {
		Axis::LeftX => 0,
		Axis::LeftY => 1,
		Axis::RightX => 2,
		Axis::RightY => 3,
		Axis::TriggerLeft => 4,
		Axis::TriggerRight => 5,
	}
}

fn load_saved_unit() -> Option<Unit> {
	use std::fs::File;
	use std::io::prelude::*;
	let file = File::open("save");
	file.ok().and_then(|mut file|{
		let mut buf = Vec::<u8>::new();
		file.read_to_end(&mut buf).ok().and_then(|_|{
			deserialize(&buf).ok()
		})
	})
}

lazy_static! {
	static ref LINE_CLEAR_DURATION: Duration = Duration::from_secs_f64(0.2);
	static ref GAME_OF_LIFE_DURATION: Duration = Duration::from_secs_f64(0.25);
}

const MENU_FONT_SIZE: u16 = 32;
const BIG_FONT_SIZE: u16 = 128;

const MAX_PLAYERS: usize = 8;

fn main() {
	let sdl_context = sdl2::init()
		.expect("Failed to initialize sdl2");
	let video_subsystem = sdl_context.video()
		.expect("Failed to initialize video subsystem");
	let game_controller_subsystem = sdl_context.game_controller()
		.expect("Failed to initialize controller subsystem");
	let ttf_context = sdl2::ttf::init()
		.expect("Failed to initialize ttf");
	let sdl_event = sdl_context.event().unwrap();
	
	myevents::register_custom_event::<myevents::MyControllerButtonDown>(&sdl_event).unwrap();
	myevents::register_custom_event::<myevents::MyControllerButtonUp>(&sdl_event).unwrap();
	myevents::register_custom_event::<myevents::MyControllerAxisDown>(&sdl_event).unwrap();
	myevents::register_custom_event::<myevents::MyControllerAxisUp>(&sdl_event).unwrap();
	
	let mut controllers = BTreeMap::<_, (sdl2::controller::GameController,usize,[bool;6])>::new();
	let mut unused_controller_ids: BTreeSet<usize> = (0..MAX_PLAYERS).collect();
	
	let config = Config::from_file();
	
	let window_rect = if let (Some(width), Some(height)) = (config.width, config.height) {
		Rect::new(0, 0, width, height)
	}else {
		video_subsystem.display_bounds(0).unwrap()
	};
	
	let mut window = video_subsystem.window("Tetris part 3", window_rect.width(), window_rect.height());
	window.position_centered();
	if config.borderless {
		window.borderless();
	};
	
	let mut window = window.build()
		.expect("Failed to create window");
	
	let icon = Surface::from_file("gfx/icon.png")
		.expect("Could not load icon");
	window.set_icon(icon);
	
	let window = window;
	
	let mut canvas = window.into_canvas().build()
		.expect("Failed to create canvas");
	canvas.set_blend_mode(BlendMode::Blend);
	
	let mut event_pump = sdl_context.event_pump()
		.expect("Failed to create event pump");
	
	let texture_creator = canvas.texture_creator();
	let block = texture_creator.load_texture(&config.block_path)
		.expect("Failed to load block texture");
	let line_clear = texture_creator.load_texture(&config.line_clear_path)
		.expect("Failed to load line clear texture");
	
	let mut block_canvas = block::Canvas::new(block, line_clear, config.block_size_tex, config.block_size_draw, config.line_clear_frames);
	
	let menu_font = ttf_context.load_font("gfx/IBMPlexMono-Regular.otf", MENU_FONT_SIZE)
		.expect("Failed to load font");
	let game_font = ttf_context.load_font("gfx/IBMPlexMono-Regular.otf", config.block_size_draw as u16)
		.expect("Failed to load font");
	let big_font = ttf_context.load_font("gfx/IBMPlexMono-Regular.otf", BIG_FONT_SIZE)
		.expect("Failed to load font");
	
	let text_creator = TextCreator::new(&texture_creator, &menu_font, &game_font, &big_font);
	
	let title = texture_creator.load_texture("gfx/title.png").unwrap();
	
	let game_over_text = text_creator.builder("Game over").game().build();
	let game_won_text = text_creator.builder("You won").game().build();
	
	let host_start_text = text_creator.builder("Press enter to start game")
		.with_wrap(window_rect.width() as u32).build();
	let add_player_text = text_creator.builder("Press q to add a player")
		.with_wrap(window_rect.width() as u32).build();
	let waiting_for_host_text = text_creator.builder("Waiting for host to start game...")
		.with_wrap(window_rect.width() as u32).build();
	
	let local_player_text = text_creator.builder(" (Local)").build();
	let network_player_text = text_creator.builder(" (Network)").build();
	
	let get_player_text = |player: &Player|{
		match player.kind {
			PlayerKind::Local {..} => &local_player_text,
			PlayerKind::Network {..} => &network_player_text,
		}
	};
	
	let fps: u32 = 60;
	let dpf: Duration = Duration::from_secs(1) / fps;
	
	let marathon_text = text_creator.builder("Marathon").build();
	let sprint_text = text_creator.builder("Sprint").build();
	let versus_text = text_creator.builder("Versus").build();
	let game_of_life_text = text_creator.builder("Game of life").build();
	let get_game_mode_text = |selected_game_mode: &GameModeSelection|
		match *selected_game_mode {
			GameModeSelection::Marathon => &marathon_text,
			GameModeSelection::Sprint => &sprint_text,
			GameModeSelection::Versus => &versus_text,
			GameModeSelection::GameOfLife => &game_of_life_text,
		};
	
	// NETWORK STATE
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
	
	let mut lines_cleared_text: Vec<_> = iter::from_fn(||Some(LinesClearedText::new(&text_creator, config.block_size_draw))).take(MAX_PLAYERS).collect();
	let mut level_text: Vec<_> = iter::from_fn(||Some(LevelText::new(&text_creator, config.block_size_draw))).take(MAX_PLAYERS).collect();
	
	let can_continue_text = text_creator.builder("Continue").build();
	let cant_continue_text = text_creator.builder("Continue").color(Color::GRAY).build();
	let mut saved_unit = load_saved_unit();
	
	let get_continue_text = |can_continue|{
		if can_continue {&can_continue_text}
		else {&cant_continue_text}
	};
	
	let mut quick_game = true;
	let new_game_text = text_creator.builder("New Game").build();
	let quick_game_text = text_creator.builder("Quick Game").build();
	
	let get_game_text = |quick_game|if quick_game {&quick_game_text} else {&new_game_text};
	
	let mut title_selection = TitleSelection::Continue;
	
	// PAUSE
	let paused_text = text_creator.builder("Paused").big().build();
	
	let resume_text = text_creator.builder("Resume").build();
	let save_text = text_creator.builder("Save").build();
	let saved_text = text_creator.builder("Saved ✓").build();
	let restart_text = text_creator.builder("Restart").build();
	let quit_to_title_text = text_creator.builder("Quit to title").build();
	let quit_to_desktop_text = text_creator.builder("Quit to desktop").build();
	
	let mut just_saved = false;
	let get_save_text = |just_saved|if just_saved {&saved_text} else {&save_text};
	
	let mut network_players = 0u32;
	let mut player_names_text = Vec::<Texture>::new();
	
	let mut state = State::Title;
	
	let menu_binds = MenuBinds {
		up:         Bind::new(Keycode::W, (Button::DPadUp).into()),
		down:       Bind::new(Keycode::S, (Button::DPadDown).into()),
		left:       Bind::new(Keycode::A, (Button::DPadLeft).into()),
		right:      Bind::new(Keycode::D, (Button::DPadRight).into()),
		ok:         Bind::new(Keycode::Return, (Button::A).into()),
		add_player: Bind::new(Keycode::Q, (Button::Back).into()), // definitely remove controller bind
		pause:      Bind::new(Keycode::Escape, (Button::Start).into()),
		restart:    Bind::new(Keycode::R, (Button::Back).into()), // maybe change/remove controller bind
	};
	
	let mut name_prompt = Prompt::new(&text_creator, "Name");
	let mut ip_prompt = Prompt::new(&text_creator, "IP");
	
	let mut adding_player = false;
	
	let mut room = Room::new();
	let mut commands = VecDeque::new();
	
	let mut player = Player::default();
	
	video_subsystem.text_input().stop();
	
	'running: loop {
		let start = Instant::now();
		
		// @input
		for event in event_pump.poll_iter() {
			if let Event::Quit {..} = event {break 'running}
			
			match event {
				// This here is to map the controllers in the way that I want.
				
				// unused_controller_ids[] is a set which contains all the unused
				// controller ids (here ids refers to my ids, and not sdl's ids).
				
				// controllers[] is a map, from the sdl controller id, into the data that
				// I store for each controller.
				Event::ControllerDeviceAdded {which,..} => {
					let controller = game_controller_subsystem.open(which);
					if let Ok(controller) = controller {
						let index = *unused_controller_ids.iter().next().unwrap();
						unused_controller_ids.remove(&index);
						controllers.insert(controller.instance_id(), (controller, index, [false;6]));
					}
				},
				Event::ControllerDeviceRemoved {which,..} => {
					let controller = controllers.remove(&which);
					if let Some((_,index,_)) = controller {
						unused_controller_ids.insert(index);
						
						let mut in_use = None;
						let which = index;
						for (player, p) in izip!(&room.players, 0..) {
							if let Player{kind:PlayerKind::Local(InputMethod{controller:Some(index),..}),..} = player {
								if *index == which {in_use = Some(p)}
							}
						}
						if let Some(p) = in_use {
							commands.push_back(RoomCommand::RemovePlayer(p).wrap());
						}
					}
				},
				
				// I couldn't understand the purpose of this event, and I've never managed
				// to get it to fire, so for now it's not gonna do anything.
				Event::ControllerDeviceRemapped {which,..} => println!("Remapped {:?}", which),
				
				// Converting sdl events to my events
				Event::ControllerButtonDown {timestamp, which, button} =>
				myevents::push_custom_event(&sdl_event, myevents::MyControllerButtonDown {timestamp,
				which: controllers[&which].1, button}).unwrap(),
				
				Event::ControllerButtonUp {timestamp, which, button} =>
				myevents::push_custom_event(&sdl_event, myevents::MyControllerButtonUp {timestamp,
				which: controllers[&which].1, button}).unwrap(),
				
				Event::ControllerAxisMotion {timestamp, which, axis, value} => {
					let (_,which,down) = controllers.get_mut(&which).unwrap();
					let which = *which;
					let down = &mut down[axis_to_usize(axis)];
					let should_be_down = value >= 4096;
					if *down != should_be_down {
						*down = should_be_down;
						if *down {
							myevents::push_custom_event(&sdl_event, myevents::MyControllerAxisDown{timestamp, which, axis})
						}else {
							myevents::push_custom_event(&sdl_event, myevents::MyControllerAxisUp{timestamp, which, axis})
						}.unwrap()
					}
				}
				
				_ => (),
			};

			// Some aliases to make things simpler
			let mb = &menu_binds;
			let im = InputMethod::new(true, Some(0));
			match state {
				State::Play {ref mut pause,..} => {
					
					if mb.pause.is_down(&event, &im) {
						*pause = if pause.is_some() {None} else {Some(Pause::default())};
						just_saved = false;
					}
					if mb.restart.is_down(&event, &im) {
						commands.push_back(RoomCommand::StartGame.wrap());
					}
					
					if let Some(Pause{selection}) = pause {
						*selection = prev_next_variant(
							*selection, &mb.up, &mb.down, &event, &im);
						if mb.ok.is_down(&event, &im) {
							match selection {
								PauseSelection::Resume => *pause = None,
								PauseSelection::Save => {
									if let NetworkState::Offline = network_state {
										use std::fs::File;
										use std::io::prelude::*;
										let mut file = File::create("save").unwrap();
										file.write_all(&serialize(&room.units[0]).unwrap()).unwrap();
										just_saved = true;
									}
								}
								PauseSelection::Restart => {
									commands.push_back(RoomCommand::StartGame.wrap());
								}
								PauseSelection::QuitToTitle => {
									state = State::Title;
									room.players.clear();
									player_names_text.clear();
									saved_unit = load_saved_unit();
								}
								PauseSelection::QuitToDesktop => {
									break 'running;
								}
							}
						}
					}else {
						for (unit, player) in izip!(&mut room.units, &room.players) {
							if let unit::Kind::Local{mino_controller,..} = &mut unit.kind {
								if let PlayerKind::Local(input_method) = &player.kind {
									mino_controller.update(&config.binds, input_method, &event);
								}
							}
						}
					}
				}
				State::Title => {
					title_selection = prev_next_variant(
						title_selection, &mb.up, &mb.down, &event, &im);
					
					use TitleSelection::*;
					match title_selection {
						Continue => {
							if mb.ok.is_down(&event, &im) {
								if selected_network_state == NetworkStateSelection::Offline {
									network_state = NetworkState::Offline;
									
									let unit = saved_unit.clone().unwrap();
									commands.push_back(RoomCommand::AddPlayer(Player::new(String::from(""), InputMethod::new(true, Some(0)))).wrap());
									commands.push_back(RoomCommand::StartGameFromSave(unit).wrap());
								}
							}
						},
						NewGame => {
							if mb.ok.is_down(&event, &im) {
								if quick_game {
									commands.push_back(RoomCommand::AddPlayer(Player::new(String::from(""), InputMethod::new(true, Some(0)))).wrap());
									commands.push_back(RoomCommand::StartGame.wrap());
								}else {
									match selected_network_state {
										NetworkStateSelection::Offline => {
											state = State::Lobby;
										}
										NetworkStateSelection::Host => {
											video_subsystem.text_input().start();
											state = State::PreLobby;
										}
										NetworkStateSelection::Client => {
											video_subsystem.text_input().start();
											state = State::PreLobby;
										}
									}
								}
							}
							if mb.left.is_down(&event, &im) ||
							mb.right.is_down(&event, &im) {
								quick_game = !quick_game;
							}
						},
						GameMode => {
							room.selected_game_mode = prev_next_variant(
								room.selected_game_mode, &mb.left, &mb.right, &event, &im);
						},
						NetworkMode => {
							selected_network_state = prev_next_variant(
								selected_network_state, &mb.left, &mb.right, &event, &im);
						},
					}
				}
				State::PreLobby => {
					ip_prompt.input(&event);
					if mb.ok.is_down(&event, &im) {
						let addr = string_to_addr(ip_prompt.text);
						ip_prompt = Prompt::new(&text_creator, "IP");
						network_state = match selected_network_state {
							NetworkStateSelection::Offline => {NetworkState::Offline} // This should never happen
							NetworkStateSelection::Host => {
								let listener = TcpListener::bind(addr).unwrap();
								listener.set_nonblocking(true).unwrap();
								
								NetworkState::Host {
									listener,
									streams: Vec::new(),
								}
							}
							NetworkStateSelection::Client => {
								let stream = TcpStream::connect(addr).unwrap();
								stream.set_nonblocking(true).unwrap();
								let stream = LenIO::new(stream);
								println!("Connection to host established");
								
								NetworkState::Client {
									stream,
								}
							}
						};
						video_subsystem.text_input().stop();
						state = State::Lobby;
					}
				}
				State::Lobby => {
					if adding_player {
						name_prompt.input(&event);
						if mb.ok.is_down(&event, &im) {
							player.name = name_prompt.text;
							commands.push_back(RoomCommand::AddPlayer(player).wrap());
							player = Player::default();
							name_prompt = Prompt::new(&text_creator, "Name");
							adding_player = false;
							video_subsystem.text_input().stop();
						}
					}else {
						if let NetworkState::Host {..} | NetworkState::Offline = network_state {
							if mb.ok.is_down(&event, &im) {
								commands.push_back(RoomCommand::StartGame.wrap());
							}
						}
						if let Some(myevents::MyControllerButtonDown {which, ..}) = myevents::as_user_event_type::<_>(&event) {
							let mut not_in_use = true;
							for player in &room.players {
								if let Player{kind:PlayerKind::Local(InputMethod{controller:Some(index),..}),..} = player {
									if *index == which {not_in_use = false}
								}
							}
							if not_in_use {
								adding_player = true;
								video_subsystem.text_input().start();
								player.kind = PlayerKind::Local(InputMethod::new(false, Some(which)));
							}
						}else if mb.add_player.is_down(&event, &im) {
							adding_player = true;
							video_subsystem.text_input().start();
							player.kind = PlayerKind::Local(InputMethod::new(true, None));
						}
					}
				}
			}
			
			// IMPORTANT! These functions must always stay at the bottom of the event
			// loop. Their purpose is to clean up the memory used up by the event, in
			// case it is a user event. More details in myevents.rs.
			myevents::drop_if_user_event::<myevents::MyControllerAxisDown>(&event);
			myevents::drop_if_user_event::<myevents::MyControllerAxisUp>(&event);
			myevents::drop_if_user_event::<myevents::MyControllerButtonDown>(&event);
			myevents::drop_if_user_event::<myevents::MyControllerButtonUp>(&event);
		}
		
		// @network
		let mut network_command_pump = crate::network::NetworkPump::new();
		
		while let Some(command) = network_command_pump.poll(&mut network_state) {
			match command {
				NetworkCommand::RoomCommand(command) =>
				commands.push_back(command),
				NetworkCommand::UnitCommand(command) =>
				room.commands[command.inner.0].push_back(command.map(|c|c.1)),
			}
		}
		
		if let (State::Lobby, NetworkState::Host {listener, streams}) =
		(&state, &mut network_state) {
			while let Ok(incoming) = listener.accept() {
				network_players += 1;
				let mut stream = LenIO::new(incoming.0);
				
				stream.write(
					&serialize(
						&NetworkCommand::from(RoomCommand::Init(room.clone()).wrap())
					).unwrap()
				).unwrap();
				
				streams.push(stream);
				println!("{:?}", incoming.1);
				println!("Connection to client established");
			}
		}
		
		// @update
		
		// ROOM
		while let Some(command) = commands.pop_front() {
			command.execute(&mut network_state, |c|commands.push_back(c), (&mut room, &mut state));
			if room.just_added_player {
				player_names_text.push(text_creator.builder(&room.players.last().unwrap().name).build());
			}
			if room.just_initted {
				for player in &room.players {
					player_names_text.push(text_creator.builder(&player.name).build());
				}
			}
			if room.just_started {
				for (unit, lines_cleared_text, level_text) in
				izip!(&room.units, &mut lines_cleared_text, &mut level_text) {
					lines_cleared_text.update(unit.base.lines_cleared);
					if let Mode::Marathon {level,..} = &unit.base.mode {level_text.update(*level)}
				}
			}
			if let Some(index) = room.just_removed_player {
				player_names_text.remove(index);
			}
			room.reset_flags();
		}
		
		// UNITS
		if let State::Play {over,pause,players_lost,players_won,winner,..} = &mut state {
			for unit_id in 0..room.units.len() {
				let unit = &mut room.units[unit_id];
				if let unit::State::Animation {countdown} = &mut unit.base.state {
					*countdown += dpf;
					if unit.base.lc_animation.is_some() {
						if *countdown >= *LINE_CLEAR_DURATION {
							unit.base.state = unit::State::Play;
						}
					}else if unit.base.gol_animation.is_some() {
						if *countdown >= *GAME_OF_LIFE_DURATION {
							unit.base.state = unit::State::Play;
						}
					}
				}
				
				let players = room.players.len() as u32;
				match room.selected_game_mode {
					GameModeSelection::Marathon | GameModeSelection::Sprint | GameModeSelection::GameOfLife =>
					if *players_won == players {*over = true}
					GameModeSelection::Versus =>
					if *players_lost == players-1 {
						*over = true;
						for (player, unit) in izip!(&room.players, &room.units) {
							if let unit::State::Play = unit.base.state {
								winner.get_or_insert(player.name.clone());
							}
						}
					}
				}
			}
			
			let not_paused = !pause.is_some() || network_players > 0;
			if not_paused {
				for (unit_id, unit) in izip!(0.., &mut room.units) {
					if let unit::Kind::Local {mino_controller,..} = &mut unit.kind {
						if let unit::State::Play = unit.base.state {
							mino_controller.append_commands(&mut room.commands[unit_id], &config.players, dpf);
						}
					}
				}
				
				// We loop as long as there are new commands
				let mut keep_looping = true;
				while keep_looping {
					keep_looping = false;
					for (unit_id, unit) in izip!(0usize.., &mut room.units) {
						let commands = &mut room.commands;
						
						// This while loop is ugly. Refactor it when this
						// https://github.com/rust-lang/rust/issues/53667 gets added
						while !commands[unit_id].is_empty() && matches!(unit.base.state, unit::State::Play) {
							keep_looping = true;
							let command = commands[unit_id].pop_front().unwrap();
							let command = command.map(|c|(unit_id, c));
							// println!("{:?}", command);
							let append = |c: command::CommandWrapper<unit::UnitCommandInner>|commands[c.inner.0].push_back(c.map(|c|c.1));
							command.execute(&mut network_state, append, unit);
						}
					}
				}
				
				for (unit, lines_cleared_text, level_text, player) in
				izip!(&mut room.units, &mut lines_cleared_text, &mut level_text, &room.players) {
					if unit.base.just_cleared_lines {
						lines_cleared_text.update(unit.base.lines_cleared);
					}
					if unit.base.just_changed_mino {
						if let unit::Kind::Local {mino_controller,..} = &mut unit.kind {
							mino_controller.fall_countdown = Duration::from_secs(0);
						}
					}
					if unit.base.just_changed_level {
						if let Mode::Marathon {level,..} = &unit.base.mode {
							if let unit::Kind::Local {mino_controller,..} = &mut unit.kind {
								mino_controller.fall_duration = unit::get_level_fall_duration(*level);
							}
							level_text.update(*level);
						}
					}
					if unit.base.just_lost {*players_lost += 1}
					if unit.base.just_won {*players_won += 1}
					match room.selected_game_mode {
						GameModeSelection::Marathon | GameModeSelection::Sprint =>
						if unit.base.just_won {winner.get_or_insert(player.name.clone());}
						_ => {}
					}
					unit.base.reset_flags();
				}
			}
		}
		
		
		// @draw
		
		canvas.set_draw_color(Color::BLACK);
		canvas.clear();
		match state {
			State::Title => {
				let mut layout = CenteredLayout {y:0,width:window_rect.width()};
				
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
				select(&mut canvas, rect, matches!(title_selection, TitleSelection::Continue));
				
				layout.row(height as i32);
				layout.row_margin(15);
				
				let game_text = get_game_text(quick_game);
				let (width, height) = get_texture_dim(&game_text);
				let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
				draw_same_scale(&mut canvas, &game_text, rect);
				select(&mut canvas, rect, matches!(title_selection, TitleSelection::NewGame));
				
				layout.row(height as i32);
				layout.row_margin(15);
				
				let game_mode_text = get_game_mode_text(&room.selected_game_mode);
				let (width, height) = get_texture_dim(&game_mode_text);
				let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
				draw_same_scale(&mut canvas, &game_mode_text, rect);
				select(&mut canvas, rect, matches!(title_selection, TitleSelection::GameMode));
				
				if !quick_game {
					layout.row(height as i32);
					layout.row_margin(15);
					
					let network_text = get_network_text(&selected_network_state);
					let (width, height) = get_texture_dim(&network_text);
					let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
					draw_same_scale(&mut canvas, &network_text, rect);
					select(&mut canvas, rect, matches!(title_selection, TitleSelection::NetworkMode));
				}
			}
			State::PreLobby => {
				ip_prompt.draw(&mut canvas);
			}
			State::Lobby {..} => {
				if adding_player {
					name_prompt.draw(&mut canvas);
				}else {
					let mut y = 0;
					
					match &network_state {
						NetworkState::Host {..} | NetworkState::Offline {..} => {
							let (width, height) = get_texture_dim(&host_start_text);
							let rect = Rect::new(0, y, width, height);
							draw_same_scale(&mut canvas, &host_start_text, rect);
							
							y += height as i32;
						}
						NetworkState::Client {..} => {
							let (width, height) = get_texture_dim(&waiting_for_host_text);
							let rect = Rect::new(0, y, width, height);
							draw_same_scale(&mut canvas, &waiting_for_host_text, rect);
							
							y += height as i32;
						}
					}
					
					let (width, height) = get_texture_dim(&add_player_text);
					let rect = Rect::new(0, y, width, height);
					draw_same_scale(&mut canvas, &add_player_text, rect);
					
					y += height as i32;
					
					for (player, name_text) in izip!(&room.players, &player_names_text) {
						let mut x = 0;
						
						let (width, height) = get_texture_dim(&name_text);
						let rect = Rect::new(x, y, width, height);
						draw_same_scale(&mut canvas, &name_text, rect);
						
						x += width as i32;
						
						let player_text = get_player_text(player);
						let (width, height) = get_texture_dim(&player_text);
						let rect = Rect::new(x, y, width, height);
						draw_same_scale(&mut canvas, &player_text, rect);
						
						y += height as i32;
					}
				}
			}
			State::Play {pause,..} => {
				let bs = config.block_size_draw as i32;
				let hbs = bs/2;
				
				let mut layout = GameLayout {
					x:0, y:0,
					width: window_rect.width() as i32,
					expected_width: (4*bs+hbs+10*bs+hbs+4*bs+hbs) * room.units.len() as i32 - hbs
				};
				
				for (unit, lines_cleared_text, level_text)
				in izip!(&mut room.units, &lines_cleared_text, &level_text) {
					let Unit {base: unit::Base {stored_mino, falling_mino, well, state, mode, gol_animation, lc_animation, ..}, kind} = unit;
					
					layout.row_margin(hbs);
					
					if let Some(ref stored_mino) = stored_mino {
						block_canvas.draw_mino_centered(&mut canvas, layout.as_vec2i(), stored_mino, vec2i!(4,3));
					}
					layout.row(3*bs);
					layout.row_margin(hbs);
					
					let (width, height) = get_texture_dim(&lines_cleared_text.0);
					let rect = Rect::new(layout.x(), layout.y(), width, height);
					draw_same_scale(&mut canvas, &lines_cleared_text.0, rect);
					
					layout.row(height as i32);
					layout.row_margin(hbs);
					
					if let Mode::Marathon {..} = mode {
						let (width, height) = get_texture_dim(&level_text.0);
						let rect = Rect::new(layout.x(), layout.y(), width, height);
						draw_same_scale(&mut canvas, &level_text.0, rect);
					}
					
					layout.col(4*bs);
					layout.col_margin(hbs);
					
					if let Mode::Versus {lines_received_sum,..} = mode {
						layout.row_margin(hbs);
						for y in 0..well.num_columns() {
							let data = if well.num_columns()-y > *lines_received_sum as usize {
								block::Data::EMPTY_LINE
							}else {
								block::Data::SENT_LINE
							};
							block_canvas.draw_block(&mut canvas, layout.as_vec2i(), &vec2i!(0,y), &data);
						}
						layout.col(bs);
					}
					
					layout.row_margin(hbs);
					
					let well_rect = Rect::new(
						layout.x(), layout.y(),
						well.num_rows() as u32 * bs as u32,
						well.num_columns() as u32 * bs as u32,
					);
					
					let countdown = if let unit::State::Animation {countdown} = state {*countdown} else {Duration::from_secs(0)};
					
					block_canvas.draw_well(&mut canvas, layout.as_vec2i(), &well, &lc_animation, &gol_animation, countdown);
					if let Some(falling_mino) = falling_mino {
						let shadow_mino = game::create_shadow_mino(falling_mino, &well);
						block_canvas.draw_mino(&mut canvas, layout.as_vec2i(), &shadow_mino);
						
						let do_ease = game::may_down_mino(&falling_mino, &well);
						let mut f = 0f32;
						if do_ease {
							if let unit::Kind::Local {mino_controller,..} = kind {
								f = mino_controller.fall_countdown.as_secs_f32() / mino_controller.fall_duration.as_secs_f32();
								f = f.clamp(0f32, 1f32);
								f = f*f*f*f*f*f;
							}
						};
						
						block_canvas.draw_mino(&mut canvas, layout.as_vec2i() + vec2i!(0, (f*bs as f32) as i32), falling_mino);
					}
					
					layout.col(10*bs);
					layout.col_margin(hbs);
					
					layout.row_margin(hbs);
					if let unit::Kind::Local {rng: unit::LocalMinoRng {queue,..},..} = &unit.kind {
						for mino in queue.iter() {
							block_canvas.draw_mino_centered(&mut canvas, layout.as_vec2i(), mino, vec2i!(4,3));
							layout.row(3*bs);
							layout.row_margin(hbs);
						}
						
						layout.col(4*bs);
						layout.col_margin(hbs);
					}
					
					match state {
						unit::State::Win => {
							darken(&mut canvas, Some(well_rect));
							draw_centered(&mut canvas, &game_won_text, well_rect);
						}
						unit::State::Lose => {
							darken(&mut canvas, Some(well_rect));
							draw_centered(&mut canvas, &game_over_text, well_rect);
						}
						_ => {}
					}
				}
				
				if let Some(Pause{selection}) = pause {
					darken(&mut canvas, None);
					
					let mut layout = CenteredLayout {y:0,width:window_rect.width()};
					
					let (width, height) = get_texture_dim(&paused_text);
					let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
					draw_same_scale(&mut canvas, &paused_text, rect);
					
					layout.row(height as i32);
					layout.row_margin(hbs);
					
					let (width, height) = get_texture_dim(&resume_text);
					let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
					draw_same_scale(&mut canvas, &resume_text, rect);
					select(&mut canvas, rect, matches!(selection, PauseSelection::Resume));
					
					layout.row(height as i32);
					layout.row_margin(hbs);
					
					let save_text = get_save_text(just_saved);
					let (width, height) = get_texture_dim(&save_text);
					let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
					draw_same_scale(&mut canvas, &save_text, rect);
					select(&mut canvas, rect, matches!(selection, PauseSelection::Save));
					
					layout.row(height as i32);
					layout.row_margin(hbs);
					
					let (width, height) = get_texture_dim(&restart_text);
					let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
					draw_same_scale(&mut canvas, &restart_text, rect);
					select(&mut canvas, rect, matches!(selection, PauseSelection::Restart));
					
					layout.row(height as i32);
					layout.row_margin(hbs);
					
					let (width, height) = get_texture_dim(&quit_to_title_text);
					let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
					draw_same_scale(&mut canvas, &quit_to_title_text, rect);
					select(&mut canvas, rect, matches!(selection, PauseSelection::QuitToTitle));
					
					layout.row(height as i32);
					layout.row_margin(hbs);
					
					let (width, height) = get_texture_dim(&quit_to_desktop_text);
					let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
					draw_same_scale(&mut canvas, &quit_to_desktop_text, rect);
					select(&mut canvas, rect, matches!(selection, PauseSelection::QuitToDesktop));
					
					layout.row(height as i32);
					layout.row_margin(hbs);
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