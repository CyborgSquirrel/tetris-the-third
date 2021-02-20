#![allow(dead_code)]
#![allow(non_camel_case_types)]

use std::ops::{AddAssign,Add,SubAssign,Sub,MulAssign,Mul,DivAssign,Div,Neg};
use serde::{Serialize,Deserialize};

pub type vec2i = vec2<i32>;
pub type vec2u = vec2<usize>;
pub type vec2f = vec2<f64>;

#[macro_export] macro_rules! vec2i(
    ($x:expr, $y:expr) => (
    	vec2i{x:$x as i32, y:$y as i32}
    )
);

#[macro_export] macro_rules! vec2f(
    ($x:expr, $y:expr) => (
    	vec2f{x:$x as f64, y:$y as f64}
    )
);

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct vec2<T>{
	pub x: T,
	pub y: T,
}

impl<T> vec2<T> {
	pub fn new(x: T, y: T) -> vec2<T> {
		vec2{x, y}
	}
}
impl vec2f {
	pub fn round(&self) -> vec2i {
		vec2i::new(self.x.round() as i32, self.y.round() as i32)
	}
}

impl vec2i {
	pub const UP: vec2i = vec2i!(0,-1);
	pub const RIGHT: vec2i = vec2i!(1,0);
	pub const DOWN: vec2i = vec2i!(0,1);
	pub const LEFT: vec2i = vec2i!(-1,0);
	pub const ZERO: vec2i = vec2i!(0,0);
}

impl From<(usize,usize)> for vec2i {
	fn from(other: (usize,usize)) -> Self {
		vec2{x: other.0 as i32, y: other.1 as i32}
	}
}
impl<T> From<(T,T)> for vec2<T> {
	fn from(other: (T,T)) -> Self {
		vec2{x: other.0, y: other.1}
	}
}
impl From<vec2i> for vec2u {
	fn from(other: vec2i) -> Self {
		vec2{x: other.x as usize, y: other.y as usize}
	}
}
impl From<vec2u> for vec2i {
	fn from(other: vec2u) -> Self {
		vec2{x: other.x as i32, y: other.y as i32}
	}
}
impl From<vec2i> for vec2f {
	fn from(other: vec2i) -> Self {
		vec2{x: other.x as f64, y: other.y as f64}
	}
}
impl From<vec2f> for vec2i {
	fn from(other: vec2f) -> Self {
		vec2{x: other.x as i32, y: other.y as i32}
	}
}

impl<T: Neg<Output = T>+Copy> vec2<T> {
	pub fn rot90r(&self) -> vec2<T> {
		vec2{x: -self.y, y: self.x}
	}
	pub fn rot90l(&self) -> vec2<T> {
		vec2{x: self.y, y: -self.x}
	}
}

impl<T: AddAssign> AddAssign for vec2<T> {
	fn add_assign(&mut self, rhs: Self) {
		self.x += rhs.x; self.y += rhs.y;
	}
}
impl<T: SubAssign> SubAssign for vec2<T> {
	fn sub_assign(&mut self, rhs: Self) {
		self.x -= rhs.x; self.y -= rhs.y;
	}
}

impl<T: Add<Output = T>> Add for vec2<T> {
	type Output = Self;
	fn add(self, rhs: Self) -> Self {
		Self{x: self.x+rhs.x, y: self.y+rhs.y}
	}
}
impl<T: Sub<Output = T>> Sub for vec2<T> {
	type Output = Self;
	fn sub(self, rhs: Self) -> Self {
		Self{x: self.x-rhs.x, y: self.y-rhs.y}
	}
}

impl<T: MulAssign + Copy> MulAssign<T> for vec2<T> {
	fn mul_assign(&mut self, rhs: T) {
		self.x *= rhs; self.y *= rhs;
	}
}
impl<T: DivAssign + Copy> DivAssign<T> for vec2<T> {
	fn div_assign(&mut self, rhs: T) {
		self.x /= rhs; self.y /= rhs;
	}
}

impl<T: Mul<Output = T> + Copy> Mul<T> for vec2<T> {
	type Output = Self;
	fn mul(self, rhs: T) -> Self {
		Self{x: self.x*rhs, y: self.y*rhs}
	}
}
impl<T: Div<Output = T> + Copy> Div<T> for vec2<T> {
	type Output = Self;
	fn div(self, rhs: T) -> Self {
		Self{x: self.x/rhs, y: self.y/rhs}
	}
}

impl<T: Neg<Output = T>> Neg for vec2<T> {
	type Output = Self;
	fn neg(self) -> Self {
	    Self{x: -self.x, y: -self.y}
	}
}