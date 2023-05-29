pub const U8_COUNT: usize = 0x100;

#[macro_export]
macro_rules! err {
    ($($arg:tt)*) => { Err(format!($($arg)*)) }
}
