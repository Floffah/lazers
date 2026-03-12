//! Fixed-capacity process environment storage.
//!
//! This is the first environment-variable layer for Lazers. It is intentionally
//! small and allocation-free so it matches the rest of the bootstrap runtime.
//! The kernel stores environment as explicit key/value pairs owned by a
//! process, and child processes inherit a copy of that state during spawn.

pub const MAX_ENV_VARS: usize = 16;
pub const MAX_ENV_KEY_LEN: usize = 32;
pub const MAX_ENV_VALUE_LEN: usize = 128;

/// Failures returned by environment mutation helpers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvironmentError {
    InvalidKey,
    KeyTooLong,
    ValueTooLong,
    CapacityExceeded,
}

/// Fixed-capacity environment block owned by a process.
#[derive(Clone, Copy)]
pub struct Environment {
    entries: [EnvEntry; MAX_ENV_VARS],
    len: usize,
}

impl Environment {
    /// Creates an empty environment block.
    pub const fn new() -> Self {
        Self {
            entries: [EnvEntry::empty(); MAX_ENV_VARS],
            len: 0,
        }
    }

    /// Inserts or updates one environment variable.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), EnvironmentError> {
        validate_key(key)?;
        if value.len() > MAX_ENV_VALUE_LEN {
            return Err(EnvironmentError::ValueTooLong);
        }

        if let Some(index) = self.find_index(key) {
            self.entries[index].set_value(value);
            return Ok(());
        }

        if self.len == self.entries.len() {
            return Err(EnvironmentError::CapacityExceeded);
        }

        self.entries[self.len].set_pair(key, value);
        self.len += 1;
        Ok(())
    }

    /// Removes one environment variable if it exists.
    pub fn remove(&mut self, key: &str) -> Result<bool, EnvironmentError> {
        validate_key(key)?;
        let Some(index) = self.find_index(key) else {
            return Ok(false);
        };

        let mut current = index;
        while current + 1 < self.len {
            self.entries[current] = self.entries[current + 1];
            current += 1;
        }
        self.len -= 1;
        self.entries[self.len] = EnvEntry::empty();
        Ok(true)
    }

    /// Returns the value for a key if it exists.
    pub fn get(&self, key: &str) -> Result<Option<&str>, EnvironmentError> {
        validate_key(key)?;
        let index = self.find_index(key);
        Ok(index.map(|entry| self.entries[entry].value()))
    }

    /// Removes all environment variables.
    pub fn clear(&mut self) {
        let mut index = 0;
        while index < self.len {
            self.entries[index] = EnvEntry::empty();
            index += 1;
        }
        self.len = 0;
    }

    /// Copies this environment block into another process-owned block.
    pub fn inherit_into(&self, child: &mut Environment) -> Result<(), EnvironmentError> {
        child.clear();
        let mut index = 0;
        while index < self.len {
            child.set(self.entries[index].key(), self.entries[index].value())?;
            index += 1;
        }
        Ok(())
    }

    fn find_index(&self, key: &str) -> Option<usize> {
        let mut index = 0;
        while index < self.len {
            if self.entries[index].key() == key {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    /// Serializes the current environment as newline-delimited `KEY=VALUE`
    /// entries in insertion order.
    pub fn write_listing_into(&self, buffer: &mut [u8]) -> Result<usize, EnvironmentError> {
        let mut written = 0usize;
        let mut index = 0usize;
        while index < self.len {
            let entry = &self.entries[index];
            let needed = entry.key_len + 1 + entry.value_len + 1;
            if written + needed > buffer.len() {
                return Err(EnvironmentError::CapacityExceeded);
            }

            let key = &entry.key[..entry.key_len];
            let value = &entry.value[..entry.value_len];
            buffer[written..written + key.len()].copy_from_slice(key);
            written += key.len();
            buffer[written] = b'=';
            written += 1;
            buffer[written..written + value.len()].copy_from_slice(value);
            written += value.len();
            buffer[written] = b'\n';
            written += 1;
            index += 1;
        }

        Ok(written)
    }
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
struct EnvEntry {
    key: [u8; MAX_ENV_KEY_LEN],
    key_len: usize,
    value: [u8; MAX_ENV_VALUE_LEN],
    value_len: usize,
}

impl EnvEntry {
    const fn empty() -> Self {
        Self {
            key: [0; MAX_ENV_KEY_LEN],
            key_len: 0,
            value: [0; MAX_ENV_VALUE_LEN],
            value_len: 0,
        }
    }

    fn set_pair(&mut self, key: &str, value: &str) {
        self.key[..key.len()].copy_from_slice(key.as_bytes());
        self.key_len = key.len();
        self.set_value(value);
    }

    fn set_value(&mut self, value: &str) {
        self.value[..value.len()].copy_from_slice(value.as_bytes());
        self.value_len = value.len();
    }

    fn key(&self) -> &str {
        core::str::from_utf8(&self.key[..self.key_len]).unwrap_or("")
    }

    fn value(&self) -> &str {
        core::str::from_utf8(&self.value[..self.value_len]).unwrap_or("")
    }
}

fn validate_key(key: &str) -> Result<(), EnvironmentError> {
    if key.is_empty() || key.as_bytes().contains(&b'=') {
        return Err(EnvironmentError::InvalidKey);
    }
    if key.len() > MAX_ENV_KEY_LEN {
        return Err(EnvironmentError::KeyTooLong);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_lookup_variable() {
        let mut env = Environment::new();
        env.set("SHELL", "/bin/lash").unwrap();
        assert_eq!(env.get("SHELL"), Ok(Some("/bin/lash")));
    }

    #[test]
    fn update_existing_variable() {
        let mut env = Environment::new();
        env.set("TERM", "vt").unwrap();
        env.set("TERM", "text").unwrap();
        assert_eq!(env.get("TERM"), Ok(Some("text")));
    }

    #[test]
    fn remove_variable() {
        let mut env = Environment::new();
        env.set("USER", "root").unwrap();
        assert_eq!(env.remove("USER"), Ok(true));
        assert_eq!(env.get("USER"), Ok(None));
    }

    #[test]
    fn empty_value_is_allowed() {
        let mut env = Environment::new();
        env.set("EMPTY", "").unwrap();
        assert_eq!(env.get("EMPTY"), Ok(Some("")));
    }

    #[test]
    fn invalid_key_is_rejected() {
        let mut env = Environment::new();
        assert_eq!(env.set("", "x"), Err(EnvironmentError::InvalidKey));
        assert_eq!(env.set("BAD=KEY", "x"), Err(EnvironmentError::InvalidKey));
        assert_eq!(env.get("BAD=KEY"), Err(EnvironmentError::InvalidKey));
        assert_eq!(env.remove("BAD=KEY"), Err(EnvironmentError::InvalidKey));
    }

    #[test]
    fn capacity_exhaustion_is_reported() {
        let mut env = Environment::new();
        for index in 0..MAX_ENV_VARS {
            let key = key_for(index);
            env.set(key.as_str(), "x").unwrap();
        }

        assert_eq!(
            env.set("OVERFLOW", "x"),
            Err(EnvironmentError::CapacityExceeded)
        );
    }

    #[test]
    fn inheritance_copies_independently() {
        let mut parent = Environment::new();
        let mut child = Environment::new();
        parent.set("USER", "root").unwrap();
        parent.set("SHELL", "/bin/lash").unwrap();

        parent.inherit_into(&mut child).unwrap();
        child.set("USER", "guest").unwrap();
        child.remove("SHELL").unwrap();

        assert_eq!(parent.get("USER"), Ok(Some("root")));
        assert_eq!(parent.get("SHELL"), Ok(Some("/bin/lash")));
        assert_eq!(child.get("USER"), Ok(Some("guest")));
        assert_eq!(child.get("SHELL"), Ok(None));
    }

    fn key_for(index: usize) -> std::string::String {
        std::format!("K{}", index)
    }
}
