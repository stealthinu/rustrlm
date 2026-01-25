use crate::error::ReplError;
use rustpython_parser::{ast, Parse};

pub type Program = ast::Suite;

pub fn parse_program(code: &str) -> Result<Program, ReplError> {
    ast::Suite::parse(code, "<repl>").map_err(|e| ReplError::ParseError(e.to_string()))
}
