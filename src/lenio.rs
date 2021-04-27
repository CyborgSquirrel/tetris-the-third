#![allow(dead_code)]
use std::io::{Result,Read,Write,Error,ErrorKind};

#[derive(Debug)]
enum Length {
	Zero,
	OneByte(usize),
	TwoBytes(usize),
}

const MAX_LENGTH: usize = 4096;

#[derive(Debug)]
pub struct LenIO<T> {
	inner: T,
	// MAX_LENGTH storage bytes + 1 EOF byte (for deserializing)
	buf: [u8; MAX_LENGTH+1],
	len: Length,
	pos: usize,
}

impl<T> LenIO<T> {
	pub fn new(inner: T) -> Self {
		Self {
			inner,
			buf: [0; MAX_LENGTH+1],
			len: Length::Zero,
			pos: 0,
		}
	}
}

impl<T: Read> LenIO<T> {
	pub fn read(&mut self) -> Result<&[u8]> {
		if let Length::Zero = self.len {
			let result = self.inner.read(&mut self.buf[0..1]);
			match result {
				Ok(_) => self.len = Length::OneByte(self.buf[0] as usize),
				Err(err) => return Err(err),
			}
		}
		if let Length::OneByte(len) = self.len {
			let result = self.inner.read(&mut self.buf[0..1]);
			match result {
				Ok(_) => self.len = Length::TwoBytes((len << 8) | self.buf[0] as usize),
				Err(err) => return Err(err),
			}
		}
		if let Length::TwoBytes(len) = self.len {
			let result = self.inner.read(&mut self.buf[self.pos..len]);
			match result {
				Ok(bytes) => self.pos += bytes,
				Err(err) => return Err(err),
			}
			
			if self.pos == len {
				// This is so that deserializing doesn't return an UnexpectedEOF error
				self.buf[len] = 0;
				let buf = &self.buf[0..=len];
				self.pos = 0;
				self.len = Length::Zero;
				return Ok(buf);
			}else{
				return Err(Error::new(ErrorKind::Other, "Couldn't finish reading data"));
			}
		}
		Err(Error::new(ErrorKind::Other, "Length is incorrect"))
	}
}

impl<T: Write> LenIO<T> {
	pub fn write(&mut self, bytes: &[u8]) -> Result<()> {
		let len = bytes.len();
		assert!(len <= MAX_LENGTH);
		let len_bytes = (len as u16).to_be_bytes();
		self.inner.write(&len_bytes)
			.and_then(|_|self.inner.write(&bytes[..]))
			.map(|_|{})
	}
}