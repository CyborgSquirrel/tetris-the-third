use crate::block;
use crate::Mino;
use crate::vec2i;
use rand::{Rng,SeedableRng,rngs::SmallRng};
use std::cmp::min;
use serde::{Serialize,Deserialize};

pub type Well = array2d::Array2D<Option<block::Data>>;

#[derive(Clone, Serialize, Deserialize)]
pub enum MinoRng {
	_Hard {#[serde(skip,default="SmallRng::from_entropy")] rng: SmallRng},
	Fair {#[serde(skip,default="SmallRng::from_entropy")] rng: SmallRng, stack: Vec<Mino>},
}

impl MinoRng {
	pub fn generate(&mut self) -> Mino {
		const MINO_CTORS: [fn() -> Mino; 7] =
			[Mino::l,Mino::j,Mino::o,Mino::z,Mino::s,Mino::t,Mino::i];
		let mino = match self {
			MinoRng::_Hard {ref mut rng} => {
				MINO_CTORS[rng.gen_range(0..7)]()
			}
			MinoRng::Fair {ref mut rng, ref mut stack} => {
				if stack.is_empty() {
					for i in 0..7 {
						stack.push(MINO_CTORS[i]());
					}
					for i in 0..6 {
						let j = i + rng.gen_range(0..7-i);
						stack.swap(i, j);
					}
				}
				stack.pop().unwrap()
			}
		};
		mino
	}
	pub fn generate_centered(&mut self, well: &Well) -> Mino {
		let mut mino = self.generate();
		center_mino(&mut mino, well);
		mino
	}
	pub fn fair() -> MinoRng {
		MinoRng::Fair {rng: SmallRng::from_entropy(), stack: Vec::with_capacity(7)}
	}
}

pub fn check_block_in_bounds(block: &vec2i, dim: &vec2i) -> bool {
	block.x >= 0 && block.x < dim.x && block.y < dim.y
}

pub fn check_mino_well_collision(mino: &Mino, well: &Well) -> bool {
	let dim = vec2i::from((well.column_len(),well.row_len()));
	for block in mino.blocks.iter() {
		if block.y < 0 {continue;}
		if !check_block_in_bounds(block, &dim) {
			return true;
		}
		if well[(block.x as usize, block.y as usize)].is_some() {
			return true;
		}
	}
	false
}

pub fn may_mutate_mino<F>(mino: &Mino, well: &Well, f: F) -> bool
where F: Fn(&mut Mino) {
	let mut mutated_mino = mino.clone();
	f(&mut mutated_mino);
	!check_mino_well_collision(&mutated_mino, &well)
}
pub fn may_down_mino(mino: &Mino, well: &Well) -> bool {
	may_mutate_mino(mino, well, |mino|mino.down())
}

pub fn try_mutate_mino<F>(mino: &mut Mino, well: &Well, f: F) -> bool
where F: Fn(&mut Mino, &Well) {
	let mut mutated_mino = mino.clone();
	f(&mut mutated_mino, well);
	if !check_mino_well_collision(&mutated_mino, &well) {
		*mino = mutated_mino;
		return true;
	}
	false
}
pub fn try_left_mino(mino: &mut Mino, well: &Well) -> bool{
	try_mutate_mino(mino, well, |mino,_|mino.left())
}
pub fn try_right_mino(mino: &mut Mino, well: &Well) -> bool{
	try_mutate_mino(mino, well, |mino,_|mino.right())
}
pub fn try_down_mino(mino: &mut Mino, well: &Well) -> bool{
	try_mutate_mino(mino, well, |mino,_|mino.down())
}

pub fn create_shadow_mino(mino: &Mino, well: &Well) -> Mino {
	let mut shadow_mino = mino.clone();
	shadow_mino.make_shadow();
	while try_down_mino(&mut shadow_mino, &well) {}
	shadow_mino
}

pub fn try_clear_lines(well: &mut Well) {
	let mut dy: usize = 0;
	for y in (0..well.row_len()).rev() {
		let mut count = 0;
		for x in 0..well.column_len() {
			count += well[(x,y)].is_some() as usize;
			if dy != 0 {
				well[(x,y+dy)] = well[(x,y)];
				well[(x,y)] = None;
			}
		}
		if count == well.column_len() {
			dy += 1;
		}
	}
}

pub fn mino_fits_in_well(mino: &Mino, well: &Well) -> bool {
	for block in mino.blocks.iter() {
		if block.y < 0 || well[(block.x as usize, block.y as usize)].is_some() {
			return false;
		}
	}
	true
}

pub fn add_mino_to_well(mino: &Mino, well: &mut Well) {
	for (block, data) in mino.blocks.iter().zip(mino.blocks_data.iter()) {
		assert!(block.y >= 0 && well[(block.x as usize, block.y as usize)].is_none());
		well[(block.x as usize, block.y as usize)] = Some(*data);
	}
}

pub fn center_mino(mino: &mut Mino, well: &Well) {
	let ext = mino.get_size();
	mino.translate(vec2i::RIGHT * (well.num_rows() as i32-ext.x)/2);
}

pub fn reset_mino(mino: &mut Mino) {
	for _ in 0..mino.rotation.rem_euclid(4) {
		mino.rotl();
	}
	let (lo,_) = mino.get_rect();
	mino.translate(-lo);
}

fn move_mino_into_horizontal_bounds(mino: &mut Mino, well: &Well) {
	let (lo,hi) = mino.get_rect();
	mino.translate(vec2i!(-min(0,lo.x),0));
	mino.translate(vec2i!(min(0,well.num_rows() as i32 - hi.x - 1),0));
}
pub fn try_rotl_mino(mino: &mut Mino, well: &Well) -> bool{
	try_mutate_mino(mino, well,
		|mino,well|{
			mino.rotl();
			move_mino_into_horizontal_bounds(mino, well);})
}
pub fn try_rotr_mino(mino: &mut Mino, well: &Well) -> bool{
	try_mutate_mino(mino, well,
		|mino,well|{
			mino.rotr();
			move_mino_into_horizontal_bounds(mino, well);})
}