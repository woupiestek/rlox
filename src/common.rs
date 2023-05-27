pub const U8_COUNT: usize = 0x100;

pub fn error<Any>(msg: &str) -> Result<Any, String> {
    Err(msg.to_string())
}
