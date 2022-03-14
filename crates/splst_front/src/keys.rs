use winit::event::VirtualKeyCode;

/// TODO: Add for all possible.
pub fn keycode_name(key: VirtualKeyCode) -> &'static str {
    match key {
        VirtualKeyCode::Key0 => "0",
        VirtualKeyCode::Key1 => "1",
        VirtualKeyCode::Key2 => "2",
        VirtualKeyCode::Key3 => "3",
        VirtualKeyCode::Key4 => "4",
        VirtualKeyCode::Key5 => "5",
        VirtualKeyCode::Key6 => "6",
        VirtualKeyCode::Key7 => "7",
        VirtualKeyCode::Key8 => "8",
        VirtualKeyCode::Key9 => "9",

        VirtualKeyCode::A => "a",
        VirtualKeyCode::B => "b",
        VirtualKeyCode::C => "c",
        VirtualKeyCode::D => "d",
        VirtualKeyCode::E => "e",
        VirtualKeyCode::F => "f",
        VirtualKeyCode::G => "g",
        VirtualKeyCode::H => "h",
        VirtualKeyCode::I => "i",
        VirtualKeyCode::J => "j",
        VirtualKeyCode::K => "k",
        VirtualKeyCode::L => "l",
        VirtualKeyCode::M => "m",
        VirtualKeyCode::N => "n",
        VirtualKeyCode::O => "o",
        VirtualKeyCode::P => "p",
        VirtualKeyCode::Q => "q",
        VirtualKeyCode::R => "r",
        VirtualKeyCode::S => "s",
        VirtualKeyCode::T => "t",
        VirtualKeyCode::U => "u",
        VirtualKeyCode::V => "v",
        VirtualKeyCode::W => "w",
        VirtualKeyCode::X => "x",
        VirtualKeyCode::Y => "y",
        VirtualKeyCode::Z => "z",

        VirtualKeyCode::Down => "Down",
        VirtualKeyCode::Left => "Left",
        VirtualKeyCode::Right => "Right",
        VirtualKeyCode::Up => "Up",

        VirtualKeyCode::Escape => "Escape",
        VirtualKeyCode::Tab => "Tab",
        VirtualKeyCode::Back => "Back",
        VirtualKeyCode::Return => "Return",
        VirtualKeyCode::Space => "Space",
        VirtualKeyCode::Insert => "Insert",
        VirtualKeyCode::Delete => "Delete",
        VirtualKeyCode::Home => "Home",
        VirtualKeyCode::End => "End",
        VirtualKeyCode::PageUp => "PageUp",
        VirtualKeyCode::PageDown => "PageDown",
        _ => unreachable!("Unsupported key code")
    }
}