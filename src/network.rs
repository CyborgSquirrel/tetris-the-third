use std::net::{TcpListener, TcpStream};
use bincode::{deserialize, serialize};
use crate::{command::CommandWrapper, lenio::LenIO, room::RoomCommand, unit::UnitCommand};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub enum NetworkCommand {
	UnitCommand(UnitCommand),
	RoomCommand(CommandWrapper<RoomCommand>),
}
impl From<UnitCommand> for NetworkCommand {
	fn from(other: UnitCommand) -> Self {
		NetworkCommand::UnitCommand(other)
	}
}
impl From<CommandWrapper<RoomCommand>> for NetworkCommand {
	fn from(other: CommandWrapper<RoomCommand>) -> Self {
		NetworkCommand::RoomCommand(other)
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
	pub fn broadcast<T: Into<NetworkCommand>+Clone>(&mut self, data: &T) {
		let data: NetworkCommand = data.clone().into();
		match self {
			NetworkState::Offline => {},
			NetworkState::Client {stream} => {
				stream.write(&serialize(&data).unwrap()).unwrap();
			}
			NetworkState::Host {streams,..} => {
				let data = &serialize(&data).unwrap();
				for stream in streams {
					stream.write(data).unwrap();
				}
			}
		}
	}
}

pub struct NetworkPump {
	stream_index: usize,
}

impl NetworkPump {
	pub fn new() -> NetworkPump {
		NetworkPump {stream_index: 0}
	}
	pub fn poll(&mut self, state: &mut NetworkState) -> Option<NetworkCommand> {
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