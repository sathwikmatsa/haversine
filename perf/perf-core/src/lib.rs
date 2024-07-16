use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use lazy_static::lazy_static;
use nix::time::ClockId;

lazy_static! {
    static ref CPU_FREQ: u64 = estimate_cpu_freq(100);
    static ref TRACE_MAP: Mutex<HashMap<String, Trace>> = Mutex::new(HashMap::new());
    static ref TRACE_ORDER: Mutex<Vec<String>> = Mutex::new(Vec::new());
}

struct Trace {
    begin: Option<u64>,
    end: Option<u64>,
}

// TODO(sathwik): Use Typestate pattern https://cliffle.com/blog/rust-typestate/
impl Trace {
    fn new(begin: u64) -> Self {
        Self {
            begin: Some(begin),
            end: None,
        }
    }

    fn end(&mut self, end: u64) {
        debug_assert!(self.begin.unwrap() < end);
        self.end = Some(end);
    }

    fn delta(&self) -> u64 {
        debug_assert!(self.end.is_some());
        unsafe { self.end.unwrap_unchecked() - self.begin.unwrap_unchecked() }
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

/// Captures the timestamp counter for the start of `id`
///
/// # Panics
///
/// Repeated call with the same `id` will panic
fn trace_begin(id: &str) {
    let mut trace_map = TRACE_MAP.lock().unwrap();
    debug_assert!(
        !trace_map.contains_key(id),
        "Trace already initiated for {id}"
    );
    trace_map.insert(id.to_owned(), Trace::new(read_cpu_timer()));
    let mut trace_order = TRACE_ORDER.lock().unwrap();
    trace_order.push(id.to_owned());
}

/// Captures the timestamp counter for the end of `id`
///
/// # Panics
///
/// Calling this function without prior call to `trace_begin` will panic.
/// Repeated call with the same `id` will panic.
fn trace_end(id: &str) {
    let mut trace_map = TRACE_MAP.lock().unwrap();
    let trace = trace_map.get_mut(id);
    debug_assert!(trace.is_some(), "Trace not initialized for {id}");
    if let Some(t) = trace {
        debug_assert!(t.end.is_none(), "Trace already ended for {id}");
        t.end(read_cpu_timer());
    }
}

fn start_ts() -> &'static u64 {
    static START_TS: OnceLock<u64> = OnceLock::new();
    START_TS.get_or_init(|| read_cpu_timer())
}

fn end_ts() -> &'static u64 {
    static END_TS: OnceLock<u64> = OnceLock::new();
    END_TS.get_or_init(|| read_cpu_timer())
}

pub fn begin_profile() {
    let _ = start_ts();
}

/// Prints the stats for captures traces to stdout
///
/// # Panics
///
/// Will panic if any of the traces were not ended.
#[allow(clippy::cast_precision_loss)]
pub fn end_and_print_profile() {
    let end = *end_ts();
    let start = *start_ts();
    assert!(end > start, "ERROR: Profile end time is earlier than start time. `begin_profile` call should precede `end_and_print_profile` call.");
    let cpu_time: u64 = end - start;
    let cpu_freq = *CPU_FREQ;
    let total_time_ms: f64 = (1000f64 * cpu_time as f64) / cpu_freq as f64;
    println!("Total time: {total_time_ms} ms (CPU freq {cpu_freq})");
    let trace_map = TRACE_MAP.lock().unwrap();
    let trace_order = TRACE_ORDER.lock().unwrap();
    for trace_key in trace_order.iter() {
        let trace = trace_map.get(trace_key).unwrap();
        let section_time = trace.delta();
        let percent = (section_time as f64 / cpu_time as f64) * 100.0;
        println!("  {trace_key}: {section_time} ({percent:.2}%)");
    }
}

pub struct ScopedTrace {
    ident: String,
}

impl ScopedTrace {
    pub fn new(ident: impl AsRef<str>) -> Self {
        trace_begin(ident.as_ref());
        Self {
            ident: ident.as_ref().to_owned(),
        }
    }
}

impl Drop for ScopedTrace {
    fn drop(&mut self) {
        trace_end(self.ident.as_str());
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
