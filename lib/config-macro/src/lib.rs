#![allow(clippy::all)]

use std::str::FromStr;

use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::{format_ident, quote, ToTokens};
use syn::{parse_macro_input, parse_quote, Item};

#[proc_macro_derive(ConfigInterface, attributes(conf_alias))]
pub fn config(tokens: TokenStream) -> TokenStream {
    let base_input = parse_macro_input!(tokens as Item);

    let output;

    match &base_input {
        Item::Enum(e) => {
            let name = &e.ident;

            let allowed_names: Vec<String> =
                e.variants.iter().map(|a| a.ident.to_string()).collect();

            let allowed_names_lower: Vec<String> =
                allowed_names.iter().map(|a| a.to_lowercase()).collect();

            let names: Vec<Ident> = e.variants.iter().map(|a| a.ident.clone()).collect();

            output = quote! {
                impl config::traits::ConfigInterface for #name {
                    fn conf_value() -> config::traits::ConfigValue {
                        config::traits::ConfigValue::StringOfList {
                            allowed_values: vec![#(#allowed_names.to_string()),*]
                        }
                    }

                    fn try_set_from_str(&mut self, path: String, _modifier: Option<String>, val: Option<String>, conf_val: Option<&config::traits::ConfigValue>, _flags: i32) -> anyhow::Result<String, config::traits::ConfigFromStrErr> {
                        if path.is_empty() {
                            if let Some(val) = val {
                                *self = match val.to_lowercase().as_str() {
                                    #(#allowed_names_lower => Self::#names,)*
                                    _ => {
                                        return Err(config::traits::ConfigFromStrErr::PathErr(config::traits::ConfigFromStrPathErr::PathNotFound {
                                            path: val.to_string(),
                                            allowed_paths: vec![#(#allowed_names.to_string()),*],
                                        }))
                                    }
                                };
                            }
                            Ok(match self {
                                #(Self::#names => #allowed_names.to_string(),)*
                            })
                        } else {
                            Err(config::traits::ConfigFromStrErr::PathErr(config::traits::ConfigFromStrPathErr::EndOfPath(path)))
                        }
                    }
                }
            };
        }
        Item::Struct(s) => {
            let name = &s.ident;
            let mut conf_aliases_from: Vec<String> = Vec::new();
            let mut conf_aliases_to: Vec<String> = Vec::new();

            let field_values: Vec<proc_macro2::TokenStream> = s
				.fields
				.iter()
				.map(|a| {
					let ident = a.ident.as_ref().unwrap();
					let ty = &a.ty;
					let ident = ident.to_string();

					let mut validation = None;
					if !a.attrs.is_empty() {
						a.attrs.iter().find(|a| {
							let meta = a.meta.to_token_stream().to_string().replace(" ", "");
							if meta.starts_with("conf_valid(") {
								let mut inner = meta.replacen("conf_valid(", "", 1);
								inner.pop();
								validation = Some(inner);
								true
							} else {
								false
							}
						});
						for attr in a.attrs.iter() {
							let meta = attr.meta.to_token_stream().to_string().replace(" ", "");
							if meta.starts_with("conf_alias(") {
								let mut inner = meta.replacen("conf_alias(", "", 1);
								inner.pop();
								let (from, to) = inner.split_once(",").map(|(s,t)|(s.to_string(), t.to_string())).unwrap();
								conf_aliases_from.push(from);
								conf_aliases_to.push(to);
							}
						}
					}

					let validation = validation.unwrap_or_default();

					let name = quote! {
						config::traits::ConfigValueAttr { name: #ident.to_string(), val: {
								let mut default_config_val = <#ty>::conf_value();
								let validation = #validation;

                                let mut range_min_str = None;
                                let mut range_max_str = None;

								// check if there is a validation range
                                if validation.starts_with("range(") {
                                    // parse min, max
                                    let range = validation["range(".len()..validation.len() - 1].to_string();
                                    let parts = range.split(",");
                                    let parts: Vec<String> = parts.map(|s| s.to_string()).collect();
                                    for part in parts {
                                        let part = part.replace(" ", "");
                                        let inner_parts = part.split("=");
                                        let inner_parts: Vec<String> = inner_parts.map(|s| s.to_string()).collect();
                                        if inner_parts.len() == 2 {
                                            if inner_parts[0] == "min" {
                                                range_min_str = Some(inner_parts[1].to_string());
                                            }
                                            else if inner_parts[0] == "max" {
                                                range_max_str = Some(inner_parts[1].to_string());
                                            }
                                        }
                                    }
                                }

                                let mut len_min = 0;
                                let mut len_max = usize::MAX;

                                // check if there is a validation length
                                if validation.starts_with("length(") {
                                    // parse min, max
                                    let range = validation["length(".len()..validation.len() - 1].to_string();
                                    let parts = range.split(",");
                                    let parts: Vec<String> = parts.map(|s| s.to_string()).collect();
                                    for part in parts {
                                        let part = part.replace(" ", "");
                                        let inner_parts = part.split("=");
                                        let inner_parts: Vec<String> = inner_parts.map(|s| s.to_string()).collect();
                                        if inner_parts.len() == 2 {
                                            if inner_parts[0] == "min" {
                                                len_min = inner_parts[1].parse::<usize>().unwrap();
                                            }
                                            else if inner_parts[0] == "max" {
                                                len_max = inner_parts[1].parse::<usize>().unwrap();
                                            }
                                        }
                                    }
                                }

								match &mut default_config_val {
									config::traits::ConfigValue::Int {min, max} => {
                                        if let Some(range_min_str) = range_min_str {
                                            *min = range_min_str.parse::<i64>().unwrap();
                                        }
                                        if let Some(range_max_str) = range_max_str {
                                            *max = range_max_str.parse::<u64>().unwrap();
                                        }
									},
									config::traits::ConfigValue::Float {min, max} => {
                                        if let Some(range_min_str) = range_min_str {
                                            *min = range_min_str.parse::<f64>().unwrap();
                                        }
                                        if let Some(range_max_str) = range_max_str {
                                            *max = range_max_str.parse::<f64>().unwrap();
                                        }
									},
									config::traits::ConfigValue::String {min_length, max_length}  => {
                                        *min_length = len_min;
                                        *max_length = len_max;
									},
									config::traits::ConfigValue::StringOfList {..} => {},
									config::traits::ConfigValue::Array {min_length, max_length, ..} => {
                                        *min_length = len_min;
                                        *max_length = len_max;
                                    },
									config::traits::ConfigValue::JSONRecord {..} => {},
									config::traits::ConfigValue::Struct {..} => {
										// nothing to do
									},
								}
								default_config_val
							}
						}
					};

					name
				})
				.collect();

            let field_names: Vec<Ident> = s
                .fields
                .iter()
                .map(|a| a.ident.as_ref().unwrap().clone())
                .collect();
            let field_names_str: Vec<String> = s
                .fields
                .iter()
                .map(|a| a.ident.as_ref().unwrap().to_string())
                .collect();
            let field_names_lowercase: Vec<String> = s
                .fields
                .iter()
                .map(|a| a.ident.as_ref().unwrap().to_string().to_lowercase())
                .collect();

            output = quote! {
                impl config::traits::ConfigInterface for #name {
                    fn conf_value() -> config::traits::ConfigValue {
                        config::traits::ConfigValue::Struct {
                            attributes: vec![#(#field_values),*],
                            aliases: vec![#(#conf_aliases_from.to_string()),*].into_iter().zip(vec![#(#conf_aliases_to.to_string()),*].into_iter()).collect(),
                        }
                    }

                    fn try_set_from_str(&mut self, path: String, _modifier: Option<String>, val: Option<String>, conf_val: Option<&config::traits::ConfigValue>, flags: i32) -> anyhow::Result<String, config::traits::ConfigFromStrErr> {
                        if path.is_empty() {
                            if let Some(val) = val {
                                *self = serde_json::from_str(&val).map_err(|err: serde_json::Error| {
                                    config::traits::ConfigFromStrErr::PathErr(config::traits::ConfigFromStrPathErr::ParsingErr(err.to_string()))
                                })?;
                            }
                            Ok(serde_json::to_string(self).map_err(|err| {
                                config::traits::ConfigFromStrErr::FatalErr(format!("Could not serialize current value: {err}"))
                            })?)
                        } else {
                            let splits = path.split(".").next();
                            let has_dot = path.contains(".");
                            let path_val = splits.unwrap_or_else(|| &path).to_string();
                            let remaining_path = if has_dot {
                                path.replacen(&(path_val.clone() + "."), "", 1)
                            }
                            else {
                                "".to_string()
                            };

                            let from_list: Vec<&str> = vec![#(#conf_aliases_from),*];
                            let to_list: Vec<&str> = vec![#(#conf_aliases_to),*];
                            let path_val = from_list.iter().enumerate().find(|(from_index, from)| {
                                if path_val.to_lowercase() == from.to_lowercase() {
                                    true
                                }
                                else {
                                    false
                                }
                            }).map(|(from_index, from)| {
                                to_list[from_index].to_string()
                            }).unwrap_or(path_val);

                            let (modifier, path_val) = {
                                let mut path_val_res = String::new();
                                let mut modifier_res = String::new();
                                let mut brackets = 0;
                                for c in path_val.chars() {
                                    if c == '[' {
                                        brackets += 1;
                                    }
                                    else if c == ']' {
                                        brackets -= 1;
                                    }
                                    else if brackets == 0 {
                                        path_val_res.push(c);
                                    }
                                    else {
                                        modifier_res.push(c);
                                    }
                                }

                                (if !modifier_res.is_empty() { Some(modifier_res) } else { None }, path_val_res)
                            };

                            match path_val.to_lowercase().as_str() {
                                #(#field_names_lowercase => self.#field_names.try_set_from_str(remaining_path, modifier, val, Some(&#field_values.val), flags).map_err(|err| {
                                    if let config::traits::ConfigFromStrErr::FatalErr(err_str) = err {
                                        config::traits::ConfigFromStrErr::PathErr(
                                            config::traits::ConfigFromStrPathErr::FatalErr(err_str)
                                        )
                                    }
                                    else {
                                        err
                                    }
                                }),)*
                                _ => {
                                    return Err(config::traits::ConfigFromStrErr::FatalErr(
                                        config::traits::ConfigFromStrPathErr::PathNotFound {
                                            path: path_val.to_string(),
                                            allowed_paths: vec![#(#field_names_str.to_string()),*]
                                        }.to_string()
                                    ));
                                }
                            }
                        }
                    }
                }
            };
        }
        _ => {
            panic!("this macro is only useful for enums and structs")
        }
    }

    //panic!("{:?}", output.to_token_stream().to_string());
    output.to_token_stream().into()
}

/// the `config_default` macro implements default for the struct which it attributes.
/// the `config_default` macro has two attributes interesting for implementing a default & serialization for a field:
/// - `#[default = ...]` will implement the given value as default value for this struct. Additionally, if the field is missing
///     during deserialization or if the deserlization has an error, it will automatically use this default value
/// - `#[conf_valid(...)]` can validate certain attributes (where `...` is replaced by below syntax, for both the min & max are optional):
///     - length(min = x, max = y): the min/max length of a String or Vec. Note: length of a String here is the unicode length (so basically str.chars().count())
///     - range(min = x, max = y) a range of a primitive numeric type.
#[proc_macro_attribute]
pub fn config_default(_attr: TokenStream, tokens: TokenStream) -> TokenStream {
    let mut base = parse_macro_input!(tokens as Item);

    let mut extra_modules: Vec<proc_macro2::TokenStream> = Vec::new();

    let default_impl: proc_macro2::TokenStream;

    match &mut base {
        Item::Struct(s) => {
            let struct_name = &s.ident;

            let mut field_names: Vec<Ident> = Default::default();
            let mut field_defaults: Vec<proc_macro2::TokenStream> = Default::default();

            for field in &mut s.fields {
                let field_ident = field
                    .ident
                    .as_ref()
                    .unwrap_or_else(|| panic!("all fields in this struct must have a name"));
                field_names.push(field_ident.clone());

                let mut field_default = None;
                let mut validation = None;
                field.attrs = field
                    .attrs
                    .iter()
                    .filter_map(|attr| {
                        let meta = attr
                                    .meta
                                    .to_token_stream()
                                    .to_string();
                        if meta.starts_with("default")
                        {
                            let attr_str = attr
                                .meta
                                .to_token_stream()
                                .to_string();
                            let (_, val) = attr_str
                                .split_once("=")
                                .unwrap_or_else(|| panic!("correct syntax for default attribute is: #[default = val]. where val can be anything"));
                            field_default = Some(val.trim().to_string());
                            None
                        }
                        else if meta.starts_with("conf_valid(") {
                            let mut inner = meta.replacen("conf_valid(", "", 1);
                            inner.pop();
                            validation = Some(inner);
                            None
                        }
                         else {
                            Some(attr.clone())
                        }
                    })
                    .collect();

                let field_ty = &field.ty;
                let field_ty_str = field_ty.to_token_stream().to_string().replace(" ", "");
                let mod_ident =
                    format_ident!("{}_{}", struct_name.to_string().to_lowercase(), field_ident);
                let mod_ident_str = mod_ident.to_string();
                let default_as_tokens = if let Some(field_default) = &field_default {
                    proc_macro2::TokenStream::from_str(&field_default).unwrap()
                } else {
                    quote!(Default::default())
                };
                let default_parsed = if !field_default.is_some_and(|s| s.contains("\"")) {
                    quote!(#default_as_tokens)
                } else {
                    quote!(#default_as_tokens.into())
                };
                field_defaults.push(default_parsed.clone());
                let validation_extra = if let Some(validation) = validation {
                    // length
                    let mut min_length = 0;
                    let mut max_length = usize::MAX;
                    if validation.starts_with("length(") {
                        // parse min, max
                        let range = validation["length(".len()..validation.len() - 1].to_string();
                        let parts = range.split(",");
                        let parts: Vec<String> = parts.map(|s| s.to_string()).collect();
                        for part in parts {
                            let part = part.replace(" ", "");
                            let inner_parts = part.split("=");
                            let inner_parts: Vec<String> =
                                inner_parts.map(|s| s.to_string()).collect();
                            if inner_parts.len() == 2 {
                                if inner_parts[0] == "min" {
                                    min_length = inner_parts[1].parse::<usize>().unwrap();
                                } else if inner_parts[0] == "max" {
                                    max_length = inner_parts[1].parse::<usize>().unwrap();
                                }
                            }
                        }
                    }

                    // range
                    let mut range_min_str = None;
                    let mut range_max_str = None;

                    // check if there is a validation range
                    if validation.starts_with("range(") {
                        // parse min, max
                        let range = validation["range(".len()..validation.len() - 1].to_string();
                        let parts = range.split(",");
                        let parts: Vec<String> = parts.map(|s| s.to_string()).collect();
                        for part in parts {
                            let part = part.replace(" ", "");
                            let inner_parts = part.split("=");
                            let inner_parts: Vec<String> =
                                inner_parts.map(|s| s.to_string()).collect();
                            if inner_parts.len() == 2 {
                                if inner_parts[0] == "min" {
                                    range_min_str = Some(inner_parts[1].to_string());
                                } else if inner_parts[0] == "max" {
                                    range_max_str = Some(inner_parts[1].to_string());
                                }
                            }
                        }
                    }

                    if field_ty_str == "String" {
                        // check for length
                        quote! {
                            let chars_count = res.chars().count();
                            if chars_count > #max_length {
                                res = match res.char_indices().nth(#max_length) {
                                    None => &res,
                                    Some((idx, _)) => &res[..idx],
                                }.to_string();
                            }
                            else if chars_count < #min_length {
                                let def_val: #field_ty = #default_parsed;
                                while res.chars().count() < #min_length {
                                    res.push(def_val.chars().nth(res.chars().count()).unwrap());
                                }
                            }
                        }
                    } else if field_ty_str.starts_with("Vec<") {
                        // check for length
                        quote! {
                            let vec_len = res.len();
                            if vec_len > #max_length {
                                res = match res.iter().enumerate().nth(#max_length) {
                                    None => &res,
                                    Some((idx, _)) => &res[..idx],
                                }.to_vec();
                            }
                            else if vec_len < #min_length {
                                let def_val: #field_ty = #default_parsed;
                                while res.len() < #min_length {
                                    res.push(def_val.iter().nth(res.len()).unwrap().clone());
                                }
                            }
                        }
                    } else if range_min_str.is_some() || range_max_str.is_some() {
                        let range_min_str = range_min_str.unwrap_or("".to_string());
                        let range_max_str = range_max_str.unwrap_or("".to_string());
                        quote! {
                            res = res.clamp(
                                #range_min_str.parse().unwrap_or(<#field_ty>::MIN),
                                #range_max_str.parse().unwrap_or(<#field_ty>::MAX),
                            );
                        }
                    } else {
                        quote!()
                    }
                } else {
                    quote!()
                };
                extra_modules.push(quote! {
                        mod #mod_ident {
                            use serde::Deserialize;
                            use serde::Serialize;
                            use super::*;

                            pub fn def() -> #field_ty {
                                #default_parsed
                            }

                            pub fn deserialize<'de, D>(deserializer: D) -> Result<#field_ty, D::Error>
                            where
                                D: serde::Deserializer<'de>,
                            {
                                use serde::de::Error;

                                let mut res = <#field_ty>::deserialize(deserializer).unwrap_or(#default_parsed);
                                #validation_extra
                                Ok(res)
                            }

                            pub fn serialize<S>(v: &#field_ty, serializer: S) -> Result<S::Ok, S::Error>
                            where
                                S: serde::Serializer,
                            {
                                <#field_ty>::serialize(v, serializer)
                            }

                        }
                    });

                field
                    .attrs
                    .push(parse_quote!(#[serde(with = #mod_ident_str)]));
                let mod_ident_def = mod_ident_str + "::def";
                field
                    .attrs
                    .push(parse_quote!(#[serde(default = #mod_ident_def)]));
            }

            default_impl = quote! {
                impl Default for #struct_name {
                    fn default() -> Self {
                        Self {
                            #(#field_names: #field_defaults),*
                        }
                    }
                }
            };
        }
        _ => panic!("this macro can only be applied to structs"),
    }

    let mut res = base.to_token_stream();

    res.extend(extra_modules.into_iter());
    res.extend(default_impl);

    res.into()
}
