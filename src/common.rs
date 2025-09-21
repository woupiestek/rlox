pub const STACK_SIZE: usize = 0x4000;

#[macro_export]
macro_rules! err {
    ($($arg:tt)*) => { Err(format!($($arg)*)) }
}
