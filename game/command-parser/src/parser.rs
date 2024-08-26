use std::{
    collections::{HashMap, VecDeque},
    ops::Range,
};

use anyhow::anyhow;
use hiarc::Hiarc;
use logos::Logos;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::tokenizer::{tokenize, HumanReadableToken, Token};

/// Which argument type the command expects next
#[derive(Debug, Hiarc, Clone, PartialEq, Serialize, Deserialize)]
pub enum CommandArgType {
    /// Expects a whole command (including parsing the arguments the command requires)
    Command,
    /// Expects a the name of a command
    CommandIdent,
    /// Expects one or more commands (including parsing the arguments the command requires)
    Commands,
    /// Expects a command and the args of the command twice (special casing for toggle command)
    CommandDoubleArg,
    /// Expects a number
    Number,
    /// Expects a json object like string
    JsonObjectLike,
    /// Expects a json array like string
    JsonArrayLike,
    /// Expects a text/string
    Text,
    /// Expects a text that is part of the given array
    TextFrom(Vec<String>),
}

impl HumanReadableToken for CommandArgType {
    fn human_readable(&self) -> String {
        match self {
            CommandArgType::Command => "command/variable".to_string(),
            CommandArgType::CommandIdent => "command name".to_string(),
            CommandArgType::Commands => "command(s)".to_string(),
            CommandArgType::CommandDoubleArg => "command arg arg".to_string(),
            CommandArgType::Number => "number".to_string(),
            CommandArgType::JsonObjectLike => "json-like object".to_string(),
            CommandArgType::JsonArrayLike => "json-like array".to_string(),
            CommandArgType::Text => "text".to_string(),
            CommandArgType::TextFrom(texts) => format!("one of [{}]", texts.join(", ")),
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    pub ident: String,
    /// original unmodified text that lead to the above ident
    pub cmd_text: String,
    /// original unmodified text range
    pub cmd_range: Range<usize>,
    pub args: Vec<(Syn, Range<usize>)>,
}

impl ToString for Command {
    fn to_string(&self) -> String {
        let mut res = format!("{} ", self.cmd_text);

        for (syn, _) in &self.args {
            res.push_str(
                snailquote::escape(&match syn {
                    Syn::Command(cmd) => cmd.to_string(),
                    Syn::Commands(cmds) => cmds
                        .iter()
                        .map(|cmd| cmd.to_string())
                        .collect::<Vec<_>>()
                        .join(";"),
                    Syn::Text(text) => {
                        if let Some(Ok(Token::Text)) = Token::lexer(text).next() {
                            text.clone()
                        } else {
                            snailquote::escape(text).to_string()
                        }
                    }
                    Syn::Number(num) => num.clone(),
                    Syn::JsonObjectLike(_) => todo!(),
                    Syn::JsonArrayLike(_) => todo!(),
                })
                .to_string()
                .as_str(),
            )
        }

        res
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CommandType {
    /// Fully parsed command
    Full(Command),
    /// Partially parsed command, e.g. a syntax error or smth
    Partial(CommandParseResult),
}
impl CommandType {
    pub fn unwrap_ref_full(&self) -> &Command {
        let Self::Full(cmd) = self else {
            panic!("not a fully parsed command")
        };
        cmd
    }
    pub fn unwrap_ref_partial(&self) -> &CommandParseResult {
        let Self::Partial(cmd) = self else {
            panic!("not a partially parsed command")
        };
        cmd
    }
}
pub type Commands = Vec<Command>;
pub type CommandsTyped = Vec<CommandType>;

#[derive(Debug, Hash, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Syn {
    Command(Box<Command>),
    Commands(Commands),
    Text(String),
    Number(String),
    JsonObjectLike(String),
    JsonArrayLike(String),
}

#[derive(Debug, Hiarc, Clone, Serialize, Deserialize)]
pub struct CommandArg {
    /// Defines the type of command to parse
    pub expected_ty: CommandArgType,
}

pub struct TokenStackEntry {
    pub tokens: VecDeque<(Token, String, Range<usize>)>,
    pub raw_str: String,
    pub offset_in_parent: usize,
}

pub struct TokenStack {
    pub tokens: Vec<TokenStackEntry>,
}

impl TokenStack {
    pub fn peek(&self) -> Option<(&Token, &String, Range<usize>)> {
        self.tokens.last().and_then(|tokens| {
            tokens.tokens.front().map(|(token, text, range)| {
                (
                    token,
                    text,
                    range.start + tokens.offset_in_parent..range.end + tokens.offset_in_parent,
                )
            })
        })
    }
    pub fn next(&mut self) -> Option<(Token, String, Range<usize>)> {
        if let Some(tokens) = self.tokens.last_mut() {
            let res = tokens.tokens.pop_front();
            let offset_in_parent = tokens.offset_in_parent;
            res.map(|(token, text, range)| {
                (
                    token,
                    text,
                    range.start + offset_in_parent..range.end + offset_in_parent,
                )
            })
        } else {
            None
        }
    }
    pub fn can_pop(&mut self) {
        if let Some(tokens) = self.tokens.last_mut() {
            if tokens.tokens.is_empty() {
                self.tokens.pop();
            }
        }
    }
    pub fn token_cur_stack_left_count(&self) -> usize {
        self.tokens
            .last()
            .map(|tokens| tokens.tokens.len())
            .unwrap_or_default()
    }
    pub fn cur_stack_raw(&self) -> &str {
        self.tokens
            .last()
            .map(|tokens| tokens.raw_str.as_str())
            .unwrap_or("")
    }
    pub fn take_cur_stack(&mut self) -> Option<(VecDeque<(Token, String, Range<usize>)>, String)> {
        self.tokens
            .last_mut()
            .map(|stack| (std::mem::take(&mut stack.tokens), stack.raw_str.clone()))
    }
    pub fn cur_stack(&mut self) -> Option<&TokenStackEntry> {
        self.tokens.last()
    }
}

fn parse_command_ident(
    tokens: &mut TokenStack,
    commands: &HashMap<String, Vec<CommandArg>>,
) -> anyhow::Result<(String, Range<usize>), Option<Range<usize>>> {
    if let Some((token, text, range)) = tokens.peek() {
        let res = if let Token::Quoted = token {
            let text = snailquote::unescape(text).map_err(|_| Some(range.clone()))?;

            Ok(text)
        } else if let Token::Text = token {
            let text = text.clone();

            Ok(text)
        } else {
            Err(anyhow!(
                "Expected a text or literal, but found a {:?}",
                token
            ))
        };

        res.and_then(|text| {
            if commands.contains_key(&text) {
                tokens.next();
                Ok((text, range.clone()))
            } else {
                Err(anyhow!("Found text was not a command ident"))
            }
        })
        .map_err(|_| Some(range))
    } else {
        Err(None)
    }
}

fn parse_text_token(
    tokens: &mut TokenStack,
    allow_text: &impl Fn(&str) -> anyhow::Result<()>,
) -> anyhow::Result<(String, Range<usize>)> {
    if let Some((token, text, range)) = tokens.peek() {
        if let Token::Text = token {
            allow_text(text)?;
            let text = text.clone();
            tokens.next();

            Ok((text, range))
        } else {
            Err(anyhow!("Expected a text, but found a {:?}", token))
        }
    } else {
        Err(anyhow!("Expected a text, but found end of string."))
    }
}

fn parse_json_like_obj(tokens: &mut TokenStack) -> anyhow::Result<(String, Range<usize>)> {
    if let Some((token, text, range)) = tokens.peek() {
        if let Token::Text = token {
            serde_json::from_str::<serde_json::value::Map<String, _>>(text)
                .map_err(|err| anyhow!(err))?;
            let text = text.clone();
            tokens.next();

            Ok((text, range))
        } else if let Token::Quoted = token {
            let text = snailquote::unescape(text)?;
            serde_json::from_str::<serde_json::value::Map<String, _>>(&text)
                .map_err(|err| anyhow!(err))?;
            let text = text.clone();
            tokens.next();

            Ok((text, range))
        } else {
            Err(anyhow!("Expected a text, but found a {:?}", token))
        }
    } else {
        Err(anyhow!("Expected a text, but found end of string."))
    }
}

fn parse_json_like_arr(tokens: &mut TokenStack) -> anyhow::Result<(String, Range<usize>)> {
    if let Some((token, text, range)) = tokens.peek() {
        if let Token::Text = token {
            serde_json::from_str::<Vec<serde_json::Value>>(text).map_err(|err| anyhow!(err))?;
            let text = text.clone();
            tokens.next();

            Ok((text, range))
        } else if let Token::Quoted = token {
            let text = snailquote::unescape(text)?;
            serde_json::from_str::<Vec<serde_json::Value>>(&text).map_err(|err| anyhow!(err))?;
            let text = text.clone();
            tokens.next();

            Ok((text, range))
        } else {
            Err(anyhow!("Expected a text, but found a {:?}", token))
        }
    } else {
        Err(anyhow!("Expected a text, but found end of string."))
    }
}

fn parse_text(
    tokens: &mut TokenStack,
    is_last_arg: bool,
    allow_text: &impl Fn(&str) -> anyhow::Result<()>,
) -> anyhow::Result<(String, Range<usize>)> {
    if is_last_arg {
        parse_raw_non_empty(tokens, allow_text)
            .or_else(|_| parse_text_token(tokens, allow_text))
            .or_else(|_| parse_literal(tokens, allow_text))
            .or_else(|_| parse_raw(tokens, allow_text))
    } else {
        parse_text_token(tokens, allow_text).or_else(|_| parse_literal(tokens, allow_text))
    }
}

fn parse_literal(
    tokens: &mut TokenStack,
    allow_text: &impl Fn(&str) -> anyhow::Result<()>,
) -> anyhow::Result<(String, Range<usize>)> {
    if let Some((token, text, range)) = tokens.peek() {
        if let Token::Quoted = token {
            let text = snailquote::unescape(text)?;
            allow_text(&text)?;
            tokens.next();

            Ok((text, range))
        } else {
            Err(anyhow!("Expected a literal, but found a {:?}", token))
        }
    } else {
        Err(anyhow!("Expected a literal, but found end of string."))
    }
}

fn parse_number(tokens: &mut TokenStack) -> anyhow::Result<(String, Range<usize>)> {
    if let Some((token, text, range)) = tokens.peek() {
        if let Token::Text = token {
            anyhow::ensure!(
                text.parse::<i64>().is_ok() || text.parse::<u64>().is_ok(),
                "Expected a number, found {text}"
            );
            let text = text.clone();
            tokens.next();
            Ok((text, range))
        } else if let Token::Quoted = token {
            let text = snailquote::unescape(text)?;
            anyhow::ensure!(
                text.parse::<i64>().is_ok() || text.parse::<u64>().is_ok(),
                "Expected a number, found {text}"
            );
            tokens.next();

            Ok((text, range))
        } else {
            Err(anyhow!("Expected a number, but found a {:?}", token))
        }
    } else {
        Err(anyhow!("Expected a number, but found end of string."))
    }
}

fn parse_raw(
    tokens: &mut TokenStack,
    allow_text: &impl Fn(&str) -> anyhow::Result<()>,
) -> anyhow::Result<(String, Range<usize>)> {
    tokens
        .take_cur_stack()
        .and_then(|(mut tokens, raw_str)| {
            tokens.pop_front().and_then(|(_, _, range)| {
                raw_str.get(range.start..).map(|str| {
                    (
                        ToString::to_string(str),
                        range.start..range.start + raw_str.len(),
                    )
                })
            })
        })
        .ok_or_else(|| anyhow!("Expected a token stack, but there was none."))
        .and_then(|res| allow_text(&res.0).map(|_| res))
}

fn parse_raw_non_empty(
    tokens: &mut TokenStack,
    allow_text: &impl Fn(&str) -> anyhow::Result<()>,
) -> anyhow::Result<(String, Range<usize>)> {
    if tokens.token_cur_stack_left_count() > 1 {
        parse_raw(tokens, allow_text)
    } else {
        Err(anyhow!("Expected a token stack, but there was none."))
    }
}

/// The result of parsing a command.
#[derive(Error, Debug, Serialize, Deserialize)]
pub enum CommandParseResult {
    #[error("Expected a command")]
    InvalidCommandIdent(Range<usize>),
    #[error("{err}")]
    InvalidArg {
        partial_cmd: Command,
        err: String,
        range: Range<usize>,
    },
    #[error("Failed to tokenize quoted string")]
    InvalidQuoteParsing(Range<usize>),
    #[error("{err}")]
    Other { range: Range<usize>, err: String },
}

impl CommandParseResult {
    pub fn range(&self) -> &Range<usize> {
        match self {
            CommandParseResult::InvalidCommandIdent(range) => range,
            CommandParseResult::InvalidArg { range, .. } => range,
            CommandParseResult::InvalidQuoteParsing(range) => range,
            CommandParseResult::Other { range, .. } => range,
        }
    }
    pub fn unwrap_ref_cmd_partial(&self) -> &Command {
        let Self::InvalidArg { partial_cmd, .. } = self else {
            panic!("not a partial parsed command");
        };
        partial_cmd
    }
}

fn parse_command(
    tokens: &mut TokenStack,
    commands: &HashMap<String, Vec<CommandArg>>,
    double_arg_mode: bool,
) -> anyhow::Result<Command, CommandParseResult> {
    // if literal, then unescape the literal and push to stack
    while let Some((Token::Quoted, text, range)) = tokens.peek() {
        let text = text.clone();
        tokens.next();
        let text = snailquote::unescape(&text).map_err(|err| CommandParseResult::Other {
            err: err.to_string(),
            range: range.clone(),
        })?;
        let stack_tokens = tokenize(&text)
            .map_err(|(_, (_, range))| CommandParseResult::InvalidQuoteParsing(range))?;

        let offset_in_parent = range.start;
        let mut token_entries = generate_token_stack_entries(stack_tokens, &text, offset_in_parent);
        tokens.tokens.append(&mut token_entries);
    }
    if let Some((Token::Text, (Some(cmd_args), text, original_text, range))) =
        tokens.peek().map(|(token, text, range)| {
            // parse ident
            let ident = text;
            let reg = regex::Regex::new(r"\[([^\]]+)\]").unwrap();
            let replace = |caps: &regex::Captures| -> String {
                dbg!(caps);
                if caps
                    .get(1)
                    .map(|cap| cap.as_str().starts_with(|c: char| c.is_ascii_digit()))
                    .unwrap_or_default()
                {
                    "[$INDEX$]".into()
                } else {
                    "[$KEY$]".into()
                }
            };
            let ident = reg.replace_all(ident, replace).to_string();
            (
                token,
                (commands.get(&ident), ident, text.clone(), range.clone()),
            )
        })
    {
        let mut cmd = Command {
            ident: text.clone(),
            cmd_text: original_text,
            cmd_range: range.clone(),
            args: Default::default(),
        };
        tokens.next();

        let args = cmd_args.iter().chain(cmd_args.iter());

        let args_logic_len = if double_arg_mode {
            cmd_args.len() * 2
        } else {
            cmd_args.len()
        };
        for (arg_index, arg) in args.take(args_logic_len).enumerate() {
            let is_last = arg_index == args_logic_len - 1;
            // find the required arg in tokens
            // respect the allowed syn
            enum SynOrErr {
                Syn((Syn, Range<usize>)),
                ParseRes(CommandParseResult),
            }
            let mut syn = || match &arg.expected_ty {
                CommandArgType::Command => Some(
                    parse_command(tokens, commands, false)
                        .map(|s| {
                            let range = s.cmd_range.start
                                ..s.args
                                    .last()
                                    .map(|(_, arg_range)| arg_range.end)
                                    .unwrap_or(s.cmd_range.end);
                            SynOrErr::Syn((Syn::Command(Box::new(s)), range))
                        })
                        .unwrap_or_else(SynOrErr::ParseRes),
                ),
                CommandArgType::CommandIdent => Some(
                    parse_command_ident(tokens, commands)
                        .map(|(s, range)| SynOrErr::Syn((Syn::Text(s), range)))
                        .unwrap_or_else(|range_err| {
                            let range = range_err.unwrap_or_else(|| range.clone());
                            SynOrErr::ParseRes(CommandParseResult::InvalidCommandIdent(range))
                        }),
                ),
                CommandArgType::Commands => {
                    let mut cmds: Commands = Default::default();
                    while tokens.peek().is_some() {
                        if let Ok(cmd) = match parse_command(tokens, commands, false) {
                            Ok(cmd) => anyhow::Ok(cmd),
                            Err(err) => {
                                return Some(SynOrErr::ParseRes(err));
                            }
                        } {
                            cmds.push(cmd)
                        }
                    }
                    let range = cmds
                        .first()
                        .and_then(|first| cmds.last().map(|last| (first, last)))
                        .and_then(|(first, last)| {
                            last.args
                                .last()
                                .map(|(_, arg_range)| first.cmd_range.start..arg_range.end)
                        })
                        .unwrap_or_default();
                    Some(SynOrErr::Syn((Syn::Commands(cmds), range)))
                }
                CommandArgType::CommandDoubleArg => Some(
                    parse_command(tokens, commands, true)
                        .map(|s| {
                            let cmd_range_end = s.cmd_range.end;
                            let range = s.cmd_range.start
                                ..s.args
                                    .last()
                                    .map(|(_, arg_range)| arg_range.end)
                                    .unwrap_or(cmd_range_end);
                            SynOrErr::Syn((Syn::Command(Box::new(s)), range))
                        })
                        .unwrap_or_else(SynOrErr::ParseRes),
                ),
                CommandArgType::Number => parse_number(tokens)
                    .ok()
                    .map(|(s, range)| SynOrErr::Syn((Syn::Number(s), range))),
                CommandArgType::JsonObjectLike => parse_json_like_obj(tokens)
                    .ok()
                    .map(|(s, range)| SynOrErr::Syn((Syn::JsonObjectLike(s), range))),
                CommandArgType::JsonArrayLike => parse_json_like_arr(tokens)
                    .ok()
                    .map(|(s, range)| SynOrErr::Syn((Syn::JsonArrayLike(s), range))),
                CommandArgType::Text => parse_text(tokens, is_last, &|_| Ok(()))
                    .ok()
                    .map(|(s, range)| SynOrErr::Syn((Syn::Text(s), range))),
                CommandArgType::TextFrom(texts) => parse_text(tokens, is_last, &|str| {
                    texts
                        .iter()
                        .any(|s| s == str)
                        .then_some(())
                        .ok_or_else(|| anyhow!("text must be either of [{}]", texts.join(", ")))
                })
                .ok()
                .map(|(s, range)| SynOrErr::Syn((Syn::Text(s), range))),
            };
            let syn = syn();
            match syn {
                Some(syn) => match syn {
                    SynOrErr::Syn(syn) => {
                        cmd.args.push(syn);
                    }
                    SynOrErr::ParseRes(res) => {
                        return Err(res);
                    }
                },
                None => {
                    let token = tokens.next();
                    tokens.can_pop();
                    let (err, range) = token
                        .map(|(token, _, range)| {
                            (
                                anyhow!(
                                    "Expected {}, but found {} instead",
                                    arg.expected_ty.human_readable(),
                                    token.human_readable()
                                ),
                                range,
                            )
                        })
                        .unwrap_or((
                            anyhow!(
                                "{} is required as command argument, but not found.",
                                arg.expected_ty.human_readable()
                            ),
                            range.end.saturating_sub(1)..range.end,
                        ));
                    return Err(CommandParseResult::InvalidArg {
                        err: err.to_string(),
                        range,
                        partial_cmd: cmd,
                    });
                }
            }
        }

        tokens.can_pop();
        Ok(cmd)
    } else {
        let peek_token = tokens.next().map(|(_, _, range)| range);
        let res = Err(CommandParseResult::InvalidCommandIdent(
            if let Some(range) = peek_token {
                range
            } else {
                tokens
                    .cur_stack()
                    .map(|stack| {
                        stack.offset_in_parent..stack.offset_in_parent + stack.raw_str.len()
                    })
                    .unwrap_or_else(|| (0..0))
            },
        ));

        tokens.can_pop();
        res
    }
}

fn generate_token_stack_entries(
    tokens: Vec<(Token, String, Range<usize>)>,
    full_text: &str,
    text_range_start: usize,
) -> Vec<TokenStackEntry> {
    let mut res: Vec<TokenStackEntry> = Default::default();

    let splits = tokens.split(|(token, _, _)| matches!(token, Token::Semicolon));

    for tokens in splits.rev() {
        if !tokens.is_empty() {
            let start_range = tokens.first().unwrap().2.start;
            let end_range = tokens.last().unwrap().2.end;

            res.push(TokenStackEntry {
                tokens: tokens
                    .iter()
                    .map(|(token, text, range)| {
                        (
                            *token,
                            text.clone(),
                            range.start - start_range..range.end - start_range,
                        )
                    })
                    .collect(),
                raw_str: full_text[start_range..end_range].to_string(),
                offset_in_parent: text_range_start + start_range,
            });
        }
    }

    res
}

pub fn parse(raw: &str, commands: &HashMap<String, Vec<CommandArg>>) -> CommandsTyped {
    let (tokens, token_err) = tokenize(raw)
        .map(|tokens| (tokens, None))
        .unwrap_or_else(|(tokens, (err_str, err_range))| (tokens, Some((err_str, err_range))));

    let mut res: CommandsTyped = Default::default();

    let tokens = generate_token_stack_entries(tokens, raw, 0);
    let mut tokens = TokenStack { tokens };
    while tokens.peek().is_some() {
        match parse_command(&mut tokens, commands, false) {
            Ok(cmd) => {
                res.push(CommandType::Full(cmd));
            }
            Err(cmd_err) => {
                res.push(CommandType::Partial(cmd_err));
            }
        }
    }

    if let (Some((err_token_text, err_range)), last_mut) = (token_err, res.last_mut()) {
        let err_token = || {
            super::tokenizer::token_err(&err_token_text).unwrap_or(anyhow!(err_token_text.clone()))
        };
        let cmd_partial = CommandType::Partial(CommandParseResult::Other {
            range: err_range.clone(),
            err: err_token().to_string(),
        });
        if let Some(CommandType::Partial(cmd)) = last_mut {
            match cmd {
                CommandParseResult::InvalidArg { err, range, .. } => {
                    *range = err_range;
                    *err = err_token().to_string();
                }
                CommandParseResult::InvalidCommandIdent(_)
                | CommandParseResult::InvalidQuoteParsing(_)
                | CommandParseResult::Other { .. } => {
                    *last_mut.unwrap() = cmd_partial;
                }
            }
        } else {
            res.push(cmd_partial);
        }
    }

    res
}

#[cfg(test)]
mod test {
    use crate::parser::{parse, CommandParseResult, CommandType, Syn};

    use super::{CommandArg, CommandArgType};

    #[test]
    fn console_tests() {
        let lex = parse(
            "cl.map \"name with\\\" spaces\"",
            &vec![(
                "cl.map".to_string(),
                vec![CommandArg {
                    expected_ty: CommandArgType::Text,
                }],
            )]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        assert!(lex[0].unwrap_ref_full().args[0].0 == Syn::Text("name with\" spaces".to_string()));

        let lex = parse(
            "bind b cl.map \"name with\\\" spaces\"",
            &vec![
                (
                    "bind".to_string(),
                    vec![
                        CommandArg {
                            expected_ty: CommandArgType::Text,
                        },
                        CommandArg {
                            expected_ty: CommandArgType::Command,
                        },
                    ],
                ),
                (
                    "cl.map".to_string(),
                    vec![CommandArg {
                        expected_ty: CommandArgType::Text,
                    }],
                ),
            ]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        assert!(lex[0].unwrap_ref_full().args[0].0 == Syn::Text("b".to_string()));
        assert!(matches!(
            lex[0].unwrap_ref_full().args[1].0,
            Syn::Command(_)
        ));

        let lex = parse(
            "bind b cl.map \"name with\\\" spaces\"",
            &vec![
                (
                    "bind".to_string(),
                    vec![
                        CommandArg {
                            expected_ty: CommandArgType::Text,
                        },
                        CommandArg {
                            expected_ty: CommandArgType::Command,
                        },
                    ],
                ),
                (
                    "cl.map".to_string(),
                    vec![CommandArg {
                        expected_ty: CommandArgType::Text,
                    }],
                ),
            ]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        assert!(lex[0].unwrap_ref_full().args[0].0 == Syn::Text("b".to_string()));
        assert!(matches!(
            lex[0].unwrap_ref_full().args[1].0,
            Syn::Command(_)
        ));

        let lex = parse(
            "bind b \"cl.map \\\"name with\\\\\\\" spaces\\\"; cl.rate 50;\"",
            &vec![
                (
                    "bind".to_string(),
                    vec![
                        CommandArg {
                            expected_ty: CommandArgType::Text,
                        },
                        CommandArg {
                            expected_ty: CommandArgType::Commands,
                        },
                    ],
                ),
                (
                    "cl.map".to_string(),
                    vec![CommandArg {
                        expected_ty: CommandArgType::Text,
                    }],
                ),
                (
                    "cl.rate".to_string(),
                    vec![CommandArg {
                        expected_ty: CommandArgType::Number,
                    }],
                ),
            ]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        assert!(lex[0].unwrap_ref_full().args[0].0 == Syn::Text("b".to_string()));
        assert!(matches!(
            lex[0].unwrap_ref_full().args[1].0,
            Syn::Commands(_)
        ));

        let lex = parse(
            "player.name \"name with\\\" spaces\" abc",
            &vec![(
                "player.name".to_string(),
                vec![CommandArg {
                    expected_ty: CommandArgType::Text,
                }],
            )]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        assert!(
            lex[0].unwrap_ref_full().args[0].0
                == Syn::Text("\"name with\\\" spaces\" abc".to_string())
        );

        let lex = parse(
            "push players",
            &vec![
                (
                    "push".to_string(),
                    vec![CommandArg {
                        expected_ty: CommandArgType::CommandIdent,
                    }],
                ),
                ("players".to_string(), vec![]),
            ]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        assert!(lex[0].unwrap_ref_full().args[0].0 == Syn::Text("players".to_string()));

        let lex = parse(
            "toggle cl.map \"map1 \" \" map2\"",
            &vec![
                (
                    "toggle".to_string(),
                    vec![CommandArg {
                        expected_ty: CommandArgType::CommandDoubleArg,
                    }],
                ),
                (
                    "cl.map".to_string(),
                    vec![CommandArg {
                        expected_ty: CommandArgType::Text,
                    }],
                ),
            ]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        let cmds = lex;
        assert!(
            cmds[0].unwrap_ref_full().ident == "toggle" && {
                if let Syn::Command(cmd) = &cmds[0].unwrap_ref_full().args[0].0 {
                    cmd.args.len() == 2
                } else {
                    false
                }
            }
        );

        let lex = parse(
            "cl.refresh_rate \"\" player \"\"; player \"\"",
            &vec![
                (
                    "cl.refresh_rate".to_string(),
                    vec![CommandArg {
                        expected_ty: CommandArgType::Number,
                    }],
                ),
                (
                    "player".to_string(),
                    vec![CommandArg {
                        expected_ty: CommandArgType::Text,
                    }],
                ),
            ]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        let cmds = lex;
        assert!(cmds.len() == 3);
    }

    #[test]
    fn console_test_index() {
        let lex = parse(
            "players[0] something",
            &vec![(
                "players[$INDEX$]".to_string(),
                vec![CommandArg {
                    expected_ty: CommandArgType::Text,
                }],
            )]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        assert!(lex[0].unwrap_ref_full().args[0].0 == Syn::Text("something".to_string()));
        let lex = parse(
            "players[0][name] something",
            &vec![(
                "players[$INDEX$][$KEY$]".to_string(),
                vec![CommandArg {
                    expected_ty: CommandArgType::Text,
                }],
            )]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        assert!(lex[0].unwrap_ref_full().args[0].0 == Syn::Text("something".to_string()));
    }

    #[test]
    fn err_console_tests() {
        let lex = parse(
            "cl.map \"name with\\\" ",
            &vec![(
                "cl.map".to_string(),
                vec![CommandArg {
                    expected_ty: CommandArgType::Text,
                }],
            )]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        let cmds = lex;
        assert!(matches!(
            cmds[0].unwrap_ref_partial(),
            CommandParseResult::InvalidArg { .. }
        ));

        let lex = parse(
            "toggle cl.map \"map1 \" map2\"",
            &vec![
                (
                    "toggle".to_string(),
                    vec![CommandArg {
                        expected_ty: CommandArgType::CommandDoubleArg,
                    }],
                ),
                (
                    "cl.map".to_string(),
                    vec![CommandArg {
                        expected_ty: CommandArgType::Text,
                    }],
                ),
            ]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        let cmds = lex;
        assert!(matches!(
            cmds[0].unwrap_ref_partial(),
            CommandParseResult::InvalidArg { .. }
        ));

        let lex = parse(
            "cl.refresh_rate \"\" player \"\"; player",
            &vec![
                (
                    "cl.refresh_rate".to_string(),
                    vec![CommandArg {
                        expected_ty: CommandArgType::Number,
                    }],
                ),
                (
                    "player".to_string(),
                    vec![CommandArg {
                        expected_ty: CommandArgType::Text,
                    }],
                ),
            ]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        let cmds = lex;
        assert!(
            cmds.len() == 3
                && if let CommandType::Partial(CommandParseResult::InvalidArg { range, .. }) =
                    &cmds[2]
                {
                    range.end <= "cl.refresh_rate \"\" player \"\"; player".len()
                } else {
                    false
                }
        );

        let lex = parse(
            "cl.refresh_rate;player",
            &vec![
                (
                    "cl.refresh_rate".to_string(),
                    vec![CommandArg {
                        expected_ty: CommandArgType::Number,
                    }],
                ),
                (
                    "player".to_string(),
                    vec![CommandArg {
                        expected_ty: CommandArgType::Text,
                    }],
                ),
            ]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        assert!(lex.len() == 2);

        let lex = parse(
            "player;player",
            &vec![(
                "player".to_string(),
                vec![CommandArg {
                    expected_ty: CommandArgType::Text,
                }],
            )]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        assert!(lex.len() == 2);
    }
}
