use crate::bit_field::define_bit_field;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

define_bit_field!(Sides, Side, u8);
