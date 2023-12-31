#![allow(clippy::all)]
use std::{num::NonZeroU64, str::FromStr};

use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::{format_ident, quote, ToTokens};
use regex::Regex;
use syn::{parse_macro_input, parse_quote, FnArg, Item, Type, Visibility};

fn hi_arc_trait_impl(tokens: TokenStream, forced_hi_val: Option<NonZeroU64>) -> TokenStream {
    let base_input = parse_macro_input!(tokens as Item);

    let output;

    match &base_input {
        Item::Enum(_) => {
            todo!("enums are not yet implemented")
        }
        Item::Struct(s) => {
            let name = &s.ident;
            let generics = &s.generics;

            let field_values: Vec<proc_macro2::TokenStream> = s
                .fields
                .iter()
                .filter_map(|a| {
                    let ty = &a.ty;

                    let ty = ty.to_token_stream().to_string();
                    let ty = ty.replace(" ", "");
                    let ty = ty.replace("\n", "");

                    let collect_regex = |name: &str| {
                        let re = Regex::new(&(name.to_string() + r"<([^<>]|<.*?>)*>")).unwrap();

                        re.find_iter(&ty)
                            .map(|c| {
                                // match inner type
                                let re_inner = Regex::new(&(name.to_string() + r"<(.+)>")).unwrap();
                                re_inner
                                    .captures_iter(c.as_str())
                                    .map(|c_inner| {
                                        proc_macro2::TokenStream::from_str(
                                            &c_inner.extract::<1>().1[0].to_string(),
                                        )
                                        .unwrap()
                                    })
                                    .collect()
                            })
                            .collect()
                    };

                    let mut res: Vec<proc_macro2::TokenStream> = Default::default();
                    res.append(&mut collect_regex("HiArc"));
                    res.append(&mut collect_regex("HiRc"));
                    res.append(&mut collect_regex("HiBox"));
                    res.append(&mut collect_regex("Hi"));

                    // also collect manually attributed hiarcs
                    if let Some(attr) = a.attrs.iter().find(|attr| {
                        attr.meta
                            .to_token_stream()
                            .to_string()
                            .to_lowercase()
                            .starts_with("hiarc")
                    }) {
                        if attr.meta.to_token_stream().to_string().to_lowercase() == "hiarc(inner)"
                        {
                            res.append(&mut collect_regex(ty.split("<").next().unwrap()));
                        } else {
                            res.push(proc_macro2::TokenStream::from_str(&ty).unwrap());
                        }
                    }

                    if res.is_empty() {
                        None
                    } else {
                        Some(res)
                    }
                })
                .flatten()
                .collect();

            if field_values.is_empty() {
                let val = forced_hi_val.unwrap_or(NonZeroU64::new(1).unwrap()).get();

                output = quote! {
                    impl #generics hiarc::HiarcTrait for #name #generics {
                        const HI_VAL: u64 = #val;
                    }
                };
            } else {
                let field_len = field_values.len();
                let mut values: Vec<String> = field_values
                    .into_iter()
                    .map(|val| {
                        "max(".to_string()
                            + &quote! {
                                <#val>::HI_VAL
                            }
                            .to_string()
                            + ", "
                    })
                    .collect();
                values.push("0)".into());
                for _ in 0..field_len - 1 {
                    values.push(")".into());
                }
                let values = proc_macro2::TokenStream::from_str(&values.join("")).unwrap();

                let forced_val = forced_hi_val
                    .unwrap_or(NonZeroU64::new(u64::MAX).unwrap())
                    .get();

                output = quote! {
                    impl #generics hiarc::HiarcTrait for #name #generics {
                        const HI_VAL: u64 = {
                            const fn max(a: u64, b: u64) -> u64 {
                                [a, b][(a < b) as usize]
                            }
                            let val = #values;
                            assert!(#forced_val > val, "hierarchical value of this struct was smaller than the value of one of its attributes, which means that the strict hierarchy is violated");
                            if #forced_val != u64::MAX {
                                #forced_val
                            }
                            else {
                                val + 1
                            }
                        };
                    }
                };
            }
        }
        _ => {
            panic!("this macro is only useful for enums and structs")
        }
    }

    //panic!("{:?}", output.to_token_stream().to_string());
    output.to_token_stream().into()
}

/// the `Hiarc` derive macro can be used to proof hierarchy of other types that implement
/// this macro at compile time, which leads to a strict and clear hierarchy.
/// In other words if `struct A` exists and `struct B` has a field of type `A` => `A` can not
/// have a field of type `B`, because that would break the hierarchy
///
/// you can declare fields with the attribute `#[hiarc]` to tell this macro that it should
/// assume, that the field implements this macro too, alternatively you can wrap this field in a `Hi` smart wrapper.
/// use `#[hiarc(inner)]` if only the inner type of the field is hiarc. Useful for Vec<T> where T
/// would implement Hiarc.
#[proc_macro_derive(Hiarc, attributes(hiarc))]
pub fn hi_arc_derive(tokens: TokenStream) -> TokenStream {
    hi_arc_trait_impl(tokens, None)
}

/// this is similar to the `Hiarc` derive macro.
/// - For structs it allows to forcefully set the value of the hierarchy
/// `#[hiarc(10)]`. It's generally not recommended to do this, but can be great to
/// give the hierarchy an offset.
/// Consider for example that you have some backend that implements `Hiarc` and
/// some other struct `A` that implements `Hiarc`. If you want to make clear that both
/// are hierachically on the same level you could annotate both with `#[hiarc(1)]`.
/// They could not include theirselves.
/// A `#[hiarc(0)]` would be on the same level as normal std types, which would mean
/// that you can prevent using numeric types etc. in your struct, which is never useful.
///
/// # Examples
///
/// This will not compile:
/// ```no_run
/// use hiarc::HiBox;
/// use hiarc_macro::hiarc;
/// #[hiarc]
/// pub struct A {
///     #[hiarc(inner)]
///     b: Option<HiBox<B>>,
/// }
///
/// #[hiarc]
/// pub struct B {
///     #[hiarc(inner)]
///     b: Option<HiBox<A>>,
/// }
///
///
/// fn main() {
///     let _ = A { b: None };
/// }
/// ```
#[proc_macro_attribute]
pub fn hiarc(attr: TokenStream, tokens: TokenStream) -> TokenStream {
    let forced_val = if attr.to_string() != "" {
        Some(
            NonZeroU64::from_str(&attr.to_string())
                .unwrap_or_else(|_| panic!("hi_arc attribute value must be a non-zero u64 value")),
        )
    } else {
        None
    };
    let trait_impl = hi_arc_trait_impl(tokens.clone(), forced_val);
    let mut base = parse_macro_input!(tokens as Item);

    match &mut base {
        Item::Struct(s) => {
            // remove attributes
            s.fields.iter_mut().for_each(|field| {
                field.attrs.retain(|attr| {
                    if attr.meta.to_token_stream().to_string().starts_with("hiarc") {
                        false
                    } else {
                        true
                    }
                });
            });
        }
        _ => {
            // nothing to do
        }
    }

    let mut tokens: TokenStream = base.to_token_stream().into();
    tokens.extend(trait_impl.into_iter());
    tokens
}

/// This rewrites the public interface of a struct, wraps the original struct as if it would be in a
/// `Rc<RefCell<_>>>`, but with certain limitations and thus makes it safer to share. Additionally it disallows
/// any kind of borrowing of the original struct without invoking `unsafe` code.
///
/// This attribute must be used on both:
/// - struct
/// - impl of the struct (even traits)
///
/// If you required to return a ref or mut ref of the object previously, you should instead
/// accept a closure as parameter and work with that.
///
/// The `hiarc_force_impl` can be used as an attribute on a function to tell the macro that
/// this function requires an implementation, even if it's a private function.
///
/// The `hiarc_trait_is_immutable_self` attribute can be used to implement a member of a trait as `&mut self`,
/// even if the trait itself should be immutable.
///
/// # Examples
///
/// ```no_run
/// #[hiarc_macro::hiarc_safer_rc_refcell]
/// pub struct MyStruct {
/// }
///
/// #[hiarc_macro::hiarc_safer_rc_refcell]
/// impl MyStruct {
///     pub fn new() -> Self {
///         Self {}
///     }
///
///     pub fn test(&self, arg: i32) {
///         println!("it worked {arg}");
///     }
/// }
///
/// fn main() {
///     let s = MyStruct::new();
///     let t = s.clone();
///     s.test(3);
///     t.test(4);
/// }
/// ```
#[proc_macro_attribute]
pub fn hiarc_safer_rc_refcell(_attr: TokenStream, tokens: TokenStream) -> TokenStream {
    let mut base = parse_macro_input!(tokens as Item);

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
            i.trait_ = None;
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
                                (found_force || item_wrapper.trait_.is_some(), found_trait_is_immutable_self)
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
                            let mut inputs = f.sig.inputs.iter();
                            if has_self_arg {
                                // skip self
                                inputs.next();
                            }
                            let args: Vec<Ident> = inputs
                                .map(|input| match input {
                                    FnArg::Receiver(_) => {
                                        panic!("multiple self are not allowed")
                                    }
                                    FnArg::Typed(t) => {
                                        format_ident!("{}", t.pat.to_token_stream().to_string())
                                    }
                                })
                                .collect();

                            if has_self_arg {
                                if self_is_mutable {
                                    f.block.stmts = parse_quote! {
                                        unsafe { self.0.borrow_mut().#func_ident(#(#args),*) }
                                    };
                                }
                                else {
                                    f.block.stmts = parse_quote! {
                                        unsafe { self.0.borrow().#func_ident(#(#args),*) }
                                    };
                                }
                            } else {
                                // check if the function is called "new"
                                if f.sig.ident == "new"  {
                                    f.block.stmts = parse_quote! {
                                        Self(std::rc::Rc::new(hiarc::HiUnsafeRefCell::new(#self_ty::new(#(#args),*))))
                                    };
                                }
                                // same for "default"
                                else if f.sig.ident == "default" {
                                    f.block.stmts = parse_quote! {
                                        Self(std::rc::Rc::new(hiarc::HiUnsafeRefCell::new(#self_ty::default(#(#args),*))))
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
        }
        Item::Struct(s) => {
            let ident = s.ident.clone();
            let attrs = &s.attrs;
            s.ident = format_ident!("{}Impl", s.ident);
            let ident_inner = &s.ident;

            let wrapper_vis = s.vis.clone();

            s.vis = Visibility::Inherited;

            struct_wrapper = quote! {
                #(#attrs)*
                #wrapper_vis struct #ident(std::rc::Rc<hiarc::HiUnsafeRefCell<#ident_inner>>);

                impl Clone for #ident {
                    fn clone(&self) -> Self {
                        Self(self.0.clone())
                    }
                }
            };
        }
        _ => panic!("only works on structs & their impl for now"),
    }

    let mut res = base.to_token_stream();

    res.extend(struct_wrapper);

    res.into()
}
