#![feature(once_cell_get_mut)]

mod racy_unsafe_cell;
use racy_unsafe_cell::RacyUnsafeCell;
use std::cell::OnceCell;

use nix::time::ClockId;

#[cfg(feature = "perf")]
pub mod trace;
#[cfg(feature = "perf")]
use trace::*;

type ReadTimer = fn() -> u64;
static READ_TIMER: ReadTimer = read_cpu_timer;

unsafe fn timer_freq() -> u64 {
    static CELL: RacyUnsafeCell<OnceCell<u64>> = RacyUnsafeCell::new(OnceCell::new());
    *(*CELL.get()).get_or_init(|| estimate_timer_freq(100))
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

fn estimate_timer_freq(millis_to_wait: u64) -> u64 {
    let os_freq = get_os_timer_freq();
    let timer_start = READ_TIMER();
    let os_start = read_os_timer();

    let mut os_end;
    let mut os_elapsed = 0u64;
    let os_wait_time = os_freq * millis_to_wait / 1000;
    while os_elapsed < os_wait_time {
        os_end = read_os_timer();
        os_elapsed = os_end - os_start;
    }

    let timer_end = READ_TIMER();
    let timer_elapsed = timer_end - timer_start;
    os_freq * timer_elapsed / os_elapsed
}

unsafe fn start_ts() -> u64 {
    static CELL: RacyUnsafeCell<OnceCell<u64>> = RacyUnsafeCell::new(OnceCell::new());
    *(*CELL.get()).get_or_init(|| READ_TIMER())
}

/// # Safety
///
/// This struct is only safe to be used in single-threaded program.
#[cfg(feature = "perf")]
pub struct ScopedTrace {
    trace_id: TraceId,
    parent: Option<TraceId>,
    begin: u64,
    old_elapsed_inclusive: u64,
}

#[cfg(feature = "perf")]
impl ScopedTrace {
    fn new(trace_id: TraceId) -> Self {
        let trace_map = unsafe { trace_map() };
        let trace = trace_map.entry(trace_id).or_default();
        let begin = READ_TIMER();
        let old_elapsed_inclusive = trace.elapsed_inclusive;
        let current = CURRENT_TRACE.get();
        let parent = unsafe { *current };
        unsafe { *current = Some(trace_id) }
        Self {
            trace_id,
            parent,
            begin,
            old_elapsed_inclusive,
        }
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

#[cfg(feature = "perf")]
impl Drop for ScopedTrace {
    fn drop(&mut self) {
        let trace_map = unsafe { trace_map() };
        let trace = trace_map.get_mut(&self.trace_id).unwrap();
        let time = READ_TIMER() - self.begin;
        trace.elapsed_exclusive += time as i64;
        trace.hit_count += 1;
        trace.elapsed_inclusive = self.old_elapsed_inclusive + time;
        let current = CURRENT_TRACE.get();
        unsafe { *current = self.parent }
        if let Some(parent_trace_id) = self.parent {
            trace_map
                .get_mut(&parent_trace_id)
                .unwrap()
                .elapsed_exclusive -= time as i64;
        }
    }
}

/// Initializes profile environment.
/// Ideally, this should be invoked during program start up.
///
/// # Safety
///
/// This function is only safe to call in single-threaded program.
/// Invoking this function in a multi-threaded program can lead to UB.
#[cfg(feature = "perf")]
pub fn begin_profile() {
    // initialize lazy statics
    let _ = unsafe { timer_freq() };
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
#[cfg(feature = "perf")]
#[allow(clippy::cast_precision_loss)]
pub fn end_and_print_profile() {
    let end = READ_TIMER();
    let start = unsafe { start_ts() };
    assert!(end > start, "ERROR: Profile end time is earlier than start time. `begin_profile` call should precede `end_and_print_profile` call.");

    let timer_time: u64 = end - start;
    let timer_freq = unsafe { timer_freq() };
    let total_time_ms: f64 = (1000f64 * timer_time as f64) / timer_freq as f64;
    println!("Total time: {total_time_ms} ms (CPU freq {timer_freq})");

    let trace_map = unsafe { trace_map() };
    let mut trace_ids = trace_map.keys().collect::<Vec<_>>();
    trace_ids.sort_unstable_by_key(|k| trace_map.get(*k).unwrap().order);
    for trace_id in trace_ids.into_iter() {
        let trace = trace_map.get(trace_id).unwrap();
        let hits = trace.hit_count;
        if trace.elapsed_exclusive as u64 == trace.elapsed_inclusive {
            let elapsed = trace.elapsed_inclusive;
            let percent = (elapsed as f64 / timer_time as f64) * 100.0;
            println!("  {trace_id}[{hits}]: {elapsed} ({percent:.2}%)");
        } else {
            let percent_wo_children = (trace.elapsed_exclusive as f64 / timer_time as f64) * 100.0;
            let percent_w_children = (trace.elapsed_inclusive as f64 / timer_time as f64) * 100.0;
            let elapsed_self = trace.elapsed_exclusive;
            println!("  {trace_id}[{hits}]: {elapsed_self} ({percent_wo_children:.2}%, {percent_w_children:.2}% w/ children)");
        }
    }
}

#[cfg(not(feature = "perf"))]
pub fn begin_profile() {
    let _ = unsafe { start_ts() };
}

#[cfg(not(feature = "perf"))]
pub fn end_and_print_profile() {
    let end = READ_TIMER();
    let start = unsafe { start_ts() };
    assert!(end > start, "ERROR: Profile end time is earlier than start time. `begin_profile` call should precede `end_and_print_profile` call.");

    let cpu_time: u64 = end - start;
    let cpu_freq = unsafe { timer_freq() };
    let total_time_ms: f64 = (1000f64 * cpu_time as f64) / cpu_freq as f64;
    println!("Total time: {total_time_ms} ms (CPU freq {cpu_freq})");
}

#[cfg(not(feature = "perf"))]
pub struct ScopedTrace {}

#[cfg(not(feature = "perf"))]
impl ScopedTrace {
    pub fn new_section(_: &'static str, _: &'static str) -> Self {
        Self {}
    }
}
