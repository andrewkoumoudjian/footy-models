use std::collections::HashMap;

use crate::{CoreError, StringId};

/// Simple deterministic string table for moving text out of hot domain structures.
#[derive(Debug, Default, Clone)]
pub struct StringInterner {
    ids: HashMap<String, StringId>,
    values: Vec<String>,
}

impl StringInterner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            ids: HashMap::with_capacity(capacity),
            values: Vec::with_capacity(capacity),
        }
    }

    pub fn intern(&mut self, value: &str) -> Result<StringId, CoreError> {
        if let Some(id) = self.ids.get(value) {
            return Ok(*id);
        }

        let raw = u32::try_from(self.values.len()).map_err(|_| CoreError::StringTableFull)?;
        let id = StringId(raw);
        let owned = value.to_owned();
        self.values.push(owned.clone());
        self.ids.insert(owned, id);
        Ok(id)
    }

    #[inline]
    pub fn resolve(&self, id: StringId) -> Option<&str> {
        self.values.get(id.0 as usize).map(String::as_str)
    }

    #[inline]
    pub fn get(&self, value: &str) -> Option<StringId> {
        self.ids.get(value).copied()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_strings_share_an_id() {
        let mut interner = StringInterner::new();
        let first = interner.intern("Benfica").unwrap();
        let second = interner.intern("Benfica").unwrap();
        assert_eq!(first, second);
        assert_eq!(interner.resolve(first), Some("Benfica"));
        assert_eq!(interner.len(), 1);
    }
}
