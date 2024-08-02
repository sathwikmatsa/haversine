pub use perf_attributes::*;
pub use perf_core::*;

// Stolen from: https://docs.rs/stdext/0.3.3/src/stdext/macros.rs.html#63-74
#[macro_export]
macro_rules! function_name {
    () => {{
        fn f() {}
        fn type_name_of<T>(_: T) -> &'static str {
            std::any::type_name::<T>()
        }
        let name = type_name_of(f);
        // `3` is the length of the `::f`.
        &name[..name.len() - 3]
    }};
}

/// Safety: Cannot be used in a multi-threaded context
#[cfg(feature = "perf")]
#[macro_export]
macro_rules! trace_section {
    ($name:expr, $($s:stmt);+ $(;)?) => {
        let __trace_section = perf::ScopedTrace::new_section(perf::function_name!(), $name);
        $($s)*
        drop(__trace_section);
    };
}

#[cfg(not(feature = "perf"))]
#[macro_export]
macro_rules! trace_section {
    ($name:expr, $($s:stmt);+ $(;)?) => {
        $($s)*
    };
}
