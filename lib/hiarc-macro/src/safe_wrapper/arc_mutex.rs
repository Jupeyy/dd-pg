use proc_macro::TokenStream;
use quote::quote;

use super::wrapper::hiarc_safer_wrapper;

pub(crate) fn hiarc_safer_arc_mutex_impl(attr: TokenStream, tokens: TokenStream) -> TokenStream {
    hiarc_safer_wrapper(
        attr,
        tokens,
        |ty| quote!(std::sync::Arc<hiarc::HiUnsafeMutex<#ty>>),
        |inner| quote!(std::sync::Arc::new(hiarc::HiUnsafeMutex::new(#inner))),
        quote!(std::sync::Arc),
        true,
    )
}
