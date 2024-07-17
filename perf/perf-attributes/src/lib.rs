use quote::quote;
use syn::{parse_quote, Error, Expr, ItemFn, LitStr};

#[proc_macro_attribute]
pub fn instrument(
    _args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut input = syn::parse_macro_input!(item as ItemFn);
    let fn_name = input.sig.ident.to_string();
    let trace_name = format!("fn::{}", fn_name);
    let block = input.block.as_mut();
    block.stmts.insert(
        0,
        parse_quote! { let __trace_fn = perf::ScopedTrace::new(#trace_name);},
    );
    let gen = quote! {#input};
    gen.into()
}

#[proc_macro_attribute]
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
    let trace_name = format!("loop::{}", loop_name);
    let gen = quote! {{
        let __trace_loop = perf::ScopedTrace::new(#trace_name);
        #input
    }};
    gen.into()
}
