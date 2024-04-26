use std::str::FromStr;

use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse_macro_input, parse_quote, Attribute, FnArg, GenericParam, Item, Meta, Pat, Type,
    Visibility,
};

pub(crate) fn hiarc_safer_wrapper(
    attr: TokenStream,
    tokens: TokenStream,
    wrapper_ty: impl Fn(&proc_macro2::TokenStream) -> proc_macro2::TokenStream,
    wrapper_new: impl Fn(&proc_macro2::TokenStream) -> proc_macro2::TokenStream,
    outer_ty: proc_macro2::TokenStream,
    can_clone: bool,
) -> TokenStream {
    let mut base = parse_macro_input!(tokens as Item);

    let sync_send_wrapper = attr.to_string().contains("sync_send_wrapper");

    let struct_wrapper: proc_macro2::TokenStream;

    match &mut base {
        Item::Impl(i) => {
            let mut item_wrapper = i.clone();
            let self_ty = proc_macro2::TokenStream::from_str(&format!(
                "{}Impl",
                i.self_ty.to_token_stream().to_string()
            ))
            .unwrap();
            i.self_ty = Box::new(parse_quote!(#self_ty));
            // remove attributes of this macro
            i.items.iter_mut().for_each(|item| match item {
                syn::ImplItem::Fn(f) => {
                    f.attrs.retain_mut(|attr| {
                        let meta = attr.meta.to_token_stream().to_string();
                        if meta == "hiarc_force_impl" {
                            false
                        } else if meta == "hiarc_trait_is_immutable_self" {
                            false
                        } else {
                            true
                        }
                    });
                }
                _ => {}
            });
            let should_implement_wrapper = match &mut i.trait_ {
                Some((_, path, _)) => {
                    if path.is_ident(&format_ident!("Drop")) {
                        false
                    } else {
                        if !path.is_ident(&format_ident!("Default")) {
                            i.trait_ = None;
                        }
                        true
                    }
                }
                None => true,
            };
            if should_implement_wrapper {
                let self_ty = i.self_ty.to_token_stream();
                item_wrapper.items.retain_mut(|item| {
                    match item {
                        syn::ImplItem::Fn(f) => {
                            let (should_implement_fn, found_trait_is_immutable_self) = match &f.vis {
                                Visibility::Restricted(_) | Visibility::Public(_) => {
                                    (true, false)
                                }
                                _ => {
                                    // ignore rest, except it's a trait, or it has a attr called `hiarc_force_impl`
                                    let mut found_force = false;
                                    let mut found_trait_is_immutable_self = false;
                                    f.attrs.retain_mut(|attr| {
                                        let meta = attr.meta.to_token_stream().to_string();
                                        if meta == "hiarc_force_impl" {
                                            found_force = true;
                                            false
                                        }
                                        else if meta == "hiarc_trait_is_immutable_self" {
                                            found_trait_is_immutable_self = true;
                                            false
                                        }
                                        else {
                                            true
                                        }
                                    });
                                    let should_implement_fn = found_force || item_wrapper.trait_.is_some();
                                    (should_implement_fn, found_trait_is_immutable_self)
                                }
                            };
                            if should_implement_fn {
                                // for pub(_) functions implement the wrapper
                                let (has_self_arg, self_is_mutable) =
                                    if let Some(FnArg::Receiver(r)) = f.sig.inputs.iter_mut().next() {
                                        // if fn is not from a trait, rewrite it to use only &self instead of &mut self
                                        let is_mutable = r.mutability.is_some();
                                        if (!item_wrapper.trait_.is_some() || found_trait_is_immutable_self) && is_mutable {
                                            r.mutability = None;
                                            if let Type::Reference(ty_ref) = r.ty.as_mut() {
                                                ty_ref.mutability = None;
                                            }
                                        }
                                        (true, is_mutable)
                                    } else {
                                        (false, false)
                                    };

                                // if it has a self arg, simply call the function as self.0.[...]
                                let func_ident = &f.sig.ident;
                                let func_ident_str = f.sig.ident.to_string();
                                let mut inputs = f.sig.inputs.iter_mut();
                                if has_self_arg {
                                    // skip self
                                    inputs.next();
                                }
                                let args: Vec<(Ident, Box<Type>)> = inputs
                                    .map(|input| match input {
                                        FnArg::Receiver(_) => {
                                            panic!("multiple self are not allowed")
                                        }
                                        FnArg::Typed(t) => {
                                            let Pat::Ident(pat_ident) = t.pat.as_mut() else {
                                                panic!("this pattern is not yet supported: {:?}", t.pat.to_token_stream())
                                            };
                                            pat_ident.mutability = None;
                                        (format_ident!("{}", pat_ident.ident.to_token_stream().to_string()), t.ty.clone())
                                        }
                                    })
                                    .collect();

                                let (args, arg_types): (Vec<Ident>, Vec<Box<Type>>) = args.into_iter().unzip();

                                let arg_checks: Vec<proc_macro2::TokenStream> = arg_types.into_iter().map(|arg| {
                                    let found_generic =  f.sig.generics.params.iter().find(|g| {
                                        if let GenericParam::Type(ty) = g {
                                            ty.ident.to_token_stream().to_string() == arg.to_token_stream().to_string()
                                        }
                                        else {
                                            false
                                        }
                                    });
                                    let ident = if let Some(generic) = found_generic {
                                        // if arg is generic use a little trick to check if hiarc fits
                                        let GenericParam::Type(ty) = generic else {
                                            panic!("Generic params of types other than Type are not supported");
                                        };
                                        ty.ident.to_token_stream()
                                    }
                                    else {
                                        arg.to_token_stream()
                                    };
                                    let ident_str = ident.to_string();
                                    quote! {
                                        {
                                            fn internal_hi_test<A: hiarc::HiarcTrait>() -> u64 {
                                                struct InternalHiarcTest<const N: u64, A> {
                                                    d: A,
                                                }

                                                unsafe impl<const N: u64, A: hiarc::HiarcTrait> hiarc::HiarcTrait for InternalHiarcTest<N, A> {
                                                    const HI_VAL: u64 = {
                                                        assert!(N >= <A as hiarc::HiarcTrait>::HI_VAL, concat!("evaluation of the hierarchical values indicate that \"", #ident_str, "\", in the function \"", concat!(#func_ident_str, "\", is a higher level component (hierarchically). So it cannot be passed as parameter to a function in this component")));
                                                        N - <A as hiarc::HiarcTrait>::HI_VAL
                                                    };
                                                }

                                                <InternalHiarcTest::<{<#self_ty as hiarc::HiarcTrait>::HI_VAL}, A> as hiarc::HiarcTrait>::HI_VAL
                                            }
                                            // this is important so the compiler does not see the function as unused and skips evaluation
                                            assert!(internal_hi_test::<#ident>() >= 0);
                                        }
                                    }
                                }).collect();

                                // remove trait bounds from generics, so it can be used for the wrapper function calls
                                let mut generics_simplified = f.sig.generics.clone();
                               let generics_params: Vec<proc_macro2::TokenStream> = generics_simplified.params.iter_mut().map(|g| {
                                  match g {
                                    GenericParam::Lifetime(lf) => {
                                        lf.bounds = Default::default();
                                        g.to_token_stream()
                                    },
                                    GenericParam::Type(t) => {
                                        t.bounds = Default::default();
                                        g.to_token_stream()
                                    },
                                    GenericParam::Const(c) => {
                                        c.ident.to_token_stream()
                                    },
                                }
                                }).collect();

                                let generics_param = if f.sig.generics.params.is_empty() {
                                    quote!()
                                }
                                else {
                                    quote!(::<#(#generics_params),*>)
                                };

                                // to all typed generics, add hiarc trait bound
                                f.sig.generics.params.iter_mut().for_each(|g| {
                                    if let GenericParam::Type(t) = g {
                                        t.colon_token = Some(Default::default());
                                        t.bounds.push(parse_quote!(hiarc::HiarcTrait));
                                    }
                                });

                                if has_self_arg {
                                    let asserts_quote = quote! {
                                        #(#arg_checks)*
                                    };
                                    if self_is_mutable {
                                        f.block.stmts = parse_quote! {
                                            #asserts_quote
                                            unsafe { self.0.hi_borrow_mut() }.#func_ident #generics_param(#(#args),*)
                                        };
                                    }
                                    else {
                                        f.block.stmts = parse_quote! {
                                            #asserts_quote
                                            unsafe { self.0.borrow() }.#func_ident #generics_param(#(#args),*)
                                        };
                                    }
                                } else {
                                    // check if the function is called "new"
                                    if f.sig.ident == "new"  {
                                        let is_result = f.sig.output.to_token_stream().to_string().replace(" ", "").contains("Result<");
                                        let new_tokens = if is_result {
                                            wrapper_new(&quote!(#self_ty::new(#(#args),*)?))
                                        }
                                        else {
                                            wrapper_new(&quote!(#self_ty::new(#(#args),*)))
                                        };
                                        f.block.stmts = if is_result {
                                            parse_quote! {
                                                Ok(Self(#new_tokens))
                                            }
                                        }
                                        else {
                                            parse_quote! {
                                                Self(#new_tokens)
                                            }
                                        }
                                    }
                                    // same for "default"
                                    else if f.sig.ident == "default" {
                                        let new_tokens = wrapper_new(&quote!(#self_ty::default(#(#args),*)));
                                        f.block.stmts = parse_quote! {
                                            Self(#new_tokens)
                                        };
                                    }
                                    // else don't implement it
                                }
                            }
                            should_implement_fn
                        }
                        _ => {
                            // ignore
                            false
                        }
                    }
                });
                struct_wrapper = item_wrapper.to_token_stream();
            } else {
                struct_wrapper = quote! {};
            }
        }
        Item::Struct(s) => {
            let ident = s.ident.clone();
            let mut attrs: Vec<Attribute> = s
                .attrs
                .clone()
                .into_iter()
                .filter(|attr| {
                    let derive = attr.to_token_stream().to_string().replace(" ", "");
                    !(derive == "#[derive(hiarc::Hiarc)]" || derive == "#[derive(Hiarc)]")
                })
                .collect();
            attrs.iter_mut().for_each(|attr| {
                if let Meta::List(l) = &mut attr.meta {
                    if l.path.get_ident().unwrap().to_string() == "derive" {
                        if l.tokens.to_string().contains("Hiarc") {
                            // TODO: refactor this trash
                            let filtered = l.tokens.to_string().replace(" ", "");
                            let filtered = filtered.to_string().replace("hiarc::Hiarc,", "");
                            let filtered = filtered.replace(",hiarc::Hiarc", "");
                            let filtered = filtered.replace("hiarc::Hiarc", "");
                            let filtered = filtered.replace("Hiarc,", "");
                            let filtered = filtered.replace(",Hiarc", "");
                            let filtered = filtered.replace("Hiarc", "");

                            l.tokens = TokenStream::from_str(&filtered).unwrap().into();
                        }
                    }
                }
            });
            s.ident = format_ident!("{}Impl", s.ident);
            let ident_inner = &s.ident;

            // collect const generics
            let const_generics: Vec<_> = s.generics.params.iter().map(|gen| {
                if let GenericParam::Const(gen) = gen {
                    (gen.to_token_stream(), gen.ident.to_token_stream())
                }
                else {
                    panic!("generics that are not const (e.g. typed generics) are currently not supported.");
                }
            }).collect();
            let (const_generics, const_generics_names): (Vec<_>, Vec<_>) =
                const_generics.into_iter().unzip();
            let const_generics = if const_generics.is_empty() {
                quote!()
            } else {
                quote! {
                    <#(#const_generics),*>
                }
            };
            let const_generics_names = if const_generics_names.is_empty() {
                quote!()
            } else {
                quote! {
                    <#(#const_generics_names),*>
                }
            };

            let wrapper_vis = s.vis.clone();

            // TODO: change to private, collides with rc_sync_send_wrapper_ident: s.vis = Visibility::Inherited;

            let hiarc_quote = quote! {
                unsafe impl hiarc::HiarcTrait for #ident {
                    const HI_VAL: u64 = <#ident_inner as hiarc::HiarcTrait>::HI_VAL + 1;
                }
            };

            let inner_ty = wrapper_ty(&quote!(#ident_inner #const_generics_names));

            let clone_impl = if !can_clone {
                quote!()
            } else {
                quote!(impl Clone for #ident {
                    fn clone(&self) -> Self {
                        Self(self.0.clone())
                    }
                })
            };

            let (rc_special_impl, rc_special_ty) = if outer_ty.to_string()
                == quote!(std::rc::Rc).to_string()
                && sync_send_wrapper
            {
                let rc_sync_send_wrapper_ident = format_ident!("{}SyncSend", ident);
                (
                    quote! {
                        /// tries to place the inner value into a thread safe wrapper, it can later be
                        /// "reimported" into a hiarc wrapper using [`from_sync_send_wrapper`]
                        pub fn try_into_sync_send_wrapper(self) -> Result<#rc_sync_send_wrapper_ident, hiarc::HiUnsafeSyncSendCellCastError> {
                            <hiarc::HiUnsafeSyncSendCell<_>>::from_rc(self.0)
                        }

                        /// from a sync send wrapper create the original value again
                        pub fn from_sync_send_wrapper(val: #rc_sync_send_wrapper_ident) -> #ident {
                            #ident(val.into_rc_unsafe_cell())
                        }
                    },
                    quote!(pub type #rc_sync_send_wrapper_ident = hiarc::HiUnsafeSyncSendCell<#ident_inner>;),
                )
            } else {
                (quote!(), quote!())
            };

            struct_wrapper = quote! {
                #(#attrs)*
                #wrapper_vis struct #ident #const_generics(#inner_ty);

                #rc_special_ty

                #clone_impl

                impl #ident {
                    /// tries to replace the inner value with the inner value of `other`
                    /// if other is already in use, this function fails
                    pub fn try_replace_inner(&self, other: #ident) -> Result<#ident, String> {
                        unsafe { std::mem::swap(&mut *self.0.hi_borrow_mut(), &mut *other.0.hi_borrow_mut()) };
                        Ok(other)
                    }

                    #rc_special_impl
                }

                #hiarc_quote
            };
        }
        _ => panic!("only works on structs & their impl for now"),
    }

    let mut res = base.to_token_stream();

    res.extend(struct_wrapper);

    res.into()
}
