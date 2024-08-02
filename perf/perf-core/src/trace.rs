use crate::racy_unsafe_cell::RacyUnsafeCell;
use std::{cell::OnceCell, collections::HashMap, fmt::Display, hash::Hash};

pub static CURRENT_TRACE: RacyUnsafeCell<Option<TraceId>> = RacyUnsafeCell::new(None);
pub static TRACE_ID: RacyUnsafeCell<usize> = RacyUnsafeCell::new(0);

pub unsafe fn trace_map() -> &'static mut HashMap<TraceId, Trace> {
    static CELL: RacyUnsafeCell<OnceCell<HashMap<TraceId, Trace>>> =
        RacyUnsafeCell::new(OnceCell::new());
    (*CELL.get()).get_mut_or_init(|| HashMap::with_capacity(4096))
}

#[derive(PartialEq, Eq, Hash, Copy, Clone)]
pub enum TraceType {
    Fn,
    Loop(&'static str),
    Section(&'static str),
}

#[derive(PartialEq, Eq, Hash, Copy, Clone)]
pub struct TraceId {
    pub enclosing_function_name: &'static str,
    pub ty: TraceType,
}

impl Display for TraceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.ty {
            TraceType::Fn => write!(f, "{}::fn", self.enclosing_function_name),
            TraceType::Loop(lname) => {
                write!(f, "{}::{}::loop", self.enclosing_function_name, lname)
            }
            TraceType::Section(sname) => {
                write!(f, "{}::{}::section", self.enclosing_function_name, sname)
            }
        }
    }
}

pub struct Trace {
    /// without children
    pub elapsed_exclusive: i64,
    /// with children
    pub elapsed_inclusive: u64,
    pub hit_count: usize,
    pub order: usize,
}

impl Default for Trace {
    fn default() -> Self {
        Self {
            elapsed_exclusive: 0,
            elapsed_inclusive: 0,
            hit_count: 0,
            order: unsafe {
                let id = TRACE_ID.get();
                *id += 1;
                *id
            },
        }
    }
}
