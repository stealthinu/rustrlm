use std::collections::HashMap;

use crate::error::ReplError;
use base64::Engine;

use super::builtins::PrintSink;
use super::parse::Program;
use super::state::{try_from_value, ReplState};
use super::value::{UserFunc, Value};

pub struct Env {
    globals: HashMap<String, Value>,
    locals_stack: Vec<HashMap<String, Value>>,
    max_zlib_output_bytes: usize,
}

impl Env {
    pub fn new(globals: HashMap<String, Value>, max_zlib_output_bytes: usize) -> Self {
        Self {
            globals,
            locals_stack: Vec::new(),
            max_zlib_output_bytes,
        }
    }

    pub fn max_zlib_output_bytes(&self) -> usize {
        self.max_zlib_output_bytes
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some(frame) = self.locals_stack.last() {
            if let Some(v) = frame.get(name) {
                return Some(v.clone());
            }
        }
        self.globals.get(name).cloned()
    }

    pub fn set(&mut self, name: &str, value: Value) {
        if self.locals_stack.is_empty() {
            self.globals.insert(name.to_string(), value);
        } else {
            self.locals_stack
                .last_mut()
                .expect("locals_stack not empty")
                .insert(name.to_string(), value);
        }
    }

    pub fn push_locals(&mut self) {
        self.locals_stack.push(HashMap::new());
    }

    pub fn pop_locals(&mut self) {
        self.locals_stack.pop();
    }

    pub fn define_func(&mut self, f: UserFunc) {
        self.globals.insert(f.name.clone(), Value::UserFunc(f));
    }

    pub fn apply_state(&mut self, st: &ReplState) -> Result<(), ReplError> {
        for (k, sv) in st {
            if is_reserved_name(k) {
                continue;
            }
            let v = sv.to_value()?;
            self.globals.insert(k.clone(), v);
        }
        Ok(())
    }

    pub fn dump_state(&self) -> ReplState {
        let mut out: ReplState = ReplState::new();
        for (k, v) in &self.globals {
            if is_reserved_name(k) {
                continue;
            }
            if let Some(sv) = try_from_value(v) {
                out.insert(k.clone(), sv);
            }
        }
        out
    }
}

fn is_reserved_name(name: &str) -> bool {
    matches!(
        name,
        "context" | "query" | "re" | "json" | "base64" | "binascii" | "zlib"
    )
}

pub fn exec_program(
    _program: &Program,
    _env: &mut Env,
    _sink: &mut PrintSink,
) -> Result<(), ReplError> {
    exec_suite(_program, _env, _sink).map(|_| ())
}

pub fn maybe_echo_last_expr(code: &str, program: &Program, env: &mut Env, sink: &mut PrintSink) {
    // Emulate the upstream unofficial executor's "echo last expression" behavior:
    // it tries to `eval(last_line)` and appends it to output if non-None, but only when
    // the last line looks like a "simple expression" (naively filtered by substrings).
    let lines: Vec<&str> = code.trim().lines().collect();
    let Some(last_line) = lines.last().map(|s| s.trim()) else {
        return;
    };
    if last_line.is_empty() {
        return;
    }
    const SKIP_SUBSTRS: &[&str] = &["=", "import", "def", "class", "if", "for", "while", "with"];
    if SKIP_SUBSTRS.iter().any(|kw| last_line.contains(kw)) {
        return;
    }

    let Some(last_stmt) = program.last() else {
        return;
    };
    let rustpython_parser::ast::Stmt::Expr(s) = last_stmt else {
        return;
    };

    // Avoid double-running calls (the upstream eval frequently fails for `print(...)`
    // because `print` isn't a normal builtin in their RestrictedPython globals).
    if matches!(s.value.as_ref(), rustpython_parser::ast::Expr::Call(_)) {
        return;
    }

    match eval_expr(&s.value, env, sink) {
        Ok(Value::None) | Err(_) => {}
        Ok(v) => {
            let _ = sink.push_echo_line(&to_print_string(&v));
        }
    }
}

#[derive(Debug)]
enum Flow {
    Continue,
    Return(Value),
    Break,
    ContinueLoop,
}

fn exec_suite(
    stmts: &[rustpython_parser::ast::Stmt],
    env: &mut Env,
    sink: &mut PrintSink,
) -> Result<Flow, ReplError> {
    for st in stmts {
        match exec_stmt(st, env, sink)? {
            Flow::Continue => {}
            Flow::Return(v) => return Ok(Flow::Return(v)),
            Flow::Break => return Ok(Flow::Break),
            Flow::ContinueLoop => return Ok(Flow::ContinueLoop),
        }
    }
    Ok(Flow::Continue)
}

fn exec_stmt(
    stmt: &rustpython_parser::ast::Stmt,
    env: &mut Env,
    sink: &mut PrintSink,
) -> Result<Flow, ReplError> {
    use rustpython_parser::ast::Stmt::*;

    match stmt {
        Assign(s) => {
            let v = eval_expr(&s.value, env, sink)?;
            for t in &s.targets {
                match t {
                    rustpython_parser::ast::Expr::Name(n) => env.set(n.id.as_str(), v.clone()),
                    _ => return Err(ReplError::ForbiddenSyntax("assign target".into())),
                }
            }
            Ok(Flow::Continue)
        }
        AugAssign(s) => {
            use rustpython_parser::ast::Operator;
            let target = match s.target.as_ref() {
                rustpython_parser::ast::Expr::Name(n) => n.id.to_string(),
                _ => return Err(ReplError::ForbiddenSyntax("augassign target".into())),
            };
            let left = env
                .get(&target)
                .ok_or_else(|| ReplError::NameError(target.clone()))?;
            let right = eval_expr(&s.value, env, sink)?;
            let out = match s.op {
                Operator::Add => match (left, right) {
                    (Value::Str(a), Value::Str(b)) => Value::Str(a + &b),
                    (Value::Bytes(mut a), Value::Bytes(b)) => {
                        a.extend_from_slice(&b);
                        Value::Bytes(a)
                    }
                    (Value::Int(a), Value::Int(b)) => Value::Int(a + b),
                    (a, b) => {
                        return Err(ReplError::TypeError(format!(
                            "unsupported +=: {} and {}",
                            a.type_name(),
                            b.type_name()
                        )))
                    }
                },
                _ => {
                    return Err(ReplError::ForbiddenSyntax(
                        "unsupported augassign op".into(),
                    ))
                }
            };
            env.set(&target, out);
            Ok(Flow::Continue)
        }
        Expr(s) => {
            // Expressions do not print by default. (print() is explicit).
            let _ = eval_expr(&s.value, env, sink)?;
            Ok(Flow::Continue)
        }
        If(s) => {
            let test = eval_expr(&s.test, env, sink)?;
            if test.to_bool() {
                exec_suite(&s.body, env, sink)
            } else {
                exec_suite(&s.orelse, env, sink)
            }
        }
        Pass(_) => Ok(Flow::Continue),
        For(s) => {
            let iter_v = eval_expr(&s.iter, env, sink)?;
            let items = iter_to_vec(iter_v)?;
            for it in items {
                bind_for_target(s.target.as_ref(), it, env)?;
                match exec_suite(&s.body, env, sink)? {
                    Flow::Continue => {}
                    Flow::Return(v) => return Ok(Flow::Return(v)),
                    Flow::Break => break,
                    Flow::ContinueLoop => continue,
                }
            }
            Ok(Flow::Continue)
        }
        Try(s) => {
            match exec_suite(&s.body, env, sink) {
                Ok(Flow::Continue) => Ok(Flow::Continue),
                Ok(Flow::Return(v)) => Ok(Flow::Return(v)),
                Ok(Flow::Break) => Ok(Flow::Break),
                Ok(Flow::ContinueLoop) => Ok(Flow::ContinueLoop),
                Err(e) => {
                    if matches!(e, ReplError::SystemExit) {
                        return Err(e);
                    }
                    // Our subset treats any error as "Exception" and allows a single handler.
                    let h = s.handlers.first().ok_or(e)?;
                    match h {
                        rustpython_parser::ast::ExceptHandler::ExceptHandler(eh) => {
                            exec_suite(&eh.body, env, sink)
                        }
                    }
                }
            }
        }
        FunctionDef(s) => {
            let mut params = Vec::new();
            for a in &s.args.args {
                params.push(a.def.arg.to_string());
            }
            let f = UserFunc {
                name: s.name.to_string(),
                params,
                body: s.body.clone(),
            };
            env.define_func(f);
            Ok(Flow::Continue)
        }
        Return(s) => {
            let v = if let Some(e) = &s.value {
                eval_expr(e, env, sink)?
            } else {
                Value::None
            };
            Ok(Flow::Return(v))
        }
        Break(_) => Ok(Flow::Break),
        Continue(_) => Ok(Flow::ContinueLoop),
        Raise(s) => {
            if let Some(exc) = &s.exc {
                if let rustpython_parser::ast::Expr::Name(n) = exc.as_ref() {
                    if n.id.as_str() == "SystemExit" {
                        return Err(ReplError::SystemExit);
                    }
                }
            }
            Err(ReplError::RuntimeError("raise".into()))
        }
        Import(s) => {
            // Treat imports as bindings to pre-injected safe modules, otherwise no-op.
            // This avoids spurious failures when the model writes `import ...`.
            for a in &s.names {
                let mod_name = a.name.as_str();
                let bind_name = a
                    .asname
                    .as_ref()
                    .map(|x| x.as_str())
                    .unwrap_or_else(|| mod_name.split('.').next().unwrap_or(mod_name));
                if let Some(v) = env.get(mod_name) {
                    env.set(bind_name, v);
                }
            }
            Ok(Flow::Continue)
        }
        ImportFrom(s) => {
            // Treat `from X import y [as z]` as bindings to pre-injected module attributes.
            // We intentionally do not perform any dynamic importing.
            let level = s.level.map(|l| l.to_u32()).unwrap_or(0);
            if level != 0 {
                return Ok(Flow::Continue);
            }
            let Some(module) = &s.module else {
                return Ok(Flow::Continue);
            };
            let module_name = module.as_str();

            // Only bind from modules that are already present.
            if env.get(module_name).is_none() {
                return Ok(Flow::Continue);
            }

            for a in &s.names {
                let name = a.name.as_str();
                if name == "*" {
                    continue;
                }
                let bind_name = a.asname.as_ref().map(|x| x.as_str()).unwrap_or(name);
                if let Some(v) = importable_module_attr_value(module_name, name) {
                    env.set(bind_name, v);
                }
            }
            Ok(Flow::Continue)
        }
        _ => Err(ReplError::ForbiddenSyntax(format!("{:?}", stmt))),
    }
}

fn bind_for_target(
    target: &rustpython_parser::ast::Expr,
    it: Value,
    env: &mut Env,
) -> Result<(), ReplError> {
    match target {
        rustpython_parser::ast::Expr::Name(n) => {
            env.set(n.id.as_str(), it);
            Ok(())
        }
        rustpython_parser::ast::Expr::Tuple(t) => bind_unpack_elts(&t.elts, it, env),
        rustpython_parser::ast::Expr::List(t) => bind_unpack_elts(&t.elts, it, env),
        _ => Err(ReplError::ForbiddenSyntax("for target".into())),
    }
}

fn bind_unpack_elts(
    elts: &[rustpython_parser::ast::Expr],
    it: Value,
    env: &mut Env,
) -> Result<(), ReplError> {
    let Value::List(xs) = it else {
        return Err(ReplError::TypeError(
            "for target expects iterable of lists".into(),
        ));
    };
    if xs.len() != elts.len() {
        return Err(ReplError::ValueError("unpack mismatch".into()));
    }
    for (el, v) in elts.iter().zip(xs.into_iter()) {
        match el {
            rustpython_parser::ast::Expr::Name(n) => env.set(n.id.as_str(), v),
            _ => return Err(ReplError::ForbiddenSyntax("for target".into())),
        }
    }
    Ok(())
}

fn importable_module_attr_value(module: &str, attr: &str) -> Option<Value> {
    use super::value::Callable;
    match (module, attr) {
        ("re", "IGNORECASE") => Some(Value::Int(2)),
        ("re", "DOTALL") => Some(Value::Int(16)),
        ("re", "search") => Some(Value::Callable(Callable::Module {
            module: "re".into(),
            attr: "search".into(),
        })),
        ("re", "findall") => Some(Value::Callable(Callable::Module {
            module: "re".into(),
            attr: "findall".into(),
        })),
        ("base64", "b64decode") => Some(Value::Callable(Callable::Module {
            module: "base64".into(),
            attr: "b64decode".into(),
        })),
        ("binascii", "hexlify") => Some(Value::Callable(Callable::Module {
            module: "binascii".into(),
            attr: "hexlify".into(),
        })),
        ("zlib", "decompress") => Some(Value::Callable(Callable::Module {
            module: "zlib".into(),
            attr: "decompress".into(),
        })),
        ("zlib", "MAX_WBITS") => Some(Value::Int(15)),
        ("json", "loads") => Some(Value::Callable(Callable::Module {
            module: "json".into(),
            attr: "loads".into(),
        })),
        _ => None,
    }
}

fn iter_to_vec(v: Value) -> Result<Vec<Value>, ReplError> {
    match v {
        Value::Str(s) => Ok(s.chars().map(|c| Value::Str(c.to_string())).collect()),
        Value::Bytes(b) => Ok(b.into_iter().map(|x| Value::Int(x as i64)).collect()),
        Value::List(xs) => Ok(xs),
        Value::Dict(m) => Ok(m.keys().cloned().map(Value::Str).collect()),
        _ => Err(ReplError::TypeError(format!(
            "object is not iterable: {}",
            v.type_name()
        ))),
    }
}

fn eval_expr(
    expr: &rustpython_parser::ast::Expr,
    env: &mut Env,
    sink: &mut PrintSink,
) -> Result<Value, ReplError> {
    use rustpython_parser::ast::Expr::*;
    match expr {
        Constant(c) => constant_to_value(&c.value),
        Name(n) => env
            .get(n.id.as_str())
            .ok_or_else(|| ReplError::NameError(n.id.to_string())),
        BinOp(e) => eval_binop(e, env, sink),
        UnaryOp(e) => eval_unaryop(e, env, sink),
        IfExp(e) => {
            let test = eval_expr(&e.test, env, sink)?;
            if test.to_bool() {
                eval_expr(&e.body, env, sink)
            } else {
                eval_expr(&e.orelse, env, sink)
            }
        }
        Compare(e) => eval_compare(e, env, sink),
        BoolOp(e) => eval_boolop(e, env, sink),
        Call(e) => eval_call(e, env, sink),
        Attribute(e) => eval_attribute(e, env, sink),
        Subscript(e) => eval_subscript(e, env, sink),
        Slice(e) => eval_slice_expr(e, env, sink),
        List(e) => {
            let mut out = Vec::new();
            for el in &e.elts {
                out.push(eval_expr(el, env, sink)?);
            }
            Ok(Value::List(out))
        }
        Dict(e) => {
            // Dict literals are restricted to string keys by the allowlist.
            let mut out = std::collections::BTreeMap::new();
            for (k, v) in e.keys.iter().zip(e.values.iter()) {
                let key_expr = k
                    .as_ref()
                    .ok_or_else(|| ReplError::ForbiddenSyntax("dict unpack".into()))?;
                let key = match key_expr {
                    rustpython_parser::ast::Expr::Constant(c) => match &c.value {
                        rustpython_parser::ast::Constant::Str(s) => s.clone(),
                        _ => {
                            return Err(ReplError::ForbiddenSyntax(
                                "dict key must be str literal".into(),
                            ))
                        }
                    },
                    _ => {
                        return Err(ReplError::ForbiddenSyntax(
                            "dict key must be str literal".into(),
                        ))
                    }
                };
                let vv = eval_expr(v, env, sink)?;
                out.insert(key, vv);
            }
            Ok(Value::Dict(out))
        }
        Tuple(e) => {
            // treat tuple as list for now (only used for internal purposes, rarely observed)
            let mut out = Vec::new();
            for el in &e.elts {
                out.push(eval_expr(el, env, sink)?);
            }
            Ok(Value::List(out))
        }
        ListComp(e) => eval_listcomp(e, env, sink),
        _ => Err(ReplError::ForbiddenSyntax(format!("{:?}", expr))),
    }
}

fn eval_boolop(
    e: &rustpython_parser::ast::ExprBoolOp,
    env: &mut Env,
    sink: &mut PrintSink,
) -> Result<Value, ReplError> {
    use rustpython_parser::ast::BoolOp;
    match e.op {
        BoolOp::And => {
            for v in &e.values {
                let vv = eval_expr(v, env, sink)?;
                if !vv.to_bool() {
                    return Ok(Value::Bool(false));
                }
            }
            Ok(Value::Bool(true))
        }
        BoolOp::Or => {
            for v in &e.values {
                let vv = eval_expr(v, env, sink)?;
                if vv.to_bool() {
                    return Ok(Value::Bool(true));
                }
            }
            Ok(Value::Bool(false))
        }
    }
}

fn eval_listcomp(
    e: &rustpython_parser::ast::ExprListComp,
    env: &mut Env,
    sink: &mut PrintSink,
) -> Result<Value, ReplError> {
    // Restrict to a single generator: [elt for name in iterable if cond]
    if e.generators.len() != 1 {
        return Err(ReplError::ForbiddenSyntax("listcomp generators".into()));
    }
    let gen = &e.generators[0];
    if gen.is_async {
        return Err(ReplError::ForbiddenSyntax("async listcomp".into()));
    }
    let target_name = match &gen.target {
        rustpython_parser::ast::Expr::Name(n) => n.id.to_string(),
        _ => return Err(ReplError::ForbiddenSyntax("listcomp target".into())),
    };
    let iter_v = eval_expr(&gen.iter, env, sink)?;
    let items = iter_to_vec(iter_v)?;

    env.push_locals();
    let mut out = Vec::new();
    for it in items {
        env.set(&target_name, it);
        let mut ok = true;
        for if_expr in &gen.ifs {
            let v = eval_expr(if_expr, env, sink)?;
            if !v.to_bool() {
                ok = false;
                break;
            }
        }
        if !ok {
            continue;
        }
        out.push(eval_expr(&e.elt, env, sink)?);
    }
    env.pop_locals();

    Ok(Value::List(out))
}

fn constant_to_value(c: &rustpython_parser::ast::Constant) -> Result<Value, ReplError> {
    use rustpython_parser::ast::Constant::*;
    match c {
        None => Ok(Value::None),
        Bool(b) => Ok(Value::Bool(*b)),
        Str(s) => Ok(Value::Str(s.clone())),
        Bytes(b) => Ok(Value::Bytes(b.clone())),
        Int(i) => {
            let v = i
                .to_string()
                .parse::<i64>()
                .map_err(|_| ReplError::ValueError("int out of range".into()))?;
            Ok(Value::Int(v))
        }
        _ => Err(ReplError::ForbiddenSyntax("unsupported constant".into())),
    }
}

fn eval_binop(
    e: &rustpython_parser::ast::ExprBinOp,
    env: &mut Env,
    sink: &mut PrintSink,
) -> Result<Value, ReplError> {
    use rustpython_parser::ast::Operator;
    let l = eval_expr(&e.left, env, sink)?;
    let r = eval_expr(&e.right, env, sink)?;
    match e.op {
        Operator::Add => match (l, r) {
            (Value::Str(a), Value::Str(b)) => Ok(Value::Str(a + &b)),
            (Value::Bytes(mut a), Value::Bytes(b)) => {
                a.extend_from_slice(&b);
                Ok(Value::Bytes(a))
            }
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
            (a, b) => Err(ReplError::TypeError(format!(
                "unsupported +: {} and {}",
                a.type_name(),
                b.type_name()
            ))),
        },
        Operator::Sub => match (l, r) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
            (a, b) => Err(ReplError::TypeError(format!(
                "unsupported -: {} and {}",
                a.type_name(),
                b.type_name()
            ))),
        },
        Operator::Mod => match (l, r) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a % b)),
            (Value::Str(fmt), arg) => format_percent(&fmt, arg),
            (a, b) => Err(ReplError::TypeError(format!(
                "unsupported %: {} and {}",
                a.type_name(),
                b.type_name()
            ))),
        },
        Operator::BitOr => match (l, r) {
            (Value::Int(a), Value::Int(b)) => Ok(Value::Int(a | b)),
            (a, b) => Err(ReplError::TypeError(format!(
                "unsupported |: {} and {}",
                a.type_name(),
                b.type_name()
            ))),
        },
        _ => Err(ReplError::ForbiddenSyntax("unsupported operator".into())),
    }
}

fn eval_unaryop(
    e: &rustpython_parser::ast::ExprUnaryOp,
    env: &mut Env,
    sink: &mut PrintSink,
) -> Result<Value, ReplError> {
    use rustpython_parser::ast::UnaryOp;
    let v = eval_expr(&e.operand, env, sink)?;
    match e.op {
        UnaryOp::Not => Ok(Value::Bool(!v.to_bool())),
        UnaryOp::USub => match v {
            Value::Int(i) => Ok(Value::Int(-i)),
            _ => Err(ReplError::TypeError(format!(
                "bad operand type for unary -: '{}'",
                v.type_name()
            ))),
        },
        _ => Err(ReplError::ForbiddenSyntax("unsupported unary op".into())),
    }
}

fn eval_compare(
    e: &rustpython_parser::ast::ExprCompare,
    env: &mut Env,
    sink: &mut PrintSink,
) -> Result<Value, ReplError> {
    use rustpython_parser::ast::CmpOp;
    let mut left = eval_expr(&e.left, env, sink)?;
    for (op, right_expr) in e.ops.iter().zip(e.comparators.iter()) {
        let right = eval_expr(right_expr, env, sink)?;
        let ok = match op {
            CmpOp::Eq => left == right,
            CmpOp::NotEq => left != right,
            CmpOp::Is => is_same(&left, &right),
            CmpOp::IsNot => !is_same(&left, &right),
            CmpOp::In => is_in(&left, &right),
            CmpOp::NotIn => !is_in(&left, &right),
            CmpOp::Lt => cmp_int(&left, &right, |a, b| a < b)?,
            CmpOp::LtE => cmp_int(&left, &right, |a, b| a <= b)?,
            CmpOp::Gt => cmp_int(&left, &right, |a, b| a > b)?,
            CmpOp::GtE => cmp_int(&left, &right, |a, b| a >= b)?,
            // Keep this as a forward-compat fallback; should be unreachable for current CmpOp set.
            #[allow(unreachable_patterns)]
            _ => return Err(ReplError::ForbiddenSyntax("unsupported compare".into())),
        };
        if !ok {
            return Ok(Value::Bool(false));
        }
        left = right;
    }
    Ok(Value::Bool(true))
}

fn cmp_int<F>(a: &Value, b: &Value, f: F) -> Result<bool, ReplError>
where
    F: FnOnce(i64, i64) -> bool,
{
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Ok(f(*x, *y)),
        _ => Err(ReplError::TypeError("comparison expects int".into())),
    }
}

fn is_in(needle: &Value, haystack: &Value) -> bool {
    match (needle, haystack) {
        (Value::Str(n), Value::Str(h)) => h.contains(n),
        (Value::Str(n), Value::List(xs)) => xs.iter().any(|v| matches!(v, Value::Str(s) if s == n)),
        (Value::Int(i), Value::List(xs)) => xs.iter().any(|v| matches!(v, Value::Int(j) if j == i)),
        _ => false,
    }
}

fn is_same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::None, Value::None) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Bytes(x), Value::Bytes(y)) => x == y,
        _ => false,
    }
}

fn eval_call(
    e: &rustpython_parser::ast::ExprCall,
    env: &mut Env,
    sink: &mut PrintSink,
) -> Result<Value, ReplError> {
    // Special-case in-place list append: xs.append(v)
    if let rustpython_parser::ast::Expr::Attribute(a) = e.func.as_ref() {
        if a.attr.as_str() == "append" {
            if let rustpython_parser::ast::Expr::Name(n) = a.value.as_ref() {
                if e.args.len() != 1 || !e.keywords.is_empty() {
                    return Err(ReplError::TypeError("append(x)".into()));
                }
                let item = eval_expr(&e.args[0], env, sink)?;
                let cur = env
                    .get(n.id.as_str())
                    .ok_or_else(|| ReplError::NameError(n.id.to_string()))?;
                let mut xs = match cur {
                    Value::List(v) => v,
                    other => {
                        return Err(ReplError::TypeError(format!(
                            "append() target must be list, got {}",
                            other.type_name()
                        )))
                    }
                };
                xs.push(item);
                env.set(n.id.as_str(), Value::List(xs));
                return Ok(Value::None);
            }
        }
    }

    // Evaluate args first
    let mut args_v = Vec::new();
    for a in &e.args {
        args_v.push(eval_expr(a, env, sink)?);
    }
    let mut kwargs: HashMap<String, Value> = HashMap::new();
    for k in &e.keywords {
        let key = k
            .arg
            .as_ref()
            .ok_or_else(|| ReplError::ForbiddenSyntax("**kwargs".into()))?;
        kwargs.insert(key.to_string(), eval_expr(&k.value, env, sink)?);
    }

    match e.func.as_ref() {
        rustpython_parser::ast::Expr::Name(n) => {
            call_name(n.id.as_str(), args_v, kwargs, env, sink)
        }
        rustpython_parser::ast::Expr::Attribute(a) => call_attr(a, args_v, kwargs, env, sink),
        _ => Err(ReplError::ForbiddenSyntax("call target".into())),
    }
}

fn call_name(
    name: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
    env: &mut Env,
    sink: &mut PrintSink,
) -> Result<Value, ReplError> {
    match name {
        "print" => {
            if !kwargs.is_empty() {
                return Err(ReplError::ForbiddenSyntax("keyword args".into()));
            }
            let mut parts = Vec::new();
            for v in args {
                parts.push(to_print_string(&v));
            }
            sink.push_print_line(&parts.join(" "))?;
            Ok(Value::None)
        }
        "len" => {
            if !kwargs.is_empty() {
                return Err(ReplError::ForbiddenSyntax("keyword args".into()));
            }
            if args.len() != 1 {
                return Err(ReplError::TypeError(
                    "len() takes exactly one argument".into(),
                ));
            }
            match &args[0] {
                Value::Str(s) => Ok(Value::Int(s.chars().count() as i64)),
                Value::Bytes(b) => Ok(Value::Int(b.len() as i64)),
                Value::List(v) => Ok(Value::Int(v.len() as i64)),
                _ => Err(ReplError::TypeError("object has no len()".into())),
            }
        }
        "max" => {
            if !kwargs.is_empty() {
                return Err(ReplError::ForbiddenSyntax("keyword args".into()));
            }
            if args.len() != 2 {
                return Err(ReplError::TypeError(
                    "max() takes 2 arguments in this subset".into(),
                ));
            }
            match (&args[0], &args[1]) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::Int(std::cmp::max(*a, *b))),
                _ => Err(ReplError::TypeError("max() only supports int".into())),
            }
        }
        "rank_documents" => {
            // Prefer signature: rank_documents(query: str, documents: list, top_k: int=5, min_score: ignored)
            // For robustness, also accept swapped first args: (documents, query, top_k).
            let mut top_k: Option<i64> = None;
            for (k, v) in &kwargs {
                match k.as_str() {
                    "top_k" => match v {
                        Value::Int(i) => top_k = Some(*i),
                        other => {
                            return Err(ReplError::TypeError(format!(
                                "rank_documents() top_k must be int, got {}",
                                other.type_name()
                            )))
                        }
                    },
                    // accepted but ignored (we avoid floats in this subset)
                    "min_score" => {}
                    _ => return Err(ReplError::ForbiddenSyntax("keyword args".into())),
                }
            }

            if args.len() < 2 || args.len() > 4 {
                return Err(ReplError::TypeError(
                    "rank_documents() takes 2-4 positional arguments".into(),
                ));
            }

            let (docs, query) = match (&args[0], &args[1]) {
                (Value::Str(q), Value::List(xs)) => (xs.clone(), q.clone()),
                (Value::List(xs), Value::Str(q)) => (xs.clone(), q.clone()),
                (a, b) => {
                    return Err(ReplError::TypeError(format!(
                        "rank_documents() expects (str, list, ...), got ({}, {})",
                        a.type_name(),
                        b.type_name()
                    )))
                }
            };

            if let Some(v) = args.get(2) {
                match v {
                    Value::Int(i) => top_k = Some(*i),
                    other => {
                        return Err(ReplError::TypeError(format!(
                            "rank_documents() expects int for top_k, got {}",
                            other.type_name()
                        )))
                    }
                }
            }
            let top_k = top_k.unwrap_or(5);

            let out = rank_documents_impl(&docs, &query, top_k)?;
            Ok(Value::List(out))
        }
        "range" => {
            if !kwargs.is_empty() {
                return Err(ReplError::ForbiddenSyntax("keyword args".into()));
            }
            if args.is_empty() || args.len() > 3 {
                return Err(ReplError::TypeError(
                    "range() takes 1 to 3 arguments".into(),
                ));
            }
            let mut ints = Vec::new();
            for a in &args {
                match a {
                    Value::Int(i) => ints.push(*i),
                    other => {
                        return Err(ReplError::TypeError(format!(
                            "range() expects int, got {}",
                            other.type_name()
                        )))
                    }
                }
            }
            let (start, stop, step) = match ints.as_slice() {
                [stop] => (0i64, *stop, 1i64),
                [start, stop] => (*start, *stop, 1i64),
                [start, stop, step] => (*start, *stop, *step),
                _ => unreachable!(),
            };
            if step == 0 {
                return Err(ReplError::ValueError("range() step must not be 0".into()));
            }
            // Hard cap to keep resource bounded.
            const MAX_RANGE_LEN: usize = 5000;
            let mut out = Vec::new();
            let mut v = start;
            while (step > 0 && v < stop) || (step < 0 && v > stop) {
                out.push(Value::Int(v));
                if out.len() >= MAX_RANGE_LEN {
                    return Err(ReplError::ResourceLimitExceeded(
                        "range() exceeds max length".into(),
                    ));
                }
                v += step;
            }
            Ok(Value::List(out))
        }
        other => match env.get(other) {
            Some(Value::UserFunc(f)) => {
                if !kwargs.is_empty() {
                    return Err(ReplError::ForbiddenSyntax("keyword args".into()));
                }
                call_user_func(f, args, env, sink)
            }
            Some(Value::Callable(c)) => call_callable(c, args, kwargs, env, sink),
            _ => Err(ReplError::NameError(other.to_string())),
        },
    }
}

fn rank_documents_impl(docs: &[Value], query: &str, top_k: i64) -> Result<Vec<Value>, ReplError> {
    let top_k = top_k.clamp(0, 20) as usize;
    let terms: Vec<String> = query
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() >= 2)
        .map(|s| s.to_string())
        .collect();

    let mut scored: Vec<(i64, String, String)> = Vec::new(); // (score, doc_id, snippet)
    for d in docs {
        let Value::Dict(map) = d else {
            continue;
        };
        let Some(Value::Str(doc_id)) = map.get("id").cloned() else {
            continue;
        };
        let Some(Value::Str(text)) = map.get("text").cloned() else {
            continue;
        };
        let hay = text.to_lowercase();
        let mut s = 0i64;
        let mut best_snippet: Option<String> = None;
        for t in &terms {
            if let Some(i) = hay.find(t) {
                s += 1;
                if best_snippet.is_none() {
                    best_snippet = Some(extract_window(&text, i, t.len(), 80));
                }
            }
        }
        if s <= 0 {
            continue;
        }
        let snippet = best_snippet.unwrap_or_else(|| extract_window(&text, 0, 0, 80));
        scored.push((s, doc_id, snippet));
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let mut out = Vec::new();
    for (_s, doc_id, snippet) in scored.into_iter().take(top_k) {
        let mut m = std::collections::BTreeMap::new();
        // Provide both keys to reduce LLM confusion:
        // - documents use "id"
        // - downstream schema uses "doc_id"
        m.insert("id".to_string(), Value::Str(doc_id.clone()));
        m.insert("doc_id".to_string(), Value::Str(doc_id));
        m.insert("snippet".to_string(), Value::Str(snippet));
        out.push(Value::Dict(m));
    }
    Ok(out)
}

fn extract_window(text: &str, start_byte: usize, needle_len: usize, window: usize) -> String {
    // Best-effort: treat start_byte as a byte offset into UTF-8, but clamp safely.
    let window = window.max(1);
    let start = start_byte.saturating_sub(window);
    let end = (start_byte + needle_len + window).min(text.len());
    // Snap to UTF-8 boundaries.
    let start = snap_to_char_boundary(text, start);
    let end = snap_to_char_boundary(text, end);
    text[start..end].to_string()
}

fn snap_to_char_boundary(s: &str, mut i: usize) -> usize {
    i = i.min(s.len());
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn call_callable(
    c: super::value::Callable,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
    env: &mut Env,
    sink: &mut PrintSink,
) -> Result<Value, ReplError> {
    use super::value::Callable::*;
    match c {
        Module { module, attr } if module == "__builtins__" && attr == "range" => {
            // Delegate to the builtin implementation.
            call_name("range", args, kwargs, env, sink)
        }
        Module { module, attr } => call_module_method(&module, &attr, args, kwargs, env),
        BytesDecode { bytes } => call_bytes_method(&bytes, "decode", args, kwargs),
        StrStrip { s } => call_str_method(&s, "strip", args, kwargs),
        StrLower { s } => call_str_method(&s, "lower", args, kwargs),
        StrFind { s } => call_str_method(&s, "find", args, kwargs),
        StrReplace { s } => call_str_method(&s, "replace", args, kwargs),
        StrSplit { s } => call_str_method(&s, "split", args, kwargs),
        StrStartsWith { s } => call_str_method(&s, "startswith", args, kwargs),
        MatchGroup { m } => call_match_method(&m, "group", args, kwargs),
    }
}

fn call_user_func(
    f: UserFunc,
    args: Vec<Value>,
    env: &mut Env,
    sink: &mut PrintSink,
) -> Result<Value, ReplError> {
    if args.len() != f.params.len() {
        return Err(ReplError::TypeError(format!(
            "{}() takes {} positional arguments but {} were given",
            f.name,
            f.params.len(),
            args.len()
        )));
    }

    env.push_locals();
    for (name, val) in f.params.iter().zip(args.into_iter()) {
        env.set(name, val);
    }
    let res = match exec_suite(&f.body, env, sink)? {
        Flow::Continue => Value::None,
        Flow::Return(v) => v,
        Flow::Break | Flow::ContinueLoop => {
            return Err(ReplError::RuntimeError(
                "break/continue outside loop".into(),
            ))
        }
    };
    env.pop_locals();
    Ok(res)
}

fn call_attr(
    a: &rustpython_parser::ast::ExprAttribute,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
    env: &mut Env,
    sink: &mut PrintSink,
) -> Result<Value, ReplError> {
    let recv = eval_expr(&a.value, env, sink)?;
    let attr = a.attr.as_str();

    match recv {
        Value::Module(m) => call_module_method(&m.name, attr, args, kwargs, env),
        Value::Str(s) => call_str_method(&s, attr, args, kwargs),
        Value::Bytes(b) => call_bytes_method(&b, attr, args, kwargs),
        Value::Match(m) => call_match_method(&m, attr, args, kwargs),
        Value::Dict(m) => {
            if attr != "get" {
                return Err(ReplError::TypeError(format!(
                    "object has no attribute {}",
                    attr
                )));
            }
            if !kwargs.is_empty() {
                return Err(ReplError::ForbiddenSyntax("keyword args".into()));
            }
            if args.len() != 1 && args.len() != 2 {
                return Err(ReplError::TypeError("dict.get(key[, default])".into()));
            }
            let default = args.get(1).cloned().unwrap_or(Value::None);
            match &args[0] {
                Value::Str(k) => Ok(m.get(k).cloned().unwrap_or(default)),
                Value::Int(i) => {
                    if *i < 0 {
                        return Ok(default);
                    }
                    let idx = *i as usize;
                    let key = m.keys().nth(idx).cloned();
                    match key {
                        Some(k) => Ok(m.get(&k).cloned().unwrap_or(default)),
                        None => Ok(default),
                    }
                }
                other => Err(ReplError::TypeError(format!(
                    "dict.get key must be str|int, got {}",
                    other.type_name()
                ))),
            }
        }
        _ => Err(ReplError::TypeError(format!(
            "object has no attribute {}",
            attr
        ))),
    }
}

fn eval_attribute(
    a: &rustpython_parser::ast::ExprAttribute,
    env: &mut Env,
    sink: &mut PrintSink,
) -> Result<Value, ReplError> {
    let recv = eval_expr(&a.value, env, sink)?;
    let attr = a.attr.as_str();
    match recv {
        Value::Module(m) => match (m.name.as_str(), attr) {
            ("re", "IGNORECASE") => Ok(Value::Int(2)),
            ("re", "DOTALL") => Ok(Value::Int(16)),
            ("re", "search") => Ok(Value::Callable(super::value::Callable::Module {
                module: "re".into(),
                attr: "search".into(),
            })),
            ("re", "findall") => Ok(Value::Callable(super::value::Callable::Module {
                module: "re".into(),
                attr: "findall".into(),
            })),
            ("json", "loads") => Ok(Value::Callable(super::value::Callable::Module {
                module: "json".into(),
                attr: "loads".into(),
            })),
            ("json", "dumps") => Ok(Value::Callable(super::value::Callable::Module {
                module: "json".into(),
                attr: "dumps".into(),
            })),
            ("base64", "b64decode") => Ok(Value::Callable(super::value::Callable::Module {
                module: "base64".into(),
                attr: "b64decode".into(),
            })),
            ("binascii", "hexlify") => Ok(Value::Callable(super::value::Callable::Module {
                module: "binascii".into(),
                attr: "hexlify".into(),
            })),
            ("zlib", "decompress") => Ok(Value::Callable(super::value::Callable::Module {
                module: "zlib".into(),
                attr: "decompress".into(),
            })),
            ("zlib", "MAX_WBITS") => Ok(Value::Int(15)),
            _ => Err(ReplError::ForbiddenSyntax("attribute value".into())),
        },
        Value::Bytes(b) if attr == "decode" => {
            Ok(Value::Callable(super::value::Callable::BytesDecode {
                bytes: b,
            }))
        }
        Value::Str(s) if attr == "strip" => {
            Ok(Value::Callable(super::value::Callable::StrStrip { s }))
        }
        Value::Str(s) if attr == "lower" => {
            Ok(Value::Callable(super::value::Callable::StrLower { s }))
        }
        Value::Str(s) if attr == "find" => {
            Ok(Value::Callable(super::value::Callable::StrFind { s }))
        }
        Value::Str(s) if attr == "replace" => {
            Ok(Value::Callable(super::value::Callable::StrReplace { s }))
        }
        Value::Str(s) if attr == "split" => {
            Ok(Value::Callable(super::value::Callable::StrSplit { s }))
        }
        Value::Str(s) if attr == "startswith" => {
            Ok(Value::Callable(super::value::Callable::StrStartsWith { s }))
        }
        Value::Match(m) if attr == "group" => {
            Ok(Value::Callable(super::value::Callable::MatchGroup { m }))
        }
        _ => Err(ReplError::ForbiddenSyntax("attribute value".into())),
    }
}

fn call_module_method(
    module: &str,
    attr: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
    env: &mut Env,
) -> Result<Value, ReplError> {
    match module {
        "re" => call_re(attr, args, kwargs),
        "json" => call_json(attr, args, kwargs),
        "base64" => call_base64(attr, args, kwargs),
        "binascii" => call_binascii(attr, args, kwargs),
        "zlib" => call_zlib(attr, args, kwargs, env.max_zlib_output_bytes()),
        _ => Err(ReplError::NameError(module.to_string())),
    }
}

fn call_json(
    attr: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, ReplError> {
    if !kwargs.is_empty() {
        return Err(ReplError::ForbiddenSyntax("keyword args".into()));
    }
    match attr {
        "loads" => {
            if args.len() != 1 {
                return Err(ReplError::TypeError("json.loads(s)".into()));
            }
            let s = args[0].as_str()?;
            let v: serde_json::Value =
                serde_json::from_str(s).map_err(|e| ReplError::ValueError(e.to_string()))?;
            json_to_value(&v)
        }
        "dumps" => {
            if args.len() != 1 {
                return Err(ReplError::TypeError("json.dumps(obj)".into()));
            }
            let v = value_to_json(&args[0])?;
            Ok(Value::Str(
                serde_json::to_string(&v).map_err(|e| ReplError::ValueError(e.to_string()))?,
            ))
        }
        _ => Err(ReplError::NameError(format!("json.{}", attr))),
    }
}

fn value_to_json(v: &Value) -> Result<serde_json::Value, ReplError> {
    Ok(match v {
        Value::None => serde_json::Value::Null,
        Value::Bool(b) => serde_json::Value::Bool(*b),
        Value::Int(i) => serde_json::Value::Number((*i).into()),
        Value::Str(s) => serde_json::Value::String(s.clone()),
        Value::Bytes(b) => {
            serde_json::Value::String(base64::engine::general_purpose::STANDARD.encode(b))
        }
        Value::List(xs) => {
            serde_json::Value::Array(xs.iter().map(value_to_json).collect::<Result<_, _>>()?)
        }
        Value::Dict(m) => {
            let mut out = serde_json::Map::new();
            for (k, vv) in m {
                out.insert(k.clone(), value_to_json(vv)?);
            }
            serde_json::Value::Object(out)
        }
        _ => return Err(ReplError::TypeError("json.dumps unsupported type".into())),
    })
}

fn json_to_value(v: &serde_json::Value) -> Result<Value, ReplError> {
    match v {
        serde_json::Value::Null => Ok(Value::None),
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Int(i))
            } else {
                // Keep the subset small: represent non-int numbers as strings.
                Ok(Value::Str(n.to_string()))
            }
        }
        serde_json::Value::String(s) => Ok(Value::Str(s.clone())),
        serde_json::Value::Array(xs) => {
            let mut out = Vec::with_capacity(xs.len());
            for x in xs {
                out.push(json_to_value(x)?);
            }
            Ok(Value::List(out))
        }
        serde_json::Value::Object(m) => {
            let mut out = std::collections::BTreeMap::new();
            for (k, x) in m {
                out.insert(k.clone(), json_to_value(x)?);
            }
            Ok(Value::Dict(out))
        }
    }
}

fn call_re(
    attr: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, ReplError> {
    match attr {
        "search" => {
            if args.len() != 2 {
                return Err(ReplError::TypeError(
                    "re.search(pattern, string, ...)".into(),
                ));
            }
            let pat = args[0].as_str()?.to_string();
            let s = args[1].as_str()?.to_string();
            let flags = match kwargs.get("flags") {
                Some(Value::Int(i)) => *i,
                None => 0,
                _ => return Err(ReplError::TypeError("flags must be int".into())),
            };
            let re = build_regex(&pat, flags)?;
            if let Some(caps) = re.captures(&s) {
                let m0 = caps
                    .get(0)
                    .ok_or_else(|| ReplError::ValueError("no group(0)".into()))?;
                let span_start = byte_to_char_idx(&s, m0.start());
                let span_end = byte_to_char_idx(&s, m0.end());
                let mut groups = Vec::new();
                for i in 0..caps.len() {
                    groups.push(
                        caps.get(i)
                            .map(|m| m.as_str().to_string())
                            .unwrap_or_default(),
                    );
                }
                Ok(Value::Match(super::value::MatchObject {
                    groups,
                    span_start,
                    span_end,
                }))
            } else {
                Ok(Value::None)
            }
        }
        "findall" => {
            if args.len() != 2 {
                return Err(ReplError::TypeError(
                    "re.findall(pattern, string, ...)".into(),
                ));
            }
            let pat = args[0].as_str()?.to_string();
            let s = args[1].as_str()?.to_string();
            let flags = match kwargs.get("flags") {
                Some(Value::Int(i)) => *i,
                None => 0,
                _ => return Err(ReplError::TypeError("flags must be int".into())),
            };
            let re = build_regex(&pat, flags)?;
            let mut out = Vec::new();
            for m in re.find_iter(&s) {
                out.push(Value::Str(m.as_str().to_string()));
            }
            Ok(Value::List(out))
        }
        _ => Err(ReplError::NameError(format!("re.{}", attr))),
    }
}

fn build_regex(pat: &str, flags: i64) -> Result<regex::Regex, ReplError> {
    // Minimal normalization for Python-ish patterns seen in transcripts.
    // Rust's `regex` does not support `\\Z`, so map it to `\\z` (end of text).
    let pat = pat.replace("\\Z", "\\z");
    let mut b = regex::RegexBuilder::new(&pat);
    if (flags & 2) != 0 {
        b.case_insensitive(true);
    }
    if (flags & 16) != 0 {
        b.dot_matches_new_line(true);
    }
    b.build().map_err(|e| ReplError::ValueError(e.to_string()))
}

fn byte_to_char_idx(s: &str, byte_idx: usize) -> usize {
    // Python's indices are by Unicode codepoint; rust-regex exposes byte offsets.
    // Convert offsets to codepoint indices to match Python-ish `re` spans.
    let mut i = 0usize;
    for (b, _) in s.char_indices() {
        if b >= byte_idx {
            break;
        }
        i += 1;
    }
    i
}

fn call_match_method(
    m: &super::value::MatchObject,
    attr: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, ReplError> {
    if !kwargs.is_empty() {
        return Err(ReplError::ForbiddenSyntax("keyword args".into()));
    }
    match attr {
        "group" => {
            if args.len() != 1 {
                return Err(ReplError::TypeError("group(n)".into()));
            }
            let idx = match &args[0] {
                Value::Int(i) => *i,
                _ => return Err(ReplError::TypeError("group index must be int".into())),
            };
            if idx < 0 {
                return Err(ReplError::ValueError("negative group".into()));
            }
            let u = idx as usize;
            let s = m.groups.get(u).cloned().unwrap_or_default();
            Ok(Value::Str(s))
        }
        _ => Err(ReplError::NameError(format!("match.{}", attr))),
    }
}

fn call_str_method(
    s: &str,
    attr: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, ReplError> {
    if !kwargs.is_empty() {
        return Err(ReplError::ForbiddenSyntax("keyword args".into()));
    }
    match attr {
        "strip" => {
            if !args.is_empty() {
                return Err(ReplError::TypeError("strip() takes no args".into()));
            }
            Ok(Value::Str(s.trim().to_string()))
        }
        "lower" => {
            if !args.is_empty() {
                return Err(ReplError::TypeError("lower() takes no args".into()));
            }
            Ok(Value::Str(s.to_lowercase()))
        }
        "find" => {
            if args.len() != 1 {
                return Err(ReplError::TypeError("find(sub)".into()));
            }
            let sub = args[0].as_str()?;
            Ok(Value::Int(s.find(sub).map(|i| i as i64).unwrap_or(-1)))
        }
        "replace" => {
            if args.len() != 2 {
                return Err(ReplError::TypeError("replace(old, new)".into()));
            }
            let old = args[0].as_str()?;
            let new = args[1].as_str()?;
            Ok(Value::Str(s.replace(old, new)))
        }
        "split" => {
            if args.len() > 1 {
                return Err(ReplError::TypeError("split([sep])".into()));
            }
            let parts: Vec<String> = if args.is_empty() {
                s.split_whitespace().map(|x| x.to_string()).collect()
            } else {
                let sep = args[0].as_str()?;
                s.split(sep).map(|x| x.to_string()).collect()
            };
            Ok(Value::List(parts.into_iter().map(Value::Str).collect()))
        }
        "startswith" => {
            if args.len() != 1 {
                return Err(ReplError::TypeError("startswith(prefix)".into()));
            }
            let p = args[0].as_str()?;
            Ok(Value::Bool(s.starts_with(p)))
        }
        _ => Err(ReplError::NameError(format!("str.{}", attr))),
    }
}

fn call_bytes_method(
    b: &[u8],
    attr: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, ReplError> {
    match attr {
        "decode" => {
            let enc = if args.is_empty() {
                "utf-8".to_string()
            } else if args.len() == 1 {
                args[0].as_str()?.to_string()
            } else {
                return Err(ReplError::TypeError("decode([encoding])".into()));
            };
            let errors = match kwargs.get("errors") {
                None => "strict",
                Some(Value::Str(s)) => s.as_str(),
                Some(_) => return Err(ReplError::TypeError("errors must be str".into())),
            };
            if errors != "strict" && errors != "replace" {
                return Err(ReplError::ValueError("unsupported errors".into()));
            }
            match enc.as_str() {
                "utf-8" => {
                    if errors == "replace" {
                        Ok(Value::Str(String::from_utf8_lossy(b).to_string()))
                    } else {
                        let s = std::str::from_utf8(b)
                            .map_err(|e| ReplError::ValueError(e.to_string()))?;
                        Ok(Value::Str(s.to_string()))
                    }
                }
                "latin-1" | "latin1" => {
                    let s: String = b
                        .iter()
                        .map(|&x| char::from_u32(x as u32).unwrap())
                        .collect();
                    Ok(Value::Str(s))
                }
                "ascii" => {
                    if b.iter().any(|&x| x >= 0x80) {
                        if errors == "replace" {
                            let s: String = b
                                .iter()
                                .map(|&x| if x < 0x80 { x as char } else { '\u{FFFD}' })
                                .collect();
                            Ok(Value::Str(s))
                        } else {
                            Err(ReplError::ValueError("ascii decode error".into()))
                        }
                    } else {
                        Ok(Value::Str(String::from_utf8_lossy(b).to_string()))
                    }
                }
                _ => Err(ReplError::ValueError("unsupported encoding".into())),
            }
        }
        _ => Err(ReplError::NameError(format!("bytes.{}", attr))),
    }
}

fn call_base64(
    attr: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, ReplError> {
    if !kwargs.is_empty() {
        return Err(ReplError::ForbiddenSyntax("keyword args".into()));
    }
    match attr {
        "b64decode" => {
            if args.len() != 1 {
                return Err(ReplError::TypeError("b64decode(data)".into()));
            }
            let mut s = match &args[0] {
                Value::Str(v) => v.clone(),
                Value::Bytes(b) => String::from_utf8_lossy(b).to_string(),
                _ => return Err(ReplError::TypeError("b64decode expects str|bytes".into())),
            };
            s.retain(|c| !c.is_whitespace());
            let pad = (4 - (s.len() % 4)) % 4;
            for _ in 0..pad {
                s.push('=');
            }
            use base64::Engine;
            let std = base64::engine::general_purpose::STANDARD.decode(s.as_bytes());
            let out = match std {
                Ok(b) => b,
                Err(_) => base64::engine::general_purpose::URL_SAFE
                    .decode(s.as_bytes())
                    .map_err(|e| ReplError::ValueError(e.to_string()))?,
            };
            Ok(Value::Bytes(out))
        }
        _ => Err(ReplError::NameError(format!("base64.{}", attr))),
    }
}

fn call_binascii(
    attr: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
) -> Result<Value, ReplError> {
    if !kwargs.is_empty() {
        return Err(ReplError::ForbiddenSyntax("keyword args".into()));
    }
    match attr {
        "hexlify" => {
            if args.len() != 1 {
                return Err(ReplError::TypeError("hexlify(data)".into()));
            }
            let b = args[0].as_bytes()?;
            let mut out = Vec::with_capacity(b.len() * 2);
            for &x in b {
                out.push(nibble_to_hex(x >> 4));
                out.push(nibble_to_hex(x & 0x0f));
            }
            Ok(Value::Bytes(out))
        }
        _ => Err(ReplError::NameError(format!("binascii.{}", attr))),
    }
}

fn nibble_to_hex(n: u8) -> u8 {
    match n {
        0..=9 => b'0' + n,
        10..=15 => b'a' + (n - 10),
        _ => b'?',
    }
}

fn call_zlib(
    attr: &str,
    args: Vec<Value>,
    kwargs: HashMap<String, Value>,
    max_output: usize,
) -> Result<Value, ReplError> {
    if !kwargs.is_empty() {
        return Err(ReplError::ForbiddenSyntax("keyword args".into()));
    }
    match attr {
        "decompress" => {
            if args.is_empty() || args.len() > 2 {
                return Err(ReplError::TypeError(
                    "zlib.decompress(data[, wbits])".into(),
                ));
            }
            let data = args[0].as_bytes()?.to_vec();
            let wbits = if args.len() == 2 {
                match &args[1] {
                    Value::Int(i) => *i as i32,
                    _ => return Err(ReplError::TypeError("wbits must be int".into())),
                }
            } else {
                15
            };

            let out = zlib_decompress_capped(&data, wbits, max_output)?;
            Ok(Value::Bytes(out))
        }
        _ => Err(ReplError::NameError(format!("zlib.{}", attr))),
    }
}

fn zlib_decompress_capped(
    data: &[u8],
    wbits: i32,
    max_output: usize,
) -> Result<Vec<u8>, ReplError> {
    use std::io::Read;

    let mut out = Vec::new();
    let mut buf = [0u8; 8192];

    enum Kind {
        Zlib,
        Gzip,
        RawDeflate,
        Auto,
    }

    let kind = match wbits {
        15 => Kind::Zlib,
        31 => Kind::Gzip,
        -15 => Kind::RawDeflate,
        47 => Kind::Auto,
        _ => Kind::Zlib,
    };

    let mut reader: Box<dyn Read> = match kind {
        Kind::Zlib => Box::new(flate2::read::ZlibDecoder::new(data)),
        Kind::Gzip => Box::new(flate2::read::GzDecoder::new(data)),
        Kind::RawDeflate => Box::new(flate2::read::DeflateDecoder::new(data)),
        Kind::Auto => {
            // Try zlib first; if it errors immediately, fallback to gzip.
            let mut zr = flate2::read::ZlibDecoder::new(data);
            match zr.read(&mut buf) {
                Ok(n) => {
                    out.extend_from_slice(&buf[..n]);
                    Box::new(zr)
                }
                Err(_) => Box::new(flate2::read::GzDecoder::new(data)),
            }
        }
    };

    if out.len() > max_output {
        return Err(ReplError::ResourceLimitExceeded(
            "zlib output exceeds limit".into(),
        ));
    }

    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| ReplError::ValueError(e.to_string()))?;
        if n == 0 {
            break;
        }
        if out.len() + n > max_output {
            return Err(ReplError::ResourceLimitExceeded(
                "zlib output exceeds limit".into(),
            ));
        }
        out.extend_from_slice(&buf[..n]);
    }
    Ok(out)
}

fn py_repr_str(s: &str) -> String {
    // Close enough to Python's `repr(str)` for the transcripts we replay:
    // single quotes, backslash + quote escaping, and common whitespace escapes.
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out.push('\'');
    out
}

fn py_repr_bytes(b: &[u8]) -> String {
    // Python's `str(bytes)` is the same as `repr(bytes)`: b'...'
    let mut out = String::new();
    out.push('b');
    out.push('\'');
    for &x in b {
        match x {
            b'\\' => out.push_str("\\\\"),
            b'\'' => out.push_str("\\'"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7e => out.push(x as char),
            _ => out.push_str(&format!("\\x{:02x}", x)),
        }
    }
    out.push('\'');
    out
}

fn py_repr_value(v: &Value) -> String {
    match v {
        Value::None => "None".into(),
        Value::Bool(b) => {
            if *b {
                "True".into()
            } else {
                "False".into()
            }
        }
        Value::Int(i) => i.to_string(),
        Value::Str(s) => py_repr_str(s),
        Value::Bytes(b) => py_repr_bytes(b),
        Value::List(xs) => {
            let mut out = String::from("[");
            for (i, x) in xs.iter().enumerate() {
                if i != 0 {
                    out.push_str(", ");
                }
                out.push_str(&py_repr_value(x));
            }
            out.push(']');
            out
        }
        Value::Dict(m) => {
            let mut out = String::from("{");
            for (i, (k, v)) in m.iter().enumerate() {
                if i != 0 {
                    out.push_str(", ");
                }
                out.push_str(&py_repr_str(k));
                out.push_str(": ");
                out.push_str(&py_repr_value(v));
            }
            out.push('}');
            out
        }
        Value::Match(m) => {
            let matched = m.groups.first().map(|s| s.as_str()).unwrap_or("");
            format!(
                "<re.Match object; span=({}, {}), match={}>",
                m.span_start,
                m.span_end,
                py_repr_str(matched)
            )
        }
        Value::UserFunc(f) => format!("<function {}>", f.name),
        Value::Callable(_) => "<callable>".into(),
        Value::Module(m) => format!("<module {}>", m.name),
    }
}

fn to_print_string(v: &Value) -> String {
    // Python's `print(x)` uses `str(x)`. For str values that means the raw contents
    // (no quotes), while containers/bytes show a repr-like form.
    match v {
        Value::Str(s) => s.clone(),
        other => py_repr_value(other),
    }
}

fn format_percent(fmt: &str, arg: Value) -> Result<Value, ReplError> {
    let args: Vec<Value> = match arg {
        // Tuple literals are represented as Value::List in this subset.
        Value::List(xs) => xs,
        other => vec![other],
    };

    let mut out = String::new();
    let mut chars = fmt.chars().peekable();
    let mut ai = 0usize;
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let Some(n) = chars.next() else {
            return Err(ReplError::ValueError("incomplete format".into()));
        };
        if n == '%' {
            out.push('%');
            continue;
        }
        let v = args
            .get(ai)
            .cloned()
            .ok_or_else(|| ReplError::ValueError("not enough arguments for format".into()))?;
        ai += 1;
        match n {
            's' => out.push_str(&to_print_string(&v)),
            'd' => match v {
                Value::Int(i) => out.push_str(&i.to_string()),
                _ => return Err(ReplError::TypeError("format %d expects int".into())),
            },
            'x' => match v {
                Value::Int(i) => out.push_str(&format!("{:x}", i)),
                _ => return Err(ReplError::TypeError("format %x expects int".into())),
            },
            _ => return Err(ReplError::ValueError("unsupported format".into())),
        }
    }
    Ok(Value::Str(out))
}

fn eval_subscript(
    e: &rustpython_parser::ast::ExprSubscript,
    env: &mut Env,
    sink: &mut PrintSink,
) -> Result<Value, ReplError> {
    let v = eval_expr(&e.value, env, sink)?;
    match e.slice.as_ref() {
        rustpython_parser::ast::Expr::Slice(slice_expr) => apply_slice(v, slice_expr, env, sink),
        _ => {
            let idx_v = eval_expr(&e.slice, env, sink)?;
            match v {
                Value::Dict(m) => {
                    match idx_v {
                        Value::Str(s) => Ok(m.get(&s).cloned().unwrap_or(Value::None)),
                        // Non-Python extension for LLM robustness:
                        // allow integer indexing into dict values using sorted key order.
                        Value::Int(i) => {
                            if i < 0 {
                                return Err(ReplError::ValueError("index out of range".into()));
                            }
                            let idx = i as usize;
                            let key = m.keys().nth(idx).cloned();
                            match key {
                                Some(k) => Ok(m.get(&k).cloned().unwrap_or(Value::None)),
                                None => Err(ReplError::ValueError("index out of range".into())),
                            }
                        }
                        _ => Err(ReplError::TypeError("dict index must be str".into())),
                    }
                }
                Value::Str(st) => {
                    let idx = match idx_v {
                        Value::Int(i) => i,
                        _ => return Err(ReplError::TypeError("index must be int".into())),
                    };
                    let idx = normalize_index(idx, st.chars().count() as i64)?;
                    let ch = st
                        .chars()
                        .nth(idx as usize)
                        .ok_or_else(|| ReplError::ValueError("index out of range".into()))?;
                    Ok(Value::Str(ch.to_string()))
                }
                Value::Bytes(b) => {
                    let idx = match idx_v {
                        Value::Int(i) => i,
                        _ => return Err(ReplError::TypeError("index must be int".into())),
                    };
                    let idx = normalize_index(idx, b.len() as i64)?;
                    Ok(Value::Int(b[idx as usize] as i64))
                }
                Value::List(xs) => {
                    let idx = match idx_v {
                        Value::Int(i) => i,
                        _ => return Err(ReplError::TypeError("index must be int".into())),
                    };
                    let idx = normalize_index(idx, xs.len() as i64)?;
                    Ok(xs[idx as usize].clone())
                }
                _ => Err(ReplError::TypeError("unsupported subscript".into())),
            }
        }
    }
}

fn eval_slice_expr(
    _e: &rustpython_parser::ast::ExprSlice,
    _env: &mut Env,
    _sink: &mut PrintSink,
) -> Result<Value, ReplError> {
    // Slice nodes are only meaningful inside Subscript. Prevent them from escaping.
    Err(ReplError::ForbiddenSyntax("bare slice".into()))
}

fn apply_slice(
    v: Value,
    s: &rustpython_parser::ast::ExprSlice,
    env: &mut Env,
    sink: &mut PrintSink,
) -> Result<Value, ReplError> {
    let start = if let Some(lo) = &s.lower {
        match eval_expr(lo, env, sink)? {
            Value::Int(i) => Some(i),
            Value::None => None,
            _ => return Err(ReplError::TypeError("slice start must be int".into())),
        }
    } else {
        None
    };
    let stop = if let Some(up) = &s.upper {
        match eval_expr(up, env, sink)? {
            Value::Int(i) => Some(i),
            Value::None => None,
            _ => return Err(ReplError::TypeError("slice stop must be int".into())),
        }
    } else {
        None
    };
    if s.step.is_some() {
        return Err(ReplError::ForbiddenSyntax("slice step".into()));
    }
    match v {
        Value::Str(st) => {
            let chars: Vec<char> = st.chars().collect();
            let (a, b) = normalize_slice(start, stop, chars.len() as i64);
            let out: String = chars[a..b].iter().collect();
            Ok(Value::Str(out))
        }
        Value::Bytes(bs) => {
            let (a, b) = normalize_slice(start, stop, bs.len() as i64);
            Ok(Value::Bytes(bs[a..b].to_vec()))
        }
        Value::List(xs) => {
            let (a, b) = normalize_slice(start, stop, xs.len() as i64);
            Ok(Value::List(xs[a..b].to_vec()))
        }
        _ => Err(ReplError::TypeError(
            "slicing supported only on str/bytes/list".into(),
        )),
    }
}

fn normalize_index(i: i64, len: i64) -> Result<i64, ReplError> {
    let mut idx = i;
    if idx < 0 {
        idx += len;
    }
    if idx < 0 || idx >= len {
        return Err(ReplError::ValueError("index out of range".into()));
    }
    Ok(idx)
}

fn normalize_slice(start: Option<i64>, stop: Option<i64>, len: i64) -> (usize, usize) {
    let mut a = start.unwrap_or(0);
    let mut b = stop.unwrap_or(len);
    if a < 0 {
        a += len;
    }
    if b < 0 {
        b += len;
    }
    a = a.clamp(0, len);
    b = b.clamp(0, len);
    if b < a {
        b = a;
    }
    (a as usize, b as usize)
}
