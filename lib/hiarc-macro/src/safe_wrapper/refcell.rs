use proc_macro::TokenStream;
use quote::quote;

use super::wrapper::hiarc_safer_wrapper;

pub(crate) fn hiarc_safer_refcell_impl(attr: TokenStream, tokens: TokenStream) -> TokenStream {
    hiarc_safer_wrapper(
        attr,
        tokens,
        |ty| quote!(hiarc::HiUnsafeRefCell<#ty>),
        |inner| quote!(hiarc::HiUnsafeRefCell::new(#inner)),
        quote!(),
        false,
    )
}
