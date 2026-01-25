use std::collections::HashMap;

use crate::error::ReplError;

use super::eval::Env;
use super::value::{Module, Value};

pub struct PrintSink {
    buf: String,
    max_chars: usize,
    truncated: bool,
    buf_chars: usize,
    total_chars: usize,
    had_print_call: bool,
    print_state: String,
    print_state_chars: usize,
    max_print_state_chars: usize,
}

impl PrintSink {
    pub fn new(max_chars: usize, max_print_state_chars: usize) -> Self {
        Self {
            buf: String::new(),
            max_chars,
            truncated: false,
            buf_chars: 0,
            total_chars: 0,
            had_print_call: false,
            print_state: String::new(),
            print_state_chars: 0,
            max_print_state_chars,
        }
    }

    pub fn had_print_call(&self) -> bool {
        self.had_print_call
    }

    pub fn print_state_snapshot(&self) -> Option<&str> {
        if self.had_print_call {
            Some(self.print_state.as_str())
        } else {
            None
        }
    }

    pub fn push_raw_output(&mut self, s: &str) -> Result<(), ReplError> {
        // Append raw text into the output buffer (no implicit newline).
        for ch in s.chars() {
            self.total_chars += 1;
            if self.buf_chars < self.max_chars {
                self.buf.push(ch);
                self.buf_chars += 1;
            } else {
                self.truncated = true;
            }
        }
        Ok(())
    }

    pub fn push_echo_line(&mut self, s: &str) -> Result<(), ReplError> {
        self.push_raw_output(s)?;
        self.push_raw_output("\n")?;
        Ok(())
    }

    pub fn push_print_line(&mut self, s: &str) -> Result<(), ReplError> {
        self.had_print_call = true;
        self.push_echo_line(s)?;

        // Persist the printed output (like RestrictedPython's `_print.txt` join).
        if self.print_state_chars < self.max_print_state_chars {
            for ch in s.chars().chain(std::iter::once('\n')) {
                if self.print_state_chars >= self.max_print_state_chars {
                    break;
                }
                self.print_state.push(ch);
                self.print_state_chars += 1;
            }
        }
        Ok(())
    }

    pub fn finish(self) -> String {
        if self.total_chars == 0 {
            return "Code executed successfully (no output)".to_string();
        }
        if self.truncated {
            return format!(
                "{}\n\n[Output truncated: {} chars total, showing first {}]",
                self.buf, self.total_chars, self.max_chars
            );
        }
        self.buf.trim().to_string()
    }
}

pub fn make_initial_env(max_zlib_output_bytes: usize, context: &str, query: &str) -> Env {
    let mut globals: HashMap<String, Value> = HashMap::new();
    globals.insert("context".to_string(), Value::Str(context.to_string()));
    globals.insert("query".to_string(), Value::Str(query.to_string()));
    globals.insert(
        "re".to_string(),
        Value::Module(Module { name: "re".into() }),
    );
    globals.insert(
        "json".to_string(),
        Value::Module(Module {
            name: "json".into(),
        }),
    );
    globals.insert(
        "base64".to_string(),
        Value::Module(Module {
            name: "base64".into(),
        }),
    );
    globals.insert(
        "binascii".to_string(),
        Value::Module(Module {
            name: "binascii".into(),
        }),
    );
    globals.insert(
        "zlib".to_string(),
        Value::Module(Module {
            name: "zlib".into(),
        }),
    );

    Env::new(globals, max_zlib_output_bytes)
}
