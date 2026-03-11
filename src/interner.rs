use std::collections::HashSet;
use std::rc::Rc;

pub struct StringInterner {
    strings: HashSet<Rc<str>>,
}

impl StringInterner {
    pub fn new() -> Self {
        StringInterner {
            strings: HashSet::new(),
        }
    }

    pub fn intern(&mut self, s: &str) -> Rc<str> {
        if let Some(existing) = self.strings.get(s) {
            return existing.clone();
        }

        let interned: Rc<str> = Rc::from(s);
        self.strings.insert(interned.clone());
        interned
    }
}
