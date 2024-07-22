#![feature(once_cell_get_mut)]

use std::{
    cell::{OnceCell, UnsafeCell},
    collections::HashMap,
    fmt::Display,
    hash::Hash,
};

use nix::time::ClockId;

#[repr(transparent)]
struct RacyUnsafeCell<T>(UnsafeCell<T>);

unsafe impl<T> Sync for RacyUnsafeCell<T> {}

impl<T> RacyUnsafeCell<T> {
    const fn new(x: T) -> Self {
        RacyUnsafeCell(UnsafeCell::new(x))
    }

    fn get(&self) -> *mut T {
        self.0.get()
    }
}

static TRACE_ID: RacyUnsafeCell<usize> = RacyUnsafeCell::new(0);

unsafe fn trace_map() -> &'static mut HashMap<TraceId, Trace> {
    static CELL: RacyUnsafeCell<OnceCell<HashMap<TraceId, Trace>>> =
        RacyUnsafeCell::new(OnceCell::new());
    (*CELL.get()).get_mut_or_init(|| HashMap::with_capacity(4096))
}

unsafe fn cpu_freq() -> u64 {
    static CELL: RacyUnsafeCell<OnceCell<u64>> = RacyUnsafeCell::new(OnceCell::new());
    *(*CELL.get()).get_or_init(|| estimate_cpu_freq(100))
}

#[derive(PartialEq, Eq, Hash, Copy, Clone)]
enum TraceType {
    Fn,
    Loop(&'static str),
    Section(&'static str),
}

#[derive(PartialEq, Eq, Hash, Copy, Clone)]
struct TraceId {
    enclosing_function_name: &'static str,
    ty: TraceType,
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

struct Trace {
    begin: u64,
    elapsed: u64,
    hit_count: usize,
    order: usize,
}

impl Default for Trace {
    fn default() -> Self {
        Self {
            begin: 0,
            elapsed: 0,
            hit_count: 0,
            order: unsafe {
                let id = TRACE_ID.get();
                *id += 1;
                *id
            },
        }
    }
}

impl Trace {
    fn reset(&mut self, begin: u64) {
        self.begin = begin;
    }
}

fn get_os_timer_freq() -> u64 {
    1_000_000_000
}

#[allow(clippy::cast_sign_loss)]
fn read_os_timer() -> u64 {
    // https://berthub.eu/articles/posts/on-linux-vdso-and-clockgettime/
    let cur = ClockId::CLOCK_REALTIME.now().expect("Get current clock");
    cur.tv_sec() as u64 * get_os_timer_freq() + cur.tv_nsec() as u64
}

fn read_cpu_timer() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

fn estimate_cpu_freq(millis_to_wait: u64) -> u64 {
    let os_freq = get_os_timer_freq();
    let cpu_start = read_cpu_timer();
    let os_start = read_os_timer();

    let mut os_end;
    let mut os_elapsed = 0u64;
    let os_wait_time = os_freq * millis_to_wait / 1000;
    while os_elapsed < os_wait_time {
        os_end = read_os_timer();
        os_elapsed = os_end - os_start;
    }

    let cpu_end = read_cpu_timer();
    let cpu_elapsed = cpu_end - cpu_start;
    os_freq * cpu_elapsed / os_elapsed
}

unsafe fn start_ts() -> u64 {
    static CELL: RacyUnsafeCell<OnceCell<u64>> = RacyUnsafeCell::new(OnceCell::new());
    *(*CELL.get()).get_or_init(|| read_cpu_timer())
}

/// # Safety
///
/// This struct is only safe to be used in single-threaded program.
pub struct ScopedTrace {
    trace_id: TraceId,
}

impl ScopedTrace {
    fn new(trace_id: TraceId) -> Self {
        let trace_map = unsafe { trace_map() };
        let trace = trace_map.entry(trace_id).or_default();
        trace.reset(read_cpu_timer());
        Self { trace_id }
    }

    pub fn new_fn(fn_name: &'static str) -> Self {
        let trace_id = TraceId {
            enclosing_function_name: fn_name,
            ty: TraceType::Fn,
        };
        Self::new(trace_id)
    }

    pub fn new_loop(fn_name: &'static str, loop_name: &'static str) -> Self {
        let trace_id = TraceId {
            enclosing_function_name: fn_name,
            ty: TraceType::Loop(loop_name),
        };
        Self::new(trace_id)
    }

    pub fn new_section(fn_name: &'static str, section_name: &'static str) -> Self {
        let trace_id = TraceId {
            enclosing_function_name: fn_name,
            ty: TraceType::Section(section_name),
        };
        Self::new(trace_id)
    }
}

impl Drop for ScopedTrace {
    fn drop(&mut self) {
        let trace_map = unsafe { trace_map() };
        let trace = trace_map.get_mut(&self.trace_id).unwrap();
        trace.elapsed += read_cpu_timer() - trace.begin;
        trace.hit_count += 1;
    }
}

/// Initializes profile environment.
/// Ideally, this should be invoked during program start up.
///
/// # Safety
///
/// This function is only safe to call in single-threaded program.
/// Invoking this function in a multi-threaded program can lead to UB.
pub fn begin_profile() {
    // initialize lazy statics
    let _ = unsafe { cpu_freq() };
    let _ = unsafe { trace_map() };

    // capture profile start time
    let _ = unsafe { start_ts() };
}

/// Prints the perf timings of captured traces to stdout
///
/// # Panics
///
/// Will panic if `begin_profile` is not invoked before calling this fn
///
/// # Safety
///
/// This function is only safe to call in single-threaded program.
/// Invoking this function in a multi-threaded program can lead to UB.
#[allow(clippy::cast_precision_loss)]
pub fn end_and_print_profile() {
    let end = read_cpu_timer();
    let start = unsafe { start_ts() };
    assert!(end > start, "ERROR: Profile end time is earlier than start time. `begin_profile` call should precede `end_and_print_profile` call.");

    let cpu_time: u64 = end - start;
    let cpu_freq = unsafe { cpu_freq() };
    let total_time_ms: f64 = (1000f64 * cpu_time as f64) / cpu_freq as f64;
    println!("Total time: {total_time_ms} ms (CPU freq {cpu_freq})");

    let trace_map = unsafe { trace_map() };
    let mut trace_ids = trace_map.keys().collect::<Vec<_>>();
    trace_ids.sort_unstable_by_key(|k| trace_map.get(*k).unwrap().order);
    for trace_id in trace_ids.into_iter() {
        let trace = trace_map.get(trace_id).unwrap();
        let elapsed = trace.elapsed;
        let hits = trace.hit_count;
        let percent = (elapsed as f64 / cpu_time as f64) * 100.0;
        println!("  {trace_id}[{hits}]: {elapsed} ({percent:.2}%)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::pedantic)]
    fn playground() {
        let milliseconds_to_wait = 1_00u64;
        let os_freq = get_os_timer_freq();
        println!("OS Freq: {os_freq}");

        let cpu_start = read_cpu_timer();
        let os_start = read_os_timer();

        let mut os_end = 0u64;
        let mut os_elapsed = 0u64;
        let os_wait_time = os_freq * milliseconds_to_wait / 1000;
        while os_elapsed < os_wait_time {
            os_end = read_os_timer();
            os_elapsed = os_end - os_start;
        }

        let cpu_end = read_cpu_timer();
        let cpu_elapsed = cpu_end - cpu_start;
        let cpu_freq = os_freq * cpu_elapsed / os_elapsed;

        println!("OS Timer: {os_start} -> {os_end} = {os_elapsed}");
        println!("OS Seconds: {}", os_elapsed as f64 / os_freq as f64);

        println!("CPU Timer: {cpu_start} -> {cpu_end} = {cpu_elapsed}");
        println!("CPU Freq: {cpu_freq}");
    }
}
