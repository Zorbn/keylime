macro_rules! define_bit_field {
    ($bit_field_name:ident, $name:ident, $inner_type:ty) => {
        #[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
        pub struct $bit_field_name {
            bits: $inner_type,
            _phantom: std::marker::PhantomData<$name>,
        }

        #[allow(dead_code)]
        impl $bit_field_name {
            pub const NONE: Self = Self::new(0);

            pub const ALL: Self = Self::new(0xFF);

            const fn new(bits: $inner_type) -> Self {
                Self {
                    bits,
                    _phantom: std::marker::PhantomData,
                }
            }

            pub const fn from(value: $name) -> Self {
                Self::NONE.with(value)
            }

            pub const fn with(self, value: $name) -> Self {
                let value = 1 << value as $inner_type;

                Self::new(self.bits | value)
            }

            pub const fn without(self, value: $name) -> Self {
                let value = 1 << value as $inner_type;

                Self::new(self.bits & !value)
            }

            pub const fn contains(self, value: $name) -> bool {
                let value = 1 << value as $inner_type;

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

        impl<T: AsRef<[$name]>> From<T> for $bit_field_name {
            fn from(values: T) -> Self {
                let mut bit_field = Self::NONE;

                for value in values.as_ref() {
                    bit_field = bit_field.with(*value);
                }

                bit_field
            }
        }
    };
}

pub(crate) use define_bit_field;
