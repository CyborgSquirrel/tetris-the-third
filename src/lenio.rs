#![allow(dead_code)]
use std::io::{Result,Read,Write,Error,ErrorKind};

#[derive(Debug)]
pub struct LenIO<T> {
	inner: T,
	// 256 storage bytes + 1 EOF byte (for deserializing)
	buf: [u8; 257],
	len: usize,
	pos: usize,
}

impl<T> LenIO<T> {
	pub fn new(inner: T) -> Self {
		Self {
			inner,
			buf: [0;257],
			len: 0,
			pos: 0,
		}
	}
}

impl<T: Read> LenIO<T> {
	pub fn read(&mut self) -> Result<&[u8]> {
		if self.len == 0 {
			let result = self.inner.read(&mut self.buf[0..1]);
			match result {
				Ok(_) => self.len = self.buf[0] as usize,
				Err(err) => return Err(err),
			}
		}
		if self.len != 0 {
			let result = self.inner.read(&mut self.buf[self.pos..self.len]);
			match result {
				Ok(bytes) => self.pos += bytes,
				Err(err) => return Err(err),
			}
		}
		
		if self.pos == self.len {
			// This is so that deserializing doesn't return an UnexpectedEOF error
			self.buf[self.len] = 0;
			let buf = &self.buf[0..=self.len];
			self.pos = 0;
			self.len = 0;
			Ok(buf)
		}else{
			Err(Error::new(ErrorKind::Other, "Couldn't finish reading object"))
		}
	}
}

impl<T: Write> LenIO<T> {
	pub fn write(&mut self, bytes: &[u8]) -> Result<()> {
		assert!(bytes.len() <= u8::MAX as usize);
		self.inner.write(&[bytes.len() as u8]).and_then(|_|{
			self.inner.write(&bytes[..])
		}).map(|_|{})
	}
}