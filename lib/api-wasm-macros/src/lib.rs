use std::{ops::ControlFlow, str::FromStr};

use proc_macro::TokenStream;
use quote::ToTokens;
use syn::{
    parse_macro_input, token::Semi, Expr, Fields, FieldsNamed, FnArg, ImplItem, Item, Macro, Meta,
    Pat, ReturnType, Stmt, StmtMacro, Type,
};

/// prepare a host function to automatically call a wasm function
#[proc_macro_attribute]
pub fn wasm_func_auto_call(attr: TokenStream, tokens: TokenStream) -> TokenStream {
    let mut base_input = parse_macro_input!(tokens as Item);
    let no_res = attr.to_string() == "no_res";

    // go through all impls
    if let Item::Fn(fn_impl) = &mut base_input {
        fn_impl.block.stmts = Vec::new();
        // add args
        let mut arg_index = 0;
        for arg in &fn_impl.sig.inputs {
            if let FnArg::Typed(typed_arg) = arg {
                if let Pat::Ident(arg) = typed_arg.pat.as_ref() {
                    let mut arg_expr = arg.ident.to_string();
                    if let Type::Reference(_) = typed_arg.ty.as_ref() {
                        arg_expr += "";
                    } else {
                        arg_expr = "&".to_string() + &arg_expr;
                    }
                    let func_call = syn::parse::<Expr>(
                        TokenStream::from_str(
                            &("self.wasm_manager.add_param( ".to_string()
                                + &arg_index.to_string()
                                + ", "
                                + &arg_expr
                                + " )"),
                        )
                        .unwrap(),
                    )
                    .unwrap();
                    fn_impl
                        .block
                        .stmts
                        .push(Stmt::Expr(func_call, Some(Semi::default())));
                }
                arg_index += 1;
            }
        }
        let func_call = syn::parse::<Expr>(
            TokenStream::from_str(
                &("self.wasm_manager.run_by_ref(&self.".to_string()
                    + &(fn_impl.sig.ident.to_string() + "_name")
                    + ").unwrap()"),
            )
            .unwrap(),
        )
        .unwrap();
        fn_impl
            .block
            .stmts
            .push(Stmt::Expr(func_call, Some(Semi::default())));

        // if there is a result, parse it
        if !no_res {
            if let ReturnType::Type(_, _) = fn_impl.sig.output {
                let result = syn::parse::<Expr>(
                    TokenStream::from_str("self.wasm_manager.get_result_as::<_>()").unwrap(),
                )
                .unwrap();
                fn_impl.block.stmts.push(Stmt::Expr(result, None));
            }
        } else {
            fn_impl.sig.output = ReturnType::Default;
        }
    }

    //panic!("{:?}", base_input.to_token_stream().to_string());
    base_input.to_token_stream().into()
}

/// automatically fill the state of the Self object with typed function exports from
/// the wasm module
#[proc_macro_attribute]
pub fn wasm_func_state_prepare(_attr: TokenStream, tokens: TokenStream) -> TokenStream {
    let mut base_input = parse_macro_input!(tokens as Item);

    // go through all impls
    if let Item::Mod(mod_impl) = &mut base_input {
        // first find the trait GameStateInterface
        // this makes it ez to know all functions by name
        let mut func_names: Vec<String> = Default::default();
        mod_impl
            .content
            .as_ref()
            .unwrap()
            .1
            .iter()
            .try_for_each(|item| {
                if let Item::Impl(impl_impl) = item {
                    if let Some((_, trait_impl, _)) = &impl_impl.trait_ {
                        if trait_impl
                            .get_ident()
                            .is_some_and(|ident| ident == "GameStateInterface")
                        {
                            impl_impl.items.iter().for_each(|func| {
                                if let ImplItem::Fn(func) = func {
                                    if func
                                        .attrs
                                        .iter()
                                        .find(|attr| {
                                            if let Meta::Path(path) = &attr.meta {
                                                if path.segments.first().is_some()
                                                    && path
                                                        .segments
                                                        .first()
                                                        .unwrap()
                                                        .ident
                                                        .to_string()
                                                        .contains("wasm_func_auto_call")
                                                {
                                                    true
                                                } else {
                                                    false
                                                }
                                            } else {
                                                false
                                            }
                                        })
                                        .is_some()
                                    {
                                        func_names.push(func.sig.ident.to_string());
                                    }
                                }
                            });
                            return ControlFlow::Break(());
                        }
                    }
                }
                ControlFlow::Continue(())
            });

        // now find the struct name
        mod_impl
            .content
            .as_mut()
            .unwrap()
            .1
            .iter_mut()
            .try_for_each(|item| {
                if let Item::Struct(struct_impl) = item {
                    if let Fields::Named(named_fields) = &mut struct_impl.fields {
                        let mut build_func = "{".to_string();
                        func_names.iter().for_each(|name| {
                            build_func += &(name.clone() + "_name: wasmer::TypedFunction<(), ()>,");
                        });
                        build_func += "}";
                        let tokens = proc_macro2::TokenStream::from_str(&build_func).unwrap();
                        let joinable_struct = syn::parse::<FieldsNamed>(tokens.into()).unwrap();
                        named_fields.named.extend(joinable_struct.named);
                        return ControlFlow::Break(());
                    }
                }
                ControlFlow::Continue(())
            });

        // find the constructor impl and rewrite it
        mod_impl
            .content
            .as_mut()
            .unwrap()
            .1
            .iter_mut()
            .try_for_each(|item| {
                if let Item::Impl(impl_impl) = item {
                    if impl_impl
                        .attrs
                        .iter()
                        .find(|attr| {
                            if let Meta::Path(path) = &attr.meta {
                                if path.segments.first().is_some()
                                    && path
                                        .segments
                                        .first()
                                        .unwrap()
                                        .ident
                                        .to_string()
                                        .contains("constructor")
                                {
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        })
                        .is_some()
                    {
                        impl_impl.attrs.clear();

                        // find the new func
                        impl_impl.items.iter_mut().try_for_each(|func| {
                            if let ImplItem::Fn(func) = func {
                                if func.sig.ident.to_string() == "new" {
                                    let mut res_token = func
                                        .block
                                        .stmts
                                        .pop()
                                        .unwrap()
                                        .to_token_stream()
                                        .to_string();
                                    res_token = res_token
                                        .chars()
                                        .rev()
                                        .collect::<String>()
                                        .replacen("}", "", 1);
                                    res_token = res_token.replacen(",", "", 1);
                                    res_token = res_token.chars().rev().collect::<String>();

                                    let mut res_expr = res_token + ", ";
                                    func_names.iter().for_each(|name| {
                                        let var_name = name.clone() + "_name";
                                        res_expr += &(var_name.clone() + ", ");
                                        let mut func_init_stmt = "let ".to_string();
                                        func_init_stmt += &var_name;
                                        func_init_stmt += &(" = wasm_manager.run_func_by_name(\""
                                            .to_string()
                                            + &name
                                            + "\");");
                                        func.block.stmts.push(
                                            syn::parse::<Stmt>(
                                                TokenStream::from_str(&func_init_stmt).unwrap(),
                                            )
                                            .unwrap(),
                                        );
                                    });
                                    res_expr += "}";
                                    func.block.stmts.push(Stmt::Expr(
                                        syn::parse::<Expr>(
                                            TokenStream::from_str(&res_expr).unwrap(),
                                        )
                                        .unwrap(),
                                        None,
                                    ));
                                    return ControlFlow::Break(());
                                }
                            }
                            ControlFlow::Continue(())
                        });

                        return ControlFlow::Break(());
                    }
                }
                ControlFlow::Continue(())
            });
    }

    //panic!("{:?}", base_input.to_token_stream().to_string());
    base_input.to_token_stream().into()
}

/// prepare a wasm function to automatically call a host function
#[proc_macro_attribute]
pub fn host_func_auto_call(_attr: TokenStream, tokens: TokenStream) -> TokenStream {
    let mut base_input = parse_macro_input!(tokens as Item);

    // go through all impls
    if let Item::Fn(fn_impl) = &mut base_input {
        let func_name = fn_impl.sig.ident.to_string();
        fn_impl.block.stmts = Vec::new();
        // add args
        let mut arg_index = 0;
        let mut func_args: Vec<String> = Default::default();
        for arg in &fn_impl.sig.inputs {
            if let FnArg::Typed(typed_arg) = arg {
                if let Pat::Ident(arg) = typed_arg.pat.as_ref() {
                    let arg_expr = arg.ident.to_string();
                    let arg_type = if let Type::Reference(ref_type) = typed_arg.ty.as_ref() {
                        if ref_type.mutability.is_some() {
                            func_args.push("&mut ".to_string() + &arg_expr);
                            panic!("mutably referenced argument types are currently not supported, since WASM does not take any references from the host or vice versa. please manually implement this call");
                            // ref_type.elem.to_token_stream().to_string()
                        } else {
                            func_args.push("&".to_string() + &arg_expr);
                            ref_type.elem.to_token_stream().to_string()
                        }
                    } else {
                        func_args.push(arg_expr.clone());
                        typed_arg.ty.to_token_stream().to_string()
                    };
                    let func_param_assign = syn::parse::<Stmt>(
                        TokenStream::from_str(
                            &("let ".to_string()
                                + &arg_expr
                                + ": "
                                + &arg_type
                                + " = read_param_from_host_ex( "
                                + &arg_index.to_string()
                                + ", \""
                                + &arg_type
                                + "\", \""
                                + &func_name
                                + "\" );"),
                        )
                        .unwrap(),
                    )
                    .unwrap();
                    fn_impl.block.stmts.push(func_param_assign);
                }
                arg_index += 1;
            }
        }

        while !fn_impl.sig.inputs.is_empty() {
            if let FnArg::Receiver(_) = fn_impl.sig.inputs.last().unwrap() {
                break;
            }
            fn_impl.sig.inputs.pop();
        }

        let mut func_expr = "let res = self.state.".to_string() + &func_name + "(";
        for func_arg in func_args {
            func_expr += &func_arg;
            func_expr += ",";
        }
        func_expr += ")";

        let func_call = syn::parse::<Expr>(TokenStream::from_str(&func_expr).unwrap()).unwrap();
        fn_impl
            .block
            .stmts
            .push(Stmt::Expr(func_call, Some(Semi::default())));

        // if there is a return type, upload the return type and set the return type of this function to nothing
        if let ReturnType::Type(_, res_type) = &fn_impl.sig.output {
            if let Type::Reference(_) = res_type.as_ref() {
                panic!("referenced return types are currently not supported, since WASM does not take any references from the host or vice versa. please manually implement this call");
            }
            let result = syn::parse::<Expr>(
                TokenStream::from_str(&("upload_return_val::<_>(res)")).unwrap(),
            )
            .unwrap();
            fn_impl.block.stmts.push(Stmt::Expr(result, None));
        }
        fn_impl.sig.output = ReturnType::Default;
    }

    //panic!("{:?}", base_input.to_token_stream().to_string());
    base_input.to_token_stream().into()
}

/// this is just to signal the other macro, that this function is used when implementing the guest functions
/// for host access
#[proc_macro_attribute]
pub fn host_func_auto_call_dummy(_attr: TokenStream, tokens: TokenStream) -> TokenStream {
    tokens
}

/// implements guest functions for an impl block automatically
/// removes any trait association

#[proc_macro_attribute]
pub fn impl_guest_functions(_attr: TokenStream, tokens: TokenStream) -> TokenStream {
    let mut base_input = parse_macro_input!(tokens as Item);

    let mut original = base_input.clone();

    if let Item::Impl(fn_impl) = &mut base_input {
        fn_impl.trait_ = None;
    }

    let mut res = base_input.to_token_stream();

    // if a trait was used then
    // the original gets functions with todo!() statement
    // this is just, so the compiler says if a trait impl is missing
    if let Item::Impl(fn_impl) = &mut original {
        if let Some(_) = fn_impl.trait_ {
            fn_impl.items.iter_mut().for_each(|item| {
                if let ImplItem::Fn(func) = item {
                    // clear func attributes
                    func.attrs.clear();
                    let macro_stmt =
                        syn::parse::<Macro>(TokenStream::from_str("todo!()").unwrap()).unwrap();
                    func.block.stmts = vec![Stmt::Macro(StmtMacro {
                        attrs: vec![],
                        mac: macro_stmt,
                        semi_token: None,
                    })];
                }
            });
            res.extend(original.to_token_stream());
        }
    }

    // implement the public guest functions (the ones visible to the host)
    let mut guest_funcs: proc_macro2::TokenStream = Default::default();
    if let Item::Impl(fn_impl) = &mut base_input {
        for func in &fn_impl.items {
            if let ImplItem::Fn(func_impl) = func {
                if func_impl
                    .attrs
                    .iter()
                    .find(|attr| {
                        if let Meta::Path(path) = &attr.meta {
                            if path.segments.first().is_some()
                                && path
                                    .segments
                                    .first()
                                    .unwrap()
                                    .ident
                                    .to_string()
                                    .contains("host_func_auto_call")
                            {
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    })
                    .is_some()
                {
                    let func_name = func_impl.sig.ident.to_string();
                    guest_funcs.extend(
                        proc_macro2::TokenStream::from_str(
                            &("".to_string()
                                + "
                                #[no_mangle]
                                pub fn "
                                + &func_name
                                + "() {
                                    unsafe {
                                        API_STATE
                                            ."
                                + &func_name
                                + "()
                                    };
                                }
                                "),
                        )
                        .unwrap(),
                    );
                }
            }
        }
        if !fn_impl.items.is_empty() {
            res.extend(guest_funcs);
        }
    }

    //panic!("{:?}", res.to_token_stream().to_string());
    res.into()
}
