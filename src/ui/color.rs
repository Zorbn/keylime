use serde::{de::Error, Deserialize, Deserializer};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn from_hex(value: u32) -> Self {
        Self {
            r: (value >> 24) as u8,
            g: (value >> 16) as u8,
            b: (value >> 8) as u8,
            a: value as u8,
        }
    }

    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;

        let has_default_alpha = match s.len() {
            6 => true,
            8 => false,
            _ => {
                return Err(D::Error::custom(
                    "invalid hex color length, expected 6 or 8 digits",
                ))
            }
        };

        let value =
            u32::from_str_radix(s, 16).map_err(|_| D::Error::custom("invalid hex color"))?;

        let value = if has_default_alpha {
            (value << 8) | 0xFF
        } else {
            value
        };

        Ok(Self::from_hex(value))
    }
}
