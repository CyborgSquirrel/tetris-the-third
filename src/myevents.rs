#![allow(dead_code)]
use sdl2::controller::{Axis, Button};

#[derive(Debug)]
pub struct MyControllerButtonDown {
	pub timestamp: u32,
	pub which: usize,
	pub button: Button,
}

#[derive(Debug)]
pub struct MyControllerButtonUp {
	pub timestamp: u32,
	pub which: usize,
	pub button: Button,
}

#[derive(Debug)]
pub struct MyControllerAxisDown {
	pub timestamp: u32,
	pub which: usize,
	pub axis: Axis,
}

#[derive(Debug)]
pub struct MyControllerAxisUp {
	pub timestamp: u32,
	pub which: usize,
	pub axis: Axis,
}