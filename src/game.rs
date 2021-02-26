use crate::block;
use crate::Mino;
use std::{mem::swap, time::Duration};
use crate::player;
use crate::vec2i;
use itertools::izip;
use rand::{Rng,SeedableRng,rngs::SmallRng};
use std::cmp::min;
use serde::{Serialize,Deserialize};

pub type Well = array2d::Array2D<Option<block::Data>>;

#[derive(Serialize, Deserialize)]
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

pub fn mino_falling_system(
	falling_mino: &mut Mino,
	well: &Well,
	fall_countdown: &mut Duration,
	fall_duration: Duration,
	softdrop_duration: Duration,
	fall_state: &mut player::FallState)
-> (bool, bool) {
	let fall_duration = match fall_state {
		player::FallState::Fall => fall_duration,
		player::FallState::Softdrop => softdrop_duration,
		player::FallState::Harddrop => Duration::from_secs(0),
	};
	
	if player::FallState::Softdrop == *fall_state {
		*fall_countdown = std::cmp::min(*fall_countdown, softdrop_duration);
	}
	
	if player::FallState::Harddrop == *fall_state {
		*fall_state = player::FallState::Fall;
		*fall_countdown = Duration::from_secs(0);
	}
	
	let mut mino_translated = false;
	
	while *fall_countdown >= fall_duration {
		if try_down_mino(falling_mino, well) {
			mino_translated = true;
			*fall_countdown -= fall_duration;
		}else{
			return (true, mino_translated);
		}
	}
	
	(false, mino_translated)
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

pub fn try_add_bottom_line_with_gap(
	well: &mut Well,
	lines: usize,
	gap: usize,
) {
	if lines == 0 {return}
	for y in 0..well.row_len() {
		for x in 0..well.column_len() {
			if well[(x,y)].is_some() {
				if y >= lines {
					well[(x,y-lines)] = well[(x,y)];
				}
			}
			well[(x,y)] = if y >= well.row_len()-lines && x != gap
			{Some(block::Data::GRAY)} else {None}
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

pub fn mino_rotation_system(
	falling_mino: &mut Mino,
	well: &Well,
	rot_direction: &mut player::RotDirection,
) -> bool
{
	use player::RotDirection;
	let mino_mutated = match rot_direction {
		RotDirection::Left => try_rotl_mino(falling_mino, well),
		RotDirection::Right => try_rotr_mino(falling_mino, well),
		_ => false,
	};
	*rot_direction = RotDirection::None;
	mino_mutated
}

pub fn mino_movement_system(
	falling_mino: &mut Mino,
	well: &Well,
	move_state: &mut player::MoveState,
	move_direction: &mut player::MoveDirection,
	move_repeat_countdown: &mut Duration,
	move_prepeat_duration: Duration,
	move_repeat_duration: Duration,
	dpf: Duration,
) -> bool {
	use player::MoveState;
	use player::MoveDirection;
	let mut mino_mutated = false;
	if MoveState::Instant == *move_state {
		mino_mutated |= match move_direction{
			MoveDirection::Left => try_left_mino(falling_mino, &well),
			MoveDirection::Right => try_right_mino(falling_mino, &well),
			_ => panic!(),
		};
		*move_repeat_countdown = Duration::from_secs(0);
		*move_state = MoveState::Prepeat;
	}
	if MoveState::Prepeat == *move_state {
		if *move_repeat_countdown >= move_prepeat_duration {
			*move_repeat_countdown -= move_prepeat_duration;
			mino_mutated |= match move_direction{
				MoveDirection::Left => try_left_mino(falling_mino, &well),
				MoveDirection::Right => try_right_mino(falling_mino, &well),
				_ => panic!(),
			};
			*move_state = MoveState::Repeat;
		}
	}
	if MoveState::Repeat == *move_state {
		while *move_repeat_countdown >= move_repeat_duration {
			*move_repeat_countdown -= move_repeat_duration;
			mino_mutated |= match move_direction{
				MoveDirection::Left => try_left_mino(falling_mino, &well),
				MoveDirection::Right => try_right_mino(falling_mino, &well),
				_ => panic!(),
			};
		}
	}
	if MoveState::Still != *move_state {
		*move_repeat_countdown += dpf;
	}
	mino_mutated
}

pub fn mino_adding_system<F>(
	falling_mino: &mut Mino,
	well: &mut Well,
	fall_countdown: Option<&mut Duration>,
	animate_line: &mut Vec<bool>,
	can_store_mino: &mut bool,
	mut next_mino: F,
) -> (bool, u32, u32)
where F: FnMut() -> Mino
{
	let can_add = mino_fits_in_well(falling_mino, &well);
	let mut clearable_lines = 0;
	let mut sendable_lines = 0;
	if can_add {
		*can_store_mino = true;
		add_mino_to_well(falling_mino, well);
		
		if let Some(fall_countdown) = fall_countdown {
			*fall_countdown = Duration::from_secs(0);
		}
		
		for (row,clearable) in izip!(well.columns_iter(),animate_line.iter_mut()) {
			let mut count = 0;
			let mut sendable = true;
			for block in row {
				count += block.is_some() as u32;
				sendable &= block.map_or(true, |block|block!=block::Data::GRAY);
			}
			if count as usize == well.column_len() {
				clearable_lines += 1;
				sendable_lines += sendable as u32;
				*clearable = true;
			}
		}
		
		*falling_mino = next_mino();
		center_mino(falling_mino, &well);
	}
	(can_add, clearable_lines, sendable_lines)
}

pub fn mino_storage_system<F>(
	falling_mino: &mut Mino,
	stored_mino: &mut Option<Mino>,
	well: &Well,
	fall_countdown: Option<&mut Duration>,
	store: &mut bool,
	can_store_mino: &mut bool,
	next_mino: F,
	_unit_id: usize,
) -> bool
where F: FnOnce() -> Mino
{
	let r = if *store && *can_store_mino {
		*can_store_mino = false;
		if let Some(fall_countdown) = fall_countdown {
			*fall_countdown = Duration::from_secs(0);
		}
		reset_mino(falling_mino);
		if let Some(stored_mino) = stored_mino {
			swap(stored_mino, falling_mino);
		}else{
			let mut next_mino = next_mino();
			swap(&mut next_mino, falling_mino);
			*stored_mino = Some(next_mino);
		}
		center_mino(falling_mino, &well);
		true
	}else {false};
	*store = false;
	r
}

pub fn line_clearing_system(
	well: &mut Well,
	animate_line: &mut Vec<bool>,
) {
	for line in animate_line {
		*line = false;
	}
	try_clear_lines(well);
}