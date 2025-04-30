use crate::bit_field::define_bit_field;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

define_bit_field!(Sides, Side);
