if *store && *can_store_mino {			
	*can_store_mino = false;
	*store = false;
	*fall_countdown = Duration::from_secs(0);
	reset_mino(falling_mino);
	if let Some(stored_mino) = stored_mino {
		swap(stored_mino, falling_mino);
		network_state.broadcast_event(
			&NetworkEvent::UnitEvent{unit_id,event:UnitEvent::StoreMino{generated_mino:None}}
		);
	}else{
		let mut next_mino = queue.pop_front().unwrap();
		swap(&mut next_mino, falling_mino);
		*stored_mino = Some(next_mino);
		queue.push_back(rng.generate());
		network_state.broadcast_event(
			&NetworkEvent::UnitEvent{unit_id,event:UnitEvent::StoreMino{generated_mino:Some(falling_mino.clone())}}
		);
	}
	center_mino(falling_mino, &well);
}