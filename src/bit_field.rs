macro_rules! define_bit_field {
    ($bit_field_name:ident, $name:ident) => {
        #[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
        pub struct $bit_field_name {
            bits: u8,
            _phantom: std::marker::PhantomData<$name>,
        }

        #[allow(dead_code)]
        impl $bit_field_name {
            pub const NONE: Self = Self::new(0);

            pub const ALL: Self = Self::new(0xFF);

            const fn new(bits: u8) -> Self {
                Self {
                    bits,
                    _phantom: std::marker::PhantomData,
                }
            }

            pub const fn from(value: $name) -> Self {
                Self::NONE.with(value)
            }

            pub const fn with(self, value: $name) -> Self {
                let value = 1 << value as u8;

                Self::new(self.bits | value)
            }

            pub const fn without(self, value: $name) -> Self {
                let value = 1 << value as u8;

                Self::new(self.bits & !value)
            }

            pub const fn contains(self, value: $name) -> bool {
                let value = 1 << value as u8;

                self.bits & value == value
            }
        }

        impl std::ops::BitOr for $bit_field_name {
            type Output = $bit_field_name;

            fn bitor(self, rhs: Self) -> Self {
                Self::new(self.bits | rhs.bits)
            }
        }

        impl std::ops::Sub for $bit_field_name {
            type Output = $bit_field_name;

            fn sub(self, rhs: Self) -> Self {
                Self::new(self.bits & !rhs.bits)
            }
        }
    };
}

pub(crate) use define_bit_field;
