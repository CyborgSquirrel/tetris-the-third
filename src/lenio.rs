#![allow(dead_code)]
use std::io::{Result,Read,Write,Error,ErrorKind};

#[derive(Debug)]
pub struct LenReader<T> {
	inner: T,
	buf: [u8; 256],
	len: usize,
	pos: usize,
}

impl<T: Read> LenReader<T> {
	pub fn new(inner: T) -> Self {
		Self {
			inner,
			buf: [0;256],
			len: 0,
			pos: 0,
		}
	}
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
			self.pos = 0;
			self.len = 0;
			Ok(&self.buf)
		}else{
			Err(Error::new(ErrorKind::Other, "Couldn't finish reading object"))
		}
	}
}

#[derive(Debug)]
pub struct LenWriter<T> {
	inner: T,
}

impl<T: Write> LenWriter<T> {
	pub fn new(inner: T) -> Self {
		Self {
			inner,
		}
	}
	pub fn write(&mut self, bytes: Vec<u8>) -> Result<()> {
		self.inner.write(&[bytes.len() as u8]).and_then(|_|{
			self.inner.write(&bytes[..])
		}).map(|_|{})
	}
}