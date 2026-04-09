use std::sync::Arc;
use std::cell::RefCell;
use crate::value::Upvalue;
use super::VM;

impl VM {
    pub(crate) fn capture_upvalue(&mut self, index: usize) -> Arc<RefCell<Upvalue>> {
        for upvalue in &self.open_upvalues {
            if upvalue.borrow().index == index {
                return upvalue.clone();
            }
        }

        let upvalue = Arc::new(RefCell::new(Upvalue {
            index,
            closed: None,
        }));
        self.open_upvalues.push(upvalue.clone());
        upvalue
    }

    pub(crate) fn close_upvalues(&mut self, last_idx: usize) {
        let mut i = 0;
        while i < self.open_upvalues.len() {
            let upvalue_rc = self.open_upvalues[i].clone();
            if upvalue_rc.borrow().index >= last_idx {
                let val = self.stack[upvalue_rc.borrow().index].clone();
                upvalue_rc.borrow_mut().closed = Some(val);
                self.open_upvalues.remove(i);
            } else {
                i += 1;
            }
        }
    }
}
