use std::collections::HashSet;
use std::sync::Arc;

pub struct StringInterner {
    strings: HashSet<Arc<str>>,
}

impl StringInterner {
    pub fn new() -> Self {
        StringInterner {
            strings: HashSet::new(),
        }
    }

    pub fn intern(&mut self, s: &str) -> Arc<str> {
        if let Some(existing) = self.strings.get(s) {
            return existing.clone();
        }

        let interned: Arc<str> = Arc::from(s);
        self.strings.insert(interned.clone());
        interned
    }
}
