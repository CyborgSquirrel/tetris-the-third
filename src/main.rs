// #![windows_subsystem = "windows"]

use sdl2::{controller::Button, render::{BlendMode, TextureQuery}};
use sdl2::render::WindowCanvas;
use sdl2::{event::Event, render::Texture};
use sdl2::image::LoadTexture;
use sdl2::keyboard::Keycode;
use std::{collections::VecDeque, net::SocketAddr, time::{Duration, Instant}};
use std::thread::sleep;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use slotmap::{DefaultKey, SlotMap};
use std::collections::BTreeMap;
use config::{InputMethod,Bind,MenuBinds};
use lazy_static::lazy_static;

type UnitCommand = crate::unit::Command;

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
use vec2::{vec2i,vec2f};
use text::TextCreator;
use config::Config;
use unit::{Unit, Mode};

use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use lenio::LenIO;

use serde::{Serialize,Deserialize};
use bincode::{serialize,deserialize};

use mino::Mino;

use mino_controller::MinoController;

use ui::{EnumSelect, GameModeSelection, Layout, NetworkStateSelection, Pause, PauseSelection, StartLayout, TitleSelection};

enum State {
	Play {
		_players_won: u32,
		players_lost: u32,
		over: bool,
		pause: Option<Pause>,
	},
	Title,
	PreLobby,
	Lobby,
}

impl State {
	fn play() -> Self {
		State::Play {
			_players_won: 0,
			players_lost: 0,
			over: false,
			pause: None,
		}
	}
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
	Client {
		stream: LenIO<TcpStream>,
	},
	Host {
		listener: TcpListener,
		streams: Vec<LenIO<TcpStream>>,
	},
}


impl NetworkState {
	fn broadcast_command(&mut self, command: &NetworkCommand) {
		use NetworkState::*;
		match self {
			Offline => {},
			Client {stream} => {
				stream.write(&serialize(command).unwrap()).unwrap();
			}
			Host {streams,..} => {
				let command = &serialize(command).unwrap();
				for stream in streams {
					stream.write(command).unwrap();
				}
			}
		}
	}
}

struct NetworkCommandPump {
	stream_index: usize,
}

impl NetworkCommandPump {
	fn new() -> NetworkCommandPump {
		NetworkCommandPump {stream_index: 0}
	}
	fn poll_command(&mut self, state: &mut NetworkState) -> Option<NetworkCommand> {
		let Self {stream_index} = self;
		match state {
			NetworkState::Offline => None,
			NetworkState::Host {streams,..} => {
				while *stream_index < streams.len() {
					let (before, after) = streams.split_at_mut(*stream_index);
					if let Some((stream, after)) = after.split_first_mut() {
						if let Ok(serialized) = stream.read() {
							if let Ok(deserialized) = deserialize::<NetworkCommand>(serialized) {
								for stream in before.iter_mut().chain(after.iter_mut()) {
									stream.write(serialized).unwrap();
								}
								return Some(deserialized);
							}else {*stream_index += 1;}
						}else {*stream_index += 1;}
					}
				}
				None
			}
			NetworkState::Client {stream} => {
				stream.read().ok().and_then(|serialized|deserialize::<NetworkCommand>(serialized).ok())
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
enum PlayerKind {
	Local,
	Network,
}

impl Default for PlayerKind {
	fn default() -> Self {
		PlayerKind::Network
	}
}

#[derive(Debug,Clone,Serialize,Deserialize)]
struct Player {
	#[serde(skip)]kind: PlayerKind,
	name: String,
}

// impl Player {
// 	fn local(name: String) -> Player {
// 		Player {
// 			kind: PlayerKind::Local,
// 			name,
// 		}
// 	}
// 	fn network(name: String) -> Player {
// 		Player {
// 			kind: PlayerKind::Network,
// 			name,
// 		}
// 	}
// }

#[derive(Serialize, Deserialize)]
enum NetworkCommand {
	Room(Command),
	Unit(usize,UnitCommand),
}

// enum RoomState {Lobby, Game}

#[derive(Default, Serialize, Deserialize, Clone)]
struct Room {
	selected_game_mode: GameModeSelection,
	players: Vec<Player>,
	units: Vec<Unit>,
	commands: VecDeque<(usize, UnitCommand)>,
}
#[derive(Serialize, Deserialize, Clone)]
enum Command {
	Init(Room),
	StartGame,
	StartGameFromSave(Unit),
	AddPlayer(String),
}
impl Command {
	fn execute(self, room: &mut Room, network_state: &mut NetworkState, state: &mut State) {
		let command = NetworkCommand::Room(self.clone());
		network_state.broadcast_command(&command);
		match self {
			Command::Init(init_room) => {
				*room = init_room;
			}
			Command::StartGame => {
				room.units.clear();
				let mut configs = (0..4usize).cycle();
				*state = State::play();
				let players_len = room.players.len();
				for (unit_id, player) in izip!(0.., &room.players) {
					let mut unit = match player.kind {
						PlayerKind::Local => Unit::local(room.selected_game_mode.ctor()(), MinoController::new(configs.next().unwrap(), None)),
						PlayerKind::Network => Unit::network(room.selected_game_mode.ctor()()),
					};
					let Unit{kind, base} = &mut unit;
					
					if let Mode::Versus {target_unit_id,..} = &mut base.mode {
						*target_unit_id = (unit_id+1usize).rem_euclid(players_len);
					}
					
					if let unit::Kind::Local{rng,..} = kind {
						UnitCommand::NextMino(rng.next_mino_centered_bro(&base.well))
							.execute(&mut unit, unit_id, &mut room.commands, network_state);
					}
					
					room.units.push(unit);
				}
			}
			Command::StartGameFromSave(unit) => {
				*state = State::play();
				room.units.push(unit);
			}
			Command::AddPlayer (name) => {
				room.players.push(Player{name, kind: PlayerKind::Local});
			}
		}
	}
}
fn prev_next_variant<T: EnumSelect>(mut value: T, prev: &Bind, next: &Bind, event: &Event, input_method: &InputMethod) -> T {
	if prev.is_down(event, input_method) {value = value.prev_variant()}
	if next.is_down(event, input_method) {value = value.next_variant()}
	value
}

lazy_static! {
	static ref SOFTDROP_DURATION: Duration = Duration::from_secs_f64(0.05);
	static ref LINE_CLEAR_DURATION: Duration = Duration::from_secs_f64(0.1);
}

fn main() {
	let sdl_context = sdl2::init()
		.expect("Failed to initialize sdl2");
	let video_subsystem = sdl_context.video()
		.expect("Failed to initialize video subsystem");
	let game_controller_subsystem = sdl_context.game_controller()
		.expect("Failed to initialize controller subsystem");
	let ttf_context = sdl2::ttf::init()
		.expect("Failed to initialize ttf");
	
	let mut unordered_controllers = BTreeMap::new();
	let num_joysticks = game_controller_subsystem.num_joysticks()
		.expect("Couldn't enumerate joysticks");
	for i in 0..num_joysticks {
		let controller = game_controller_subsystem.open(i as u32);
		if let Ok(controller) = controller {
			println!("{:?}", controller.instance_id());
			unordered_controllers.insert(controller.instance_id(), controller);
		}
	}
	
	let mut controllers: [Option<u32>;8] = [None;8];
	for (i, (instance_id, _)) in izip!(0.., unordered_controllers.iter().rev()) {
		controllers[i] = Some(*instance_id);
	}
	
	let config = Config::from_file();
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
	let big_font = ttf_context.load_font("gfx/IBMPlexMono-Regular.otf", 128)
		.expect("Failed to load font");
	
	let text_creator = TextCreator::new(&texture_creator, &font, &big_font);
	
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
	
	let get_player_text = |player: &Player|{
		match player.kind {
			PlayerKind::Local {..} => &local_player_text,
			PlayerKind::Network {..} => &network_player_text,
		}
	};
	
	let fps: u32 = 60;
	let dpf: Duration = Duration::from_secs(1) / fps;
	
	let mut selected_game_mode = GameModeSelection::Marathon;
	let marathon_text = text_creator.builder("Marathon").build();
	let sprint_text = text_creator.builder("Sprint").build();
	let versus_text = text_creator.builder("Versus").build();
	let get_game_mode_text = |selected_game_mode: &GameModeSelection|
		match *selected_game_mode {
			GameModeSelection::Marathon => &marathon_text,
			GameModeSelection::Sprint => &sprint_text,
			GameModeSelection::Versus => &versus_text,
		};
	
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
	
	let mut title_selection = TitleSelection::Continue;
	
	let resume_text = text_creator.builder("Resume").build();
	let save_text = text_creator.builder("Save").build();
	let quit_to_title_text = text_creator.builder("Quit to title").build();
	let quit_to_desktop_text = text_creator.builder("Quit to desktop").build();
	
	let mut network_players = 0u32;
	// let mut players = SlotMap::<DefaultKey, Player>::new();
	// let mut player_keys = Vec::<DefaultKey>::new();
	let mut player_names_text = Vec::<Texture>::new();
	
	let line_clear_duration = Duration::from_secs_f64(0.1);
	
	let block_canvas = block::Canvas::new(&texture, config.block_size);
	
	let mut state = State::Title;
	
	let menu_binds = MenuBinds {
		up: Bind::new(Keycode::W, Button::DPadUp),
		down: Bind::new(Keycode::S, Button::DPadDown),
		left: Bind::new(Keycode::A, Button::DPadLeft),
		right: Bind::new(Keycode::D, Button::DPadRight),
		ok: Bind::new(Keycode::Return, Button::A),
		other: Bind::new(Keycode::Q, Button::B),
	};
	
	let mut name_prompt = Prompt::new(&text_creator, "Name");
	let mut ip_prompt = Prompt::new(&text_creator, "IP");
	
	let mut adding_player = false;
	
	let mut room = Room::default();
	// let mut commands = VecDeque::<Command>::new();
	
	video_subsystem.text_input().stop();
	
	'running: loop {
		let start = Instant::now();
		
		// @input
		for event in event_pump.poll_iter() {
			match event {
				Event::Quit{..} => break 'running,
				
				// This here is to map the controllers in the way that I want.
				
				// controllers[] maps my controller indexing to sdl's controller indexing.
				// Whenever a controller is added, its id is inserted at the first empty
				// space in controllers[]. Whenever a controller is removed, its id is
				// found, and removed from controllers[].
				
				// unordered_controllers[] contains all the controllers, indexed with
				// their sdl ids. Its purpose is to store the controller objects. If I
				// don't store them somewhere, then the controller events won't be fired.
				Event::ControllerDeviceAdded{which,..} => {
					let controller = game_controller_subsystem.open(which);
					if let Ok(controller) = controller {
						let first_none = controllers.iter_mut()
							.find(|instance_id|instance_id.is_none());
						if let Some(first_none) = first_none {
							*first_none = Some(controller.instance_id());
						}
						unordered_controllers.insert(controller.instance_id(), controller);
					}
				},
				Event::ControllerDeviceRemoved{which,..} => {
					unordered_controllers.remove(&which);
					let instance_id = controllers.iter_mut()
						.find(|instance_id|instance_id.map_or(false, |instance_id|instance_id==which));
					if let Some(instance_id) = instance_id {
						*instance_id = None;
					}
				},
				
				// I couldn't understand the purpose of this event, and I've never managed
				// to get it to fire, so for now it's not gonna do anything.
				Event::ControllerDeviceRemapped{which,..} => println!("Remapped {:?}", which),
				
				_ => (),
			};

			// Some aliases to make things simpler
			let mb = &menu_binds;
			let im = InputMethod::new(true, controllers[0]);
			match state {
				State::Play {ref mut pause,..} => {
					
					match event {
						// Deliberately not adding custom pause keybind
						Event::KeyDown{keycode: Some(Keycode::Escape),repeat: false,..} |
						Event::ControllerButtonDown{button: sdl2::controller::Button::Start,..} => {
							*pause = if pause.is_some() {None} else {Some(Pause::default())};
							just_saved = false;
						}
						
						// Restarting is broken TODO: fix this
						Event::KeyDown{keycode: Some(Keycode::R),repeat: false,..} => {
							// for unit in &mut room.units {
							// 	unit.base = unit::Base::new(selected_game_mode.ctor()());
							// }
							// network_state.broadcast_event(&NetworkEvent::RestartGame);
							// start_game(&mut state, selected_game_mode, &players, &mut network_state, &mut units);
						}
						
						_ => ()
					};
					
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
								PauseSelection::QuitToTitle => {
									state = State::Title;
								}
								PauseSelection::QuitToDesktop => {
									break 'running;
								}
							}
						}
					}else {
						for unit in &mut room.units {
							if let unit::Kind::Local{mino_controller,..} = &mut unit.kind {
								mino_controller.update(&config.binds, &event);
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
									
									let new_controller = MinoController::new(configs.next().unwrap(),Some(0));
									let mut unit = saved_unit.clone().unwrap();
									if let unit::Kind::Local{mino_controller,..} = &mut unit.kind {
										*mino_controller = new_controller;
									}
									lines_cleared_text[0] = create_lines_cleared_text(unit.base.lines_cleared, &text_creator);
									if let Unit{base:unit::Base{mode:Mode::Marathon{level,..},..},..} = unit {
										level_text[0] = create_level_text(level, &text_creator);
									}
									Command::StartGameFromSave(unit).execute(&mut room, &mut network_state, &mut state);
								}
							}
						},
						NewGame => {
							if mb.ok.is_down(&event, &im) {
								if quick_game {
									// let key = players.insert(Player::local("no name".into()));
									// player_keys.push(key);
									
									Command::AddPlayer(String::from("")).execute(&mut room, &mut network_state, &mut state);
									Command::StartGame.execute(&mut room, &mut network_state, &mut state);
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
							selected_game_mode = prev_next_variant(
								selected_game_mode, &mb.left, &mb.right, &event, &im);
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
							let name = name_prompt.text;
							player_names_text.push(text_creator.builder(&name).build());
							Command::AddPlayer(name).execute(&mut room, &mut network_state, &mut state);
							name_prompt = Prompt::new(&text_creator, "Name");
							adding_player = false;
							video_subsystem.text_input().stop();
						}
					}else  {
						if let NetworkState::Host {..} | NetworkState::Offline = network_state {
							if mb.ok.is_down(&event, &im) {
								Command::StartGame.execute(&mut room, &mut network_state, &mut state);
							}
						}
						if mb.other.is_down(&event, &im) {
							adding_player = true;
							video_subsystem.text_input().start();
						}
					}
				}
			}
		}
		
		// @network
		let mut network_command_pump = NetworkCommandPump::new();
		
		while let Some(command) = network_command_pump.poll_command(&mut network_state) {
			match command {
				NetworkCommand::Room(command) =>
				command.execute(&mut room, &mut network_state, &mut state),
				NetworkCommand::Unit(unit_id, command) =>
				room.commands.push_back((unit_id, command)),
			}
		}
		
		if let (State::Lobby, NetworkState::Host {listener, streams}) =
		(&state, &mut network_state) {
			while let Ok(incoming) = listener.accept() {
				network_players += 1;
				let mut stream = LenIO::new(incoming.0);
				
				stream.write(
					&serialize(
						&NetworkCommand::Room(Command::Init(room.clone()))
					).unwrap()
				).unwrap();
				
				streams.push(stream);
				println!("{:?}", incoming.1);
				println!("Connection established");
			}
		}
		
		// @update
		if let State::Play {over,pause,..} = &mut state {
			for unit_id in 0..room.units.len() {
				let unit = &mut room.units[unit_id];
				match &mut unit.base.state {
					unit::State::Play => {}
					unit::State::LineClear{countdown} => {
						*countdown += dpf;
						if *countdown >= line_clear_duration {
							room.commands.push_back((unit_id, UnitCommand::ClearLines));
						}
					}
					unit::State::Lose => {}
					unit::State::Win => {}
				}
				
				match selected_game_mode {
					GameModeSelection::Marathon => {
						
					}
					GameModeSelection::Sprint => {
						
					}
					GameModeSelection::Versus => {
						// if *players_lost as usize == players.len()-1 {
						// 	*over = true;
						// }
					}
				}
			}
			
			if !*over {
				let not_paused = !pause.is_some() || network_players > 0;
				if not_paused {
					for (unit_id, unit) in izip!(0.., &mut room.units) {
						if let unit::Kind::Local{mino_controller,..} = &mut unit.kind {
							mino_controller.append_commands(unit_id, &mut room.commands, &config.players, dpf);
						}
					}
					
					while let Some(command) = room.commands.pop_front() {
						command.1.execute(&mut room.units[command.0], command.0, &mut room.commands, &mut network_state);
					}
					
					for (unit, lines_cleared_text, level_text) in
					izip!(&mut room.units, &mut lines_cleared_text, &mut level_text) {
						if unit.base.just_cleared_lines {
							*lines_cleared_text =
							create_lines_cleared_text(unit.base.lines_cleared, &text_creator);
						}
						if unit.base.just_changed_mino {
							if let unit::Kind::Local{mino_controller,..} = &mut unit.kind {
								mino_controller.fall_countdown = Duration::from_secs(0);
							}
						}
						if unit.base.just_changed_level {
							if let Mode::Marathon {level,..} = &unit.base.mode {
								if let unit::Kind::Local {mino_controller,..} = &mut unit.kind {
									mino_controller.fall_duration = unit::get_level_fall_duration(*level);
								}
								*level_text =
								create_level_text(*level, &text_creator);
							}
						}
						unit.base.reset_flags();
					}
				}
			}
		}
		
		
		// @draw
		
		canvas.set_draw_color(Color::BLACK);
		canvas.clear();
		match state {
			State::Title => {
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
				select(&mut canvas, rect, matches!(title_selection, TitleSelection::Continue));
				
				layout.row(height as i32);
				layout.row_margin(15);
				
				let game_text = get_game_text(quick_game);
				let (width, height) = get_texture_dim(&game_text);
				let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
				draw_same_scale(&mut canvas, &game_text, rect);
				select(&mut canvas, rect, matches!(title_selection, TitleSelection::NewGame));
				
				if !quick_game {
					layout.row(height as i32);
					layout.row_margin(15);
					
					let game_mode_text = get_game_mode_text(&selected_game_mode);
					let (width, height) = get_texture_dim(&game_mode_text);
					let rect = Rect::new(layout.centered_x(width), layout.y, width, height);
					draw_same_scale(&mut canvas, &game_mode_text, rect);
					select(&mut canvas, rect, matches!(title_selection, TitleSelection::GameMode));
					
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
					if let NetworkState::Host {..} | NetworkState::Offline = network_state {
						let TextureQuery {width, height, ..} = host_start_text.query();
						let _ = canvas.copy(
							&host_start_text,
							Rect::new(0, 0, width, height),
							Rect::new(0, 0, width, height));
					}
					
					for (i, player, name_text) in izip!(0..,&room.players,&player_names_text) {
						let player_text = get_player_text(player);
						
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
				}
			}
			State::Play {pause,..} => {
			
				let mut layout = Layout {
					x:0,y:0,
					width:window_rect.width() as i32,
					expected_width:(4*config.block_size as i32+15+10*config.block_size as i32+15+4*config.block_size as i32+15) * room.units.len() as i32 - 15
				};
				
				for (unit, lines_cleared_text, level_text)
				in izip!(&mut room.units, &lines_cleared_text, &level_text) {
					let Unit {base: unit::Base {stored_mino, falling_mino, well, animate_line, state, mode, ..}, ..} = unit;
					
					layout.row_margin(15);
					
					if let Some(ref stored_mino) = stored_mino {
						block_canvas.draw_mino_centered(&mut canvas, layout.as_vec2i(), stored_mino, vec2i!(4,3));
					}
					layout.row(3*config.block_size as i32);
					layout.row_margin(config.block_size as i32 / 2);
					
					let TextureQuery {width, height, ..} = lines_cleared_text.query();
					let rect = Rect::new(layout.x(), layout.y(), width, height);
					draw_same_scale(&mut canvas, &lines_cleared_text, rect);
					
					layout.row(32*2);
					layout.row_margin(15);
					
					let TextureQuery {width, height, ..} = level_text.query();
					let rect = Rect::new(layout.x(), layout.y(), width, height);
					draw_same_scale(&mut canvas, &level_text, rect);
					
					layout.col(4*config.block_size as i32);
					layout.col_margin(15);
					
					if let Mode::Versus {lines_received_sum,..} = mode {
						layout.row_margin(15);
						for y in 0..well.num_columns() {
							let data = if well.num_columns()-y > *lines_received_sum as usize {
								block::Data::EMPTY_LINE
							}else {
								block::Data::SENT_LINE
							};
							block_canvas.draw_block(&mut canvas, layout.as_vec2i(), &vec2i!(0,y), &data);
						}
						layout.col(config.block_size as i32);
					}
					
					layout.row_margin(15);
					
					let well_rect = Rect::new(
						layout.x(), layout.y(),
						well.num_rows() as u32 * config.block_size,
						well.num_columns() as u32 * config.block_size,
					);
					
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