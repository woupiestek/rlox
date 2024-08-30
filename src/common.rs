pub const U8_COUNT: usize = 0x100;

#[macro_export]
macro_rules! err {
    ($($arg:tt)*) => { Err(format!($($arg)*)) }
}

pub const STRINGS:u8 = 0;
pub const NATIVES:u8 = 1;
pub const FUNCTIONS:u8 = 2;
pub const OBJECTS:u8 = 3;
// pub const UPVALUES:u8 = 4;
