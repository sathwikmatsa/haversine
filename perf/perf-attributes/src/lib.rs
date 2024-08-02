#[cfg(feature = "perf")]
use {
    quote::quote,
    syn::{parse_quote, Error, Expr, ItemFn, LitStr},
};

/// Safety: Cannot be used in a multi-threaded context
#[proc_macro_attribute]
#[cfg(feature = "perf")]
pub fn instrument(
    _args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut input = syn::parse_macro_input!(item as ItemFn);
    let block = input.block.as_mut();
    block.stmts.insert(
        0,
        parse_quote! { let __trace_fn = perf::ScopedTrace::new_fn(perf::function_name!());},
    );
    let gen = quote! {#input};
    gen.into()
}

#[proc_macro_attribute]
#[cfg(not(feature = "perf"))]
pub fn instrument(
    _args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    item
}

/// Safety: Cannot be used in a multi-threaded context
#[proc_macro_attribute]
#[cfg(feature = "perf")]
pub fn instrument_loop(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as Expr);
    if !matches!(input, Expr::ForLoop(_) | Expr::While(_) | Expr::Loop(_)) {
        // TODO(sathwik): Improve error diagnostics
        return Error::new_spanned(input, "Expected a loop construct")
            .to_compile_error()
            .into();
    }
    let loop_name = syn::parse_macro_input!(args as LitStr).value();
    let gen = quote! {{
        let __trace_loop = perf::ScopedTrace::new_loop(perf::function_name!(), #loop_name);
        #input
    }};
    gen.into()
}

#[proc_macro_attribute]
#[cfg(not(feature = "perf"))]
pub fn instrument_loop(
    _args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    item
}
