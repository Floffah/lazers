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
    #[allow(dead_code)]
    pub fn remove(&mut self, key: &str) -> bool {
        let Some(index) = self.find_index(key) else {
            return false;
        };

        let mut current = index;
        while current + 1 < self.len {
            self.entries[current] = self.entries[current + 1];
            current += 1;
        }
        self.len -= 1;
        self.entries[self.len] = EnvEntry::empty();
        true
    }

    /// Returns the value for a key if it exists.
    #[allow(dead_code)]
    pub fn get(&self, key: &str) -> Option<&str> {
        let index = self.find_index(key)?;
        Some(self.entries[index].value())
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
        assert_eq!(env.get("SHELL"), Some("/bin/lash"));
    }

    #[test]
    fn update_existing_variable() {
        let mut env = Environment::new();
        env.set("TERM", "vt").unwrap();
        env.set("TERM", "text").unwrap();
        assert_eq!(env.get("TERM"), Some("text"));
    }

    #[test]
    fn remove_variable() {
        let mut env = Environment::new();
        env.set("USER", "root").unwrap();
        assert!(env.remove("USER"));
        assert_eq!(env.get("USER"), None);
    }

    #[test]
    fn empty_value_is_allowed() {
        let mut env = Environment::new();
        env.set("EMPTY", "").unwrap();
        assert_eq!(env.get("EMPTY"), Some(""));
    }

    #[test]
    fn invalid_key_is_rejected() {
        let mut env = Environment::new();
        assert_eq!(env.set("", "x"), Err(EnvironmentError::InvalidKey));
        assert_eq!(env.set("BAD=KEY", "x"), Err(EnvironmentError::InvalidKey));
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
        child.remove("SHELL");

        assert_eq!(parent.get("USER"), Some("root"));
        assert_eq!(parent.get("SHELL"), Some("/bin/lash"));
        assert_eq!(child.get("USER"), Some("guest"));
        assert_eq!(child.get("SHELL"), None);
    }

    fn key_for(index: usize) -> std::string::String {
        std::format!("K{}", index)
    }
}
