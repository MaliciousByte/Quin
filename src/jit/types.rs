use crate::value::{Function, Value};
use crate::chunk::OpCode;

/// JIT type lattice for speculative optimization.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum JitType { Unknown, ProvenInt, ProvenFloat }

impl super::JitEngine {
    pub(crate) fn infer_types(&self, function: &Function, start_depth: usize) -> Vec<JitType> {
        let n = (start_depth + function.max_locals + 64).max(128);
        let mut vt = vec![JitType::ProvenInt; n];
        let chunk = &function.chunk;
        // Function args/closure are Unknown type (caller decides)
        for i in 0..=function.arity { vt[i] = JitType::Unknown; }

        // Fixed-point: propagate type constraints
        let mut changed = true;
        while changed {
            changed = false;
            let mut stype: Vec<JitType> = (0..n).map(|i| vt[i]).collect();
            let mut d = start_depth;

            for op in &chunk.code {
                let set = |stype: &mut Vec<JitType>, idx: usize, t: JitType| {
                    if idx < stype.len() { stype[idx] = t; }
                };
                match op {
                    OpCode::Constant(idx) => {
                        let raw = chunk.constants[*idx].0;
                        let t = if Value(raw).is_int() { JitType::ProvenInt }
                                else if Value(raw).is_float() { JitType::ProvenFloat }
                                else { JitType::Unknown };
                        set(&mut stype, d, t); d += 1;
                    }
                    OpCode::Null|OpCode::True|OpCode::False => { set(&mut stype, d, JitType::Unknown); d += 1; }
                    OpCode::Pop => { if d > start_depth { d -= 1; } }
                    OpCode::Dup => {
                        let t = if d > 0 { stype.get(d-1).copied().unwrap_or(JitType::Unknown) } else { JitType::Unknown };
                        set(&mut stype, d, t); d += 1;
                    }
                    OpCode::GetLocal(i) => {
                        let t = if *i < vt.len() { vt[*i] } else { JitType::Unknown };
                        set(&mut stype, d, t); d += 1;
                    }
                    OpCode::SetLocal(i) => {
                        let t = if d > 0 { stype.get(d-1).copied().unwrap_or(JitType::Unknown) } else { JitType::Unknown };
                        if *i < vt.len() {
                            if vt[*i] == JitType::ProvenInt && t != JitType::ProvenInt {
                                vt[*i] = if t == JitType::ProvenFloat { JitType::ProvenFloat } else { JitType::Unknown };
                                changed = true;
                            } else if vt[*i] == JitType::ProvenFloat && t != JitType::ProvenFloat {
                                vt[*i] = JitType::Unknown; changed = true;
                            }
                        }
                        // PEEK — d unchanged
                    }
                    OpCode::Add|OpCode::Subtract|OpCode::Multiply|OpCode::Divide => {
                        if d >= 2 {
                            let bv = stype.get(d-1).copied().unwrap_or(JitType::Unknown);
                            let av = stype.get(d-2).copied().unwrap_or(JitType::Unknown);
                            d -= 1;
                            let r = if av == JitType::ProvenFloat || bv == JitType::ProvenFloat {
                                JitType::ProvenFloat
                            } else if av == JitType::ProvenInt && bv == JitType::ProvenInt {
                                JitType::ProvenInt
                            } else { JitType::Unknown };
                            set(&mut stype, d-1, r);
                        }
                    }
                    OpCode::Equal|OpCode::Greater|OpCode::Less => {
                        if d >= 2 { d -= 1; set(&mut stype, d-1, JitType::Unknown); }
                    }
                    OpCode::Not|OpCode::Negate => { if d > 0 { set(&mut stype, d-1, JitType::Unknown); } }
                    OpCode::JumpIfFalse(_) => {} // PEEK — no change
                    OpCode::Return => { d = start_depth; }
                    // GetGlobal: pushes Unknown (could be anything)
                    OpCode::GetGlobal(_) => { set(&mut stype, d, JitType::Unknown); d += 1; }
                    // GetIndex: pops 2, pushes 1 Unknown
                    OpCode::GetIndex => { if d >= 2 { d -= 1; set(&mut stype, d-1, JitType::Unknown); } }
                    // SetIndex: pops 3, pushes 1 Unknown
                    OpCode::SetIndex => { if d >= 3 { d -= 2; set(&mut stype, d-1, JitType::Unknown); } }
                    // Call(n): pops n+1, pushes 1 Unknown
                    OpCode::Call(n) => {
                        let n_args = *n as usize;
                        if d >= n_args + 1 { d -= n_args; set(&mut stype, d-1, JitType::Unknown); }
                    }
                    _ => {}
                }
            }
        }
        vt
    }
}
