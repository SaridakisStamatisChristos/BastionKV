use crate::{Error, Result};

/// Persistent parsing and live-state bounds. Only synchronous durability is supported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub max_key_bytes: u32,
    pub max_value_bytes: u32,
    pub max_frame_bytes: u32,
    pub max_state_bytes: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            max_key_bytes: 4096,
            max_value_bytes: 1_048_576,
            max_frame_bytes: 67_108_864,
            max_state_bytes: 33_554_432,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        if self.max_key_bytes == 0
            || self.max_key_bytes > 1_048_576
            || self.max_value_bytes > 16_777_216
            || self.max_frame_bytes < 32
            || self.max_frame_bytes > 268_435_456
            || self.max_state_bytes < 16
            || self.max_state_bytes > u64::from(self.max_frame_bytes) - 12
            || u64::from(self.max_key_bytes) + u64::from(self.max_value_bytes) + 16
                > self.max_state_bytes
        {
            return Err(Error::InvalidConfig(
                "require key<=1MiB, value<=16MiB, frame<=256MiB and key+value+16<=state<=frame-12"
                    .into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn key(&self, k: &[u8]) -> Result<()> {
        if k.is_empty() || k.len() > self.max_key_bytes as usize {
            return Err(Error::InvalidInput(
                "key empty or exceeds configured bound".into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn value(&self, v: &[u8]) -> Result<()> {
        if v.len() > self.max_value_bytes as usize {
            return Err(Error::InvalidInput("value exceeds configured bound".into()));
        }
        Ok(())
    }
}
