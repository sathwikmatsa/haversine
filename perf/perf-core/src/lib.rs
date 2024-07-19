use std::{
    collections::HashMap,
    fmt::Display,
    sync::{atomic::AtomicUsize, Mutex, OnceLock},
};

use lazy_static::lazy_static;
use nix::time::ClockId;

lazy_static! {
    static ref CPU_FREQ: u64 = estimate_cpu_freq(100);
    static ref TRACE_MAP: Mutex<HashMap<TraceId, Trace>> = Mutex::new(HashMap::with_capacity(4096));
}

static mut TRACE_ID: AtomicUsize = AtomicUsize::new(0);

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
            order: unsafe { TRACE_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst) },
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

fn start_ts() -> &'static u64 {
    static START_TS: OnceLock<u64> = OnceLock::new();
    START_TS.get_or_init(|| read_cpu_timer())
}

pub struct ScopedTrace {
    trace_id: TraceId,
}

impl ScopedTrace {
    fn new(trace_id: TraceId) -> Self {
        let mut trace_map = TRACE_MAP.lock().unwrap();
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
        let mut trace_map = TRACE_MAP.lock().unwrap();
        let trace = trace_map.get_mut(&self.trace_id).unwrap();
        trace.elapsed += read_cpu_timer() - trace.begin;
        trace.hit_count += 1;
    }
}

/// Initializes profile environment.
/// Ideally, this should be invoked during program start up.
pub fn begin_profile() {
    // initialize lazy statics
    lazy_static::initialize(&CPU_FREQ);
    lazy_static::initialize(&TRACE_MAP);

    // capture profile start time
    let _ = start_ts();
}

/// Prints the perf timings of captured traces to stdout
///
/// # Panics
///
/// Will panic if `begin_profile` is not invoked before calling this fn
#[allow(clippy::cast_precision_loss)]
pub fn end_and_print_profile() {
    let end = read_cpu_timer();
    let start = *start_ts();
    assert!(end > start, "ERROR: Profile end time is earlier than start time. `begin_profile` call should precede `end_and_print_profile` call.");

    let cpu_time: u64 = end - start;
    let cpu_freq = *CPU_FREQ;
    let total_time_ms: f64 = (1000f64 * cpu_time as f64) / cpu_freq as f64;
    println!("Total time: {total_time_ms} ms (CPU freq {cpu_freq})");

    let trace_map = TRACE_MAP.lock().unwrap();
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
