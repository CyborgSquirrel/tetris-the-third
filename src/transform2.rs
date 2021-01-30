use std::ops::{Mul,Index,IndexMut};

#[derive(Default)]
pub struct Transform2<T> {
	matrix: [T;9],
}

impl<T> Index<(usize,usize)> for Transform2<T> {
	type Output = T;
	fn index(&self, index: (usize,usize)) -> &Self::Output {
	    &self.matrix[index.0+index.1*3]
	}
}

impl<T> IndexMut<(usize,usize)> for Transform2<T> {
	fn index_mut(&mut self, index: (usize,usize)) -> &mut Self::Output {
	    &mut self.matrix[index.0+index.1*3]
	}
}

// impl<T: Mul<T>> Mul<Transform2<T>> for Transform2<T> {
// 	type Output = Self;
// 	fn mul(self, rhs: Self) -> Self {
// 		let mut r: Self = Default::default();
// 		for x in 0..3 {
// 			for y in 0..3 {
// 				for i in 0..3 {
// 					r[(x,y)] += self[(i,y)]*rhs[(x,i)];
// 				}
// 			}
// 		}
// 		r
// 	}
// }