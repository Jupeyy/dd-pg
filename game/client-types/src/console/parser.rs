use std::{
    collections::{HashMap, VecDeque},
    ops::Range,
};

use anyhow::anyhow;
use logos::Logos;
use thiserror::Error;

use super::tokenizer::{tokenize, HumanReadableToken, Token};

/// Which argument type the command expects next
#[derive(Debug, Clone, PartialEq)]
pub enum CommandArgSyn {
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
    /// Expects a quote
    Quote,
    /// Expects the rest of the token stack as raw string,
    /// if there are multiple token left.
    /// A token stack is the global scope
    /// or a scope opened by a quote (`""`).
    /// This variant is useful if you accept only one more argument,
    /// but want to conditionally parse the argument depending on the
    /// tokens. E.g. `cl.map "quoted" not quoted` => the argument
    /// would be `"quoted" not quoted` as a whole, while
    /// [`CommandArgSyn::Quote`] would parse the quote and only
    /// return `quoted`.
    RawIfMultipleTokensInStack,
    /// Similar to [`CommandArgSyn::RawIfMultipleTokensInStack`],
    /// but it simply takes the rest of the token stack.
    Raw,
}

impl HumanReadableToken for Vec<CommandArgSyn> {
    fn human_readable(&self) -> String {
        let mut res = "[".to_string();

        let ignore_raw_if_multiple = self
            .iter()
            .find(|arg| matches!(arg, CommandArgSyn::RawIfMultipleTokensInStack))
            .and(self.iter().find(|arg| matches!(arg, CommandArgSyn::Raw)))
            .is_some();

        let it = || {
            self.iter()
                .filter(|arg| {
                    !ignore_raw_if_multiple
                        || !matches!(arg, CommandArgSyn::RawIfMultipleTokensInStack)
                })
                .enumerate()
        };
        let self_len = it().count();

        for (index, arg) in it() {
            let is_second_last = index + 2 == self_len;
            let is_last = index + 1 == self_len;
            res.push_str(match arg {
                CommandArgSyn::Command => "command/variable",
                CommandArgSyn::CommandIdent => "command name",
                CommandArgSyn::Commands => "command(s)",
                CommandArgSyn::CommandDoubleArg => "command arg arg",
                CommandArgSyn::Number => "number",
                CommandArgSyn::JsonObjectLike => todo!(),
                CommandArgSyn::JsonArrayLike => todo!(),
                CommandArgSyn::Text => "text",
                CommandArgSyn::Quote => "quoted expression",
                CommandArgSyn::RawIfMultipleTokensInStack => "raw text",
                CommandArgSyn::Raw => "raw text",
            });
            if is_second_last {
                res.push_str(" or ");
            } else if !is_last {
                res.push_str(", ");
            }
        }

        res.push(']');
        res
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
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
                            snailquote::escape(&text).to_string()
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

#[derive(Debug)]
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

#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub enum Syn {
    Command(Box<Command>),
    Commands(Commands),
    Text(String),
    Number(String),
    JsonObjectLike(String),
    JsonArrayLike(String),
}

#[derive(Debug, Clone)]
pub struct CommandArg {
    /// Defines the type of syntax to parse,
    /// Earlier syn has higher prio:
    /// If an json array and object is expected then the array
    /// is tried to be parsed first.
    pub allowed_syn: Vec<CommandArgSyn>,
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
        self.tokens
            .last()
            .map(|tokens| {
                tokens.tokens.front().map(|(token, text, range)| {
                    (
                        token,
                        text,
                        range.start + tokens.offset_in_parent..range.end + tokens.offset_in_parent,
                    )
                })
            })
            .flatten()
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
            let text = snailquote::unescape(&text).map_err(|_| Some(range.clone()))?;

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

fn parse_text(tokens: &mut TokenStack) -> anyhow::Result<(String, Range<usize>)> {
    if let Some((token, text, range)) = tokens.peek() {
        if let Token::Text = token {
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

fn parse_literal(tokens: &mut TokenStack) -> anyhow::Result<(String, Range<usize>)> {
    if let Some((token, text, range)) = tokens.peek() {
        if let Token::Quoted = token {
            let text = snailquote::unescape(&text)?;
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
        if let Token::Number = token {
            let text = text.clone();
            tokens.next();
            Ok((text, range))
        } else {
            Err(anyhow!("Expected a number, but found a {:?}", token))
        }
    } else {
        Err(anyhow!("Expected a number, but found end of string."))
    }
}

fn parse_raw(tokens: &mut TokenStack) -> anyhow::Result<(String, Range<usize>)> {
    tokens
        .take_cur_stack()
        .map(|(mut tokens, raw_str)| {
            tokens
                .pop_front()
                .map(|(_, _, range)| {
                    raw_str.get(range.start..).map(|str| {
                        (
                            ToString::to_string(str),
                            range.start..range.start + raw_str.len(),
                        )
                    })
                })
                .flatten()
        })
        .flatten()
        .ok_or_else(|| anyhow!("Expected a token stack, but there was none."))
}

fn parse_raw_non_empty(tokens: &mut TokenStack) -> anyhow::Result<(String, Range<usize>)> {
    if tokens.token_cur_stack_left_count() > 1 {
        parse_raw(tokens)
    } else {
        Err(anyhow!("Expected a token stack, but there was none."))
    }
}

/// The result of parsing a command.
#[derive(Error, Debug)]
pub enum CommandParseResult {
    #[error("Expected a variable or command")]
    InvalidCommandIdent(Range<usize>),
    #[error("{err}")]
    InvalidArg {
        partial_cmd: Command,
        err: anyhow::Error,
        range: Range<usize>,
    },
    #[error("Failed to tokenize quoted string")]
    InvalidQuoteParsing(Range<usize>),
    #[error("{err}")]
    Other {
        range: Range<usize>,
        err: anyhow::Error,
    },
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
        let text = snailquote::unescape(&text).map_err(|err| CommandParseResult::Other {
            err: err.into(),
            range: range.clone(),
        })?;
        let stack_tokens = tokenize(&text)
            .map_err(|(_, (_, range))| CommandParseResult::InvalidQuoteParsing(range))?;
        let offset_in_parent = range.start;
        tokens.next();
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
            let ident = reg.replace_all(&ident, replace).to_string();
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

        for arg in args.take(if double_arg_mode {
            cmd_args.len() * 2
        } else {
            cmd_args.len()
        }) {
            // find the required arg in tokens
            // respect the allowed syn
            enum SynOrErr {
                Syn((Syn, Range<usize>)),
                ParseRes(CommandParseResult),
            }
            let syn = arg.allowed_syn.iter().find_map(|syn| match syn {
                CommandArgSyn::Command => Some(
                    parse_command(tokens, commands, false)
                        .map(|s| {
                            let range = s.cmd_range.start
                                ..s.args
                                    .last()
                                    .map(|(_, arg_range)| arg_range.end)
                                    .unwrap_or(s.cmd_range.end);
                            SynOrErr::Syn((Syn::Command(Box::new(s)), range))
                        })
                        .unwrap_or_else(|err| SynOrErr::ParseRes(err)),
                ),
                CommandArgSyn::CommandIdent => Some(
                    parse_command_ident(tokens, commands)
                        .map(|(s, range)| SynOrErr::Syn((Syn::Text(s), range)))
                        .unwrap_or_else(|range_err| {
                            let range = range_err.unwrap_or_else(|| range.clone());
                            SynOrErr::ParseRes(CommandParseResult::InvalidCommandIdent(range))
                        }),
                ),
                CommandArgSyn::Commands => {
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
                        .map(|(first, last)| {
                            last.args
                                .last()
                                .map(|(_, arg_range)| first.cmd_range.start..arg_range.end)
                        })
                        .flatten()
                        .unwrap_or_default();
                    Some(SynOrErr::Syn((Syn::Commands(cmds), range)))
                }
                CommandArgSyn::CommandDoubleArg => Some(
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
                        .unwrap_or_else(|err| SynOrErr::ParseRes(err)),
                ),
                CommandArgSyn::Number => parse_number(tokens)
                    .ok()
                    .map(|(s, range)| SynOrErr::Syn((Syn::Number(s), range))),
                CommandArgSyn::JsonObjectLike => todo!(),
                CommandArgSyn::JsonArrayLike => todo!(),
                CommandArgSyn::Text => parse_text(tokens)
                    .ok()
                    .map(|(s, range)| SynOrErr::Syn((Syn::Text(s), range))),
                CommandArgSyn::Quote => parse_literal(tokens)
                    .ok()
                    .map(|(s, range)| SynOrErr::Syn((Syn::Text(s), range))),
                CommandArgSyn::RawIfMultipleTokensInStack => parse_raw_non_empty(tokens)
                    .ok()
                    .map(|(s, range)| SynOrErr::Syn((Syn::Text(s), range))),
                CommandArgSyn::Raw => parse_raw(tokens)
                    .ok()
                    .map(|(s, range)| SynOrErr::Syn((Syn::Text(s), range))),
            });
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
                                    arg.allowed_syn.human_readable(),
                                    token.human_readable()
                                ),
                                range,
                            )
                        })
                        .unwrap_or((
                            anyhow!(
                                "{} is required for command argument, but not found.",
                                arg.allowed_syn.human_readable()
                            ),
                            range.end.saturating_sub(1)..range.end,
                        ));
                    return Err(CommandParseResult::InvalidArg {
                        err,
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
                    .into_iter()
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

pub fn parse(raw: &str, commands: HashMap<String, Vec<CommandArg>>) -> CommandsTyped {
    let (tokens, token_err) = tokenize(raw)
        .map(|tokens| (tokens, None))
        .unwrap_or_else(|(tokens, (err_str, err_range))| (tokens, Some((err_str, err_range))));

    let mut res: CommandsTyped = Default::default();

    let tokens = generate_token_stack_entries(tokens, raw, 0);
    let mut tokens = TokenStack { tokens };
    while tokens.peek().is_some() {
        match parse_command(&mut tokens, &commands, false) {
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
            err: err_token(),
        });
        if let Some(CommandType::Partial(cmd)) = last_mut {
            match cmd {
                CommandParseResult::InvalidArg { err, range, .. } => {
                    *range = err_range;
                    *err = err_token();
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
    use crate::console::parser::{parse, CommandParseResult, CommandType, Syn};

    use super::{CommandArg, CommandArgSyn};

    #[test]
    fn console_tests() {
        let lex = parse(
            "cl.map \"name with\\\" spaces\"",
            vec![(
                "cl.map".to_string(),
                vec![CommandArg {
                    allowed_syn: vec![CommandArgSyn::Quote, CommandArgSyn::Text],
                }],
            )]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        assert!(lex[0].unwrap_ref_full().args[0].0 == Syn::Text("name with\" spaces".to_string()));

        let lex = parse(
            "bind b cl.map \"name with\\\" spaces\"",
            vec![
                (
                    "bind".to_string(),
                    vec![
                        CommandArg {
                            allowed_syn: vec![CommandArgSyn::Quote, CommandArgSyn::Text],
                        },
                        CommandArg {
                            allowed_syn: vec![CommandArgSyn::Command],
                        },
                    ],
                ),
                (
                    "cl.map".to_string(),
                    vec![CommandArg {
                        allowed_syn: vec![CommandArgSyn::Quote],
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
            "\"bind\" b cl.map \"name with\\\" spaces\"",
            vec![
                (
                    "bind".to_string(),
                    vec![
                        CommandArg {
                            allowed_syn: vec![CommandArgSyn::Quote, CommandArgSyn::Text],
                        },
                        CommandArg {
                            allowed_syn: vec![CommandArgSyn::Command],
                        },
                    ],
                ),
                (
                    "cl.map".to_string(),
                    vec![CommandArg {
                        allowed_syn: vec![CommandArgSyn::Quote, CommandArgSyn::Text],
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
            vec![
                (
                    "bind".to_string(),
                    vec![
                        CommandArg {
                            allowed_syn: vec![CommandArgSyn::Quote, CommandArgSyn::Text],
                        },
                        CommandArg {
                            allowed_syn: vec![CommandArgSyn::Commands],
                        },
                    ],
                ),
                (
                    "cl.map".to_string(),
                    vec![CommandArg {
                        allowed_syn: vec![CommandArgSyn::Quote, CommandArgSyn::Text],
                    }],
                ),
                (
                    "cl.rate".to_string(),
                    vec![CommandArg {
                        allowed_syn: vec![CommandArgSyn::Number],
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
            vec![(
                "player.name".to_string(),
                vec![CommandArg {
                    allowed_syn: vec![
                        CommandArgSyn::RawIfMultipleTokensInStack,
                        CommandArgSyn::Quote,
                        CommandArgSyn::Text,
                        CommandArgSyn::Raw,
                    ],
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
            vec![
                (
                    "push".to_string(),
                    vec![CommandArg {
                        allowed_syn: vec![CommandArgSyn::CommandIdent],
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
            vec![
                (
                    "toggle".to_string(),
                    vec![CommandArg {
                        allowed_syn: vec![CommandArgSyn::CommandDoubleArg],
                    }],
                ),
                (
                    "cl.map".to_string(),
                    vec![CommandArg {
                        allowed_syn: vec![CommandArgSyn::Quote, CommandArgSyn::Text],
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
            vec![
                (
                    "cl.refresh_rate".to_string(),
                    vec![CommandArg {
                        allowed_syn: vec![CommandArgSyn::Quote, CommandArgSyn::Number],
                    }],
                ),
                (
                    "player".to_string(),
                    vec![CommandArg {
                        allowed_syn: vec![CommandArgSyn::Quote, CommandArgSyn::Text],
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
            vec![(
                "players[$INDEX$]".to_string(),
                vec![CommandArg {
                    allowed_syn: vec![CommandArgSyn::Quote, CommandArgSyn::Text],
                }],
            )]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        assert!(lex[0].unwrap_ref_full().args[0].0 == Syn::Text("something".to_string()));
        let lex = parse(
            "players[0][name] something",
            vec![(
                "players[$INDEX$][$KEY$]".to_string(),
                vec![CommandArg {
                    allowed_syn: vec![CommandArgSyn::Quote, CommandArgSyn::Text],
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
            vec![(
                "cl.map".to_string(),
                vec![CommandArg {
                    allowed_syn: vec![CommandArgSyn::Quote, CommandArgSyn::Text],
                }],
            )]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        let cmds = lex;
        assert!(matches!(
            cmds[0].unwrap_ref_partial(),
            CommandParseResult::Other { .. }
        ));

        let lex = parse(
            "toggle cl.map \"map1 \" map2\"",
            vec![
                (
                    "toggle".to_string(),
                    vec![CommandArg {
                        allowed_syn: vec![CommandArgSyn::CommandDoubleArg],
                    }],
                ),
                (
                    "cl.map".to_string(),
                    vec![CommandArg {
                        allowed_syn: vec![CommandArgSyn::Quote, CommandArgSyn::Text],
                    }],
                ),
            ]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        let cmds = lex;
        assert!(
            cmds[0].unwrap_ref_full().ident == "toggle"
                && !cmds[0].unwrap_ref_full().args.is_empty()
                && matches!(cmds[1], CommandType::Partial(_))
        );

        let lex = parse(
            "cl.refresh_rate \"\" player \"\"; player",
            vec![
                (
                    "cl.refresh_rate".to_string(),
                    vec![CommandArg {
                        allowed_syn: vec![CommandArgSyn::Quote, CommandArgSyn::Number],
                    }],
                ),
                (
                    "player".to_string(),
                    vec![CommandArg {
                        allowed_syn: vec![CommandArgSyn::Quote, CommandArgSyn::Text],
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
            vec![
                (
                    "cl.refresh_rate".to_string(),
                    vec![CommandArg {
                        allowed_syn: vec![CommandArgSyn::Quote, CommandArgSyn::Number],
                    }],
                ),
                (
                    "player".to_string(),
                    vec![CommandArg {
                        allowed_syn: vec![CommandArgSyn::Quote, CommandArgSyn::Text],
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
            vec![(
                "player".to_string(),
                vec![CommandArg {
                    allowed_syn: vec![
                        CommandArgSyn::RawIfMultipleTokensInStack,
                        CommandArgSyn::Quote,
                        CommandArgSyn::Text,
                        CommandArgSyn::Raw,
                    ],
                }],
            )]
            .into_iter()
            .collect(),
        );

        dbg!(&lex);
        assert!(lex.len() == 2);
    }
}
