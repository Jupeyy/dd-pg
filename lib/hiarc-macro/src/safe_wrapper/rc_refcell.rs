use proc_macro::TokenStream;
use quote::quote;

use super::wrapper::hiarc_safer_wrapper;

pub(crate) fn hiarc_safer_rc_refcell_impl(attr: TokenStream, tokens: TokenStream) -> TokenStream {
    hiarc_safer_wrapper(
        attr,
        tokens,
        |ty| quote!(std::rc::Rc<hiarc::HiUnsafeRefCell<#ty>>),
        |inner| quote!(std::rc::Rc::new(hiarc::HiUnsafeRefCell::new(#inner))),
        quote!(std::rc::Rc),
        true,
    )
}
