
										
										network_state.broadcast_event(
											&NetworkEvent::UnitEvent{unit_id,event:UnitEvent::AddMinoToWell}
										);
										
										let can_add = mino_fits_in_well(&falling_mino, &well);
										if !can_add {
											*state = UnitState::Over;
										}else{
											*can_store_mino = true;
											add_mino_to_well(&falling_mino, well);
											*fall_countdown = Duration::from_secs(0);
											
											let mut clearable_lines = 0;
											mark_clearable_lines(&well, animate_line, &mut clearable_lines);
											
											if clearable_lines != 0 {
												*state = UnitState::LineClear{countdown: Duration::from_secs(0)};
												
												*lines_cleared += clearable_lines;
												*lines_cleared_text =
													create_lines_cleared_text(*lines_cleared, &font, &texture_creator);
												
												if let Mode::Marathon{level,lines_before_next_level,..} = mode {
													*lines_before_next_level -= clearable_lines as i32;
													let level_changed = *lines_before_next_level <= 0;
													while *lines_before_next_level <= 0 {
														*level += 1;
														*lines_before_next_level +=
															get_lines_before_next_level(*level) as i32;
													}
												
													if level_changed {
														*level_text =
															create_level_text(*level, &font, &texture_creator);
														*fall_duration = get_fall_duration(*level);
													}
												}
											}
											
											*falling_mino = queue.pop_front().unwrap();
											network_state.broadcast_event(
												&NetworkEvent::UnitEvent{unit_id,event:UnitEvent::GenerateMino{
													mino: falling_mino.clone(),
												}}
											);
											center_mino(falling_mino, &well);
											
											queue.push_back(rng.generate());
										}
									