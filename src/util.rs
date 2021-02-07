use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use crate::config::Controlcode;

pub fn is_key_down(event: &Event, key: Option<Keycode>) -> bool {
	if let Some(key) = key {
		if let Event::KeyDown{keycode: Some(event_key),repeat: false,..} = event {
			key == *event_key
		}else {
			false
		}
	}else {false}
}

pub fn is_key_up(event: &Event, key: Option<Keycode>) -> bool {
	if let Some(key) = key {
		if let Event::KeyUp{keycode: Some(event_key),repeat: false,..} = event {
			key == *event_key
		}else {
			false
		}
	}else {false}
}

pub fn is_controlcode_down(
	event: &Event,
	controlcode: &mut Option<Controlcode>,
	joystick_id: Option<u32>)
-> bool {
	if let Some(joystick_id) = joystick_id {
		if let Some(controlcode) = controlcode {
			match (controlcode, event) {
				(Controlcode::Button(button),
				Event::ControllerButtonDown{button: event_button,which,..})
				if joystick_id == *which => {
					button == event_button
				}
				
				(Controlcode::Axis(axis, ref mut down),
				Event::ControllerAxisMotion{axis:event_axis,value,which,..})
				if joystick_id == *which && axis == event_axis => {
					if !*down && *value >= 4096i16 {
						*down = true;
						true
					}else if *down && *value < 4096 {
						*down = false;
						false
					}else {false}
				}
				
				(_,_) => false
			}
		}else {false}
	}else {false}
}

pub fn is_controlcode_up(
	event: &Event,
	controlcode: &mut Option<Controlcode>,
	joystick_id: Option<u32>)
-> bool {
	if let Some(joystick_id) = joystick_id {
		if let Some(controlcode) = controlcode {
			match (controlcode, event) {
				(Controlcode::Button(button),
				Event::ControllerButtonUp{button: event_button,which,..})
				if joystick_id == *which => {
					button == event_button
				}
				
				(Controlcode::Axis(axis, ref mut down),
				Event::ControllerAxisMotion{axis:event_axis,value,which,..})
				if joystick_id == *which && axis == event_axis => {
					if !*down && *value >= 4096i16 {
						*down = true;
						false
					}else if *down && *value < 4096 {
						*down = false;
						true
					}else {false}
				}
				
				(_,_) => false
			}
		}else {false}
	}else {false}
}