use sdl2::keyboard::Keycode;

enum Key {
    A,
    B,
    Select,
    Start,
    Up,
    Down,
    Left,
    Right
}

pub struct Controller {
    key_state: [bool; 8],
    strobe: bool,
    key_index: u8,
}

impl Controller {
    pub fn new() -> Controller {
        Controller {
            key_state: [false; 8],
            strobe: false,
            key_index: 0,
        }
    }

    fn get_key_from_keycode(keycode: Keycode) -> Option<Key> {
        match keycode {
            Keycode::F => Some(Key::A),
            Keycode::D => Some(Key::B),
            Keycode::S => Some(Key::Select),
            Keycode::Return => Some(Key::Start),
            Keycode::Up => Some(Key::Up),
            Keycode::Down => Some(Key::Down),
            Keycode::Left => Some(Key::Left),
            Keycode::Right => Some(Key::Right),
            _ => None,
        }
    }

    pub fn handle_key_change(&mut self, keycode: Keycode, is_pressed: bool) {
        match Controller::get_key_from_keycode(keycode) {
            Some(key) => { self.key_state[key as usize] = is_pressed; },
            None => {},
        }
    }

    pub fn handle_key_down(&mut self, keycode: Keycode) {
        self.handle_key_change(keycode, true);
    }

    pub fn handle_key_up(&mut self, keycode: Keycode) {
        self.handle_key_change(keycode, false);
    }

    pub fn read_mem(&mut self, cpu_address: u16) -> u8 {
        match cpu_address {
            0x4016 => {
                if self.strobe {
                    if self.key_state[self.key_index as usize] { 1 } else { 0 }
                }
                else {
                    let result = self.key_state[self.key_index as usize];
                    self.key_index += 1;
                    if result { 1 } else { 0 }
                }
            },
            0x4017 => { 0 },
            _ => panic!("Unimplemented read address: {:04X}", cpu_address)
        }
    }

    pub fn write_mem(&mut self, cpu_address: u16, value: u8) {
        match cpu_address {
            0x4016 => {
                if value & 0x01 != 0 {
                    self.strobe = true;
                    self.key_index = 0;
                }
                else {
                    self.strobe = false;
                }
            }
            _ => panic!("Unimplemented write address: {:04X}", cpu_address)
        }
    }
}
