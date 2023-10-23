#![allow(clippy::all)]

use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, Item};

#[proc_macro_derive(ConfigInterface)]
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
                impl crate::traits::ConfigInterface for #name {
                    fn conf_value() -> crate::traits::ConfigValue {
                        crate::traits::ConfigValue::StringOfList {
                            allowed_values: vec![#(#allowed_names.to_string()),*]
                        }
                    }

                    fn set_from_str(&mut self, path: String, val: String) -> anyhow::Result<()> {
                        if path.is_empty() {
                            *self = match val.to_lowercase().as_str() {
                                #(#allowed_names_lower => Self::#names,)*
                                _ => {
                                    return Err(anyhow!("Value {val} is not part of the allowed names {:?}", vec![#(#allowed_names),*]))
                                }
                            };
                            Ok(())
                        } else {
                            Err(anyhow!("Expected end of path, but found {path}"))
                        }
                    }
                }
            };
        }
        Item::Struct(s) => {
            let name = &s.ident;

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
                            if meta.starts_with("validate(") {
                                let mut inner = meta.replacen("validate(", "", 1);
                                inner.pop();
                                validation = Some(inner);
                                true
                            } else {
                                false
                            }
                        });
                    }

                    let validation = validation.unwrap_or_default();

                    let name = quote! {
                        crate::traits::ConfigValueAttr { name: #ident.to_string(), val: {
                                let mut default_config_val = <#ty>::conf_value();
                                let validation = #validation;
                                match &mut default_config_val {
                                    crate::traits::ConfigValue::Int {min, max} => {
                                        // check if there is a validation range
                                        if validation.starts_with("range(") {
                                            // parse min, max
                                            let range = validation["range(".len()..validation.len() - 1].to_string();
                                            let parts = range.split(",");
                                            let parts: Vec<String> = parts.map(|s| s.to_string()).collect();
                                            for part in parts {
                                                let inner_parts = part.split("=");
                                                let inner_parts: Vec<String> = inner_parts.map(|s| s.to_string()).collect();
                                                if inner_parts.len() == 2 {
                                                    if inner_parts[0] == "min" {
                                                        *min = inner_parts[1].parse::<i64>().unwrap();
                                                    }
                                                    else if inner_parts[0] == "max" {
                                                        *max = inner_parts[1].parse::<u64>().unwrap();
                                                    }
                                                }
                                            }
                                        }
                                    },
                                    crate::traits::ConfigValue::Float {min, max} => {
                                        // check if there is a validation range
                                        if validation.starts_with("range(") {
                                            // parse min, max
                                            let range = validation["range(".len()..validation.len() - 1].to_string();
                                            let parts = range.split(",");
                                            let parts: Vec<String> = parts.map(|s| s.to_string()).collect();
                                            for part in parts {
                                                let inner_parts = part.split("=");
                                                let inner_parts: Vec<String> = inner_parts.map(|s| s.to_string()).collect();
                                                if inner_parts.len() == 2 {
                                                    if inner_parts[0] == "min" {
                                                        *min = inner_parts[1].parse::<f64>().unwrap();
                                                    }
                                                    else if inner_parts[0] == "max" {
                                                        *max = inner_parts[1].parse::<f64>().unwrap();
                                                    }
                                                }
                                            }
                                        }
                                    },
                                    crate::traits::ConfigValue::String {min_length, max_length}  => {
                                        // check if there is a validation length
                                        if validation.starts_with("length(") {
                                            // parse min, max
                                            let range = validation["length(".len()..validation.len() - 1].to_string();
                                            let parts = range.split(",");
                                            let parts: Vec<String> = parts.map(|s| s.to_string()).collect();
                                            for part in parts {
                                                let inner_parts = part.split("=");
                                                let inner_parts: Vec<String> = inner_parts.map(|s| s.to_string()).collect();
                                                if inner_parts.len() == 2 {
                                                    if inner_parts[0] == "min" {
                                                        *min_length = inner_parts[1].parse::<usize>().unwrap();
                                                    }
                                                    else if inner_parts[0] == "max" {
                                                        *max_length = inner_parts[1].parse::<usize>().unwrap();
                                                    }
                                                }
                                            }
                                        }
                                    },
                                    crate::traits::ConfigValue::StringOfList {..} => {},
                                    crate::traits::ConfigValue::Array {..} => {},
                                    crate::traits::ConfigValue::JSONRecord {..} => {},
                                    crate::traits::ConfigValue::Struct {..} => {
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
                impl crate::traits::ConfigInterface for #name {
                    fn conf_value() -> crate::traits::ConfigValue {
                       crate::traits::ConfigValue::Struct { attributes: vec![#(#field_values),*] }
                    }

                    fn set_from_str(&mut self, path: String, val: String) -> anyhow::Result<()> {
                        if path.is_empty() {
                            Err(anyhow!("Did not expect end of path. Structs must specify their member names. Maybe you were missing a \".\""))
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

                            match path_val.to_lowercase().as_str() {
                                #(#field_names_lowercase => self.#field_names.set_from_str(remaining_path, val),)*
                                _ => {
                                    return Err(anyhow!("Value {path_val} is not part of the allowed names {:?}", vec![#(#field_names_str),*]));
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
