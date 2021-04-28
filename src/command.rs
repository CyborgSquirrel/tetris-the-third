use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommandWrapper<T> {
	pub inner: T,
	
	#[serde(skip_serializing)]
	pub original: bool,
}
impl<'a, T> CommandWrapper<T> {
	pub fn new(inner: T) -> Self {
		Self {inner, original: true}
	}
}

impl<'a, T> CommandWrapper<T> where
T: Command<'a>,
CommandWrapper<T>: Into<crate::network::NetworkCommand>+Clone {
	pub fn execute<F>(self, network_state: &mut crate::network::NetworkState, mut append: F, params: T::Params)
	where F: FnMut(Self) {
		let original = self.original;
		if original {
			network_state.broadcast(&self);
		}
		self.inner.execute(|inner|append(Self {original, inner}), params);
	}
}

impl<T> CommandWrapper<T> {
	pub fn map<F,U>(self, f: F) -> CommandWrapper<U> where F: FnOnce(T) -> U {
		CommandWrapper {inner: f(self.inner), original: self.original}
	}
}

pub trait Command<'a> {
	type Params;
	fn execute<F>(self, append: F, params: Self::Params)
	where F: FnMut(Self), Self: Sized;
	fn wrap(self) -> CommandWrapper<Self> where Self: Sized {
		CommandWrapper::new(self)
	}
}