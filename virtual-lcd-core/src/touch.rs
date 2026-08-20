#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TouchState {
    Released,
    Pressed { x: u16, y: u16 },
}

impl Default for TouchState {
    fn default() -> Self {
        TouchState::Released
    }
}
