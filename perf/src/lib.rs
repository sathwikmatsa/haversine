pub use perf_attributes::*;
pub use perf_core::*;

#[macro_export]
macro_rules! trace_section {
    ($name:expr, $($s:stmt);+ $(;)?) => {
        let __trace_section = perf::ScopedTrace::new($name);
        $($s)*
        drop(__trace_section);
    };
}
