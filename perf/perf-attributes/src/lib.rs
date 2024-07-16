use proc_macro2::Span;
use quote::quote;
use syn::{parse_quote, Ident, ItemFn};

#[proc_macro_attribute]
pub fn instrument(
    _args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut input = syn::parse_macro_input!(item as ItemFn);
    let fn_name = input.sig.ident.to_string();
    let trace_name = format!("fn::{}", fn_name);
    let tracer_var = Ident::new(format!("_trace_{}", fn_name).as_str(), Span::call_site());
    let block = input.block.as_mut();
    block.stmts.insert(
        0,
        parse_quote! { let #tracer_var = perf::ScopedTrace::new(#trace_name);},
    );
    let gen = quote! {#input};
    gen.into()
}
