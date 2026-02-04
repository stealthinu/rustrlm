use crate::error::ReplError;

use rustpython_parser::ast;

use super::parse::Program;

const FORBIDDEN_NAMES: &[&str] = &[
    "__import__",
    "eval",
    "exec",
    "open",
    "globals",
    "locals",
    "vars",
    "getattr",
    "setattr",
    "delattr",
];

pub fn validate(program: &Program) -> Result<(), ReplError> {
    for stmt in program {
        validate_stmt(stmt)?;
    }
    Ok(())
}

fn validate_stmt(stmt: &ast::Stmt) -> Result<(), ReplError> {
    use ast::Stmt::*;
    match stmt {
        Assign(s) => {
            for t in &s.targets {
                match t {
                    ast::Expr::Name(n) => validate_name(n.id.as_str())?,
                    _ => return Err(ReplError::ForbiddenSyntax("assign target".into())),
                }
            }
            validate_expr(&s.value)?;
            Ok(())
        }
        AugAssign(s) => {
            // Allow `x += expr` and similar on simple names only.
            match s.target.as_ref() {
                ast::Expr::Name(n) => validate_name(n.id.as_str())?,
                _ => return Err(ReplError::ForbiddenSyntax("augassign target".into())),
            }
            validate_expr(&s.value)?;
            Ok(())
        }
        Expr(s) => validate_expr(&s.value),
        If(s) => {
            validate_expr(&s.test)?;
            for st in &s.body {
                validate_stmt(st)?;
            }
            for st in &s.orelse {
                validate_stmt(st)?;
            }
            Ok(())
        }
        Pass(_) => Ok(()),
        For(s) => {
            match s.target.as_ref() {
                ast::Expr::Name(n) => validate_name(n.id.as_str())?,
                ast::Expr::Tuple(t) => {
                    for el in &t.elts {
                        match el {
                            ast::Expr::Name(n) => validate_name(n.id.as_str())?,
                            _ => return Err(ReplError::ForbiddenSyntax("for target".into())),
                        }
                    }
                }
                ast::Expr::List(t) => {
                    for el in &t.elts {
                        match el {
                            ast::Expr::Name(n) => validate_name(n.id.as_str())?,
                            _ => return Err(ReplError::ForbiddenSyntax("for target".into())),
                        }
                    }
                }
                _ => return Err(ReplError::ForbiddenSyntax("for target".into())),
            };
            validate_expr(&s.iter)?;
            for st in &s.body {
                validate_stmt(st)?;
            }
            for st in &s.orelse {
                validate_stmt(st)?;
            }
            Ok(())
        }
        Try(s) => {
            for st in &s.body {
                validate_stmt(st)?;
            }
            for h in &s.handlers {
                validate_handler(h)?;
            }
            for st in &s.orelse {
                validate_stmt(st)?;
            }
            for st in &s.finalbody {
                validate_stmt(st)?;
            }
            Ok(())
        }
        FunctionDef(s) => {
            validate_name(s.name.as_str())?;
            validate_args(&s.args)?;
            for st in &s.body {
                validate_stmt(st)?;
            }
            Ok(())
        }
        Return(s) => {
            if let Some(v) = &s.value {
                validate_expr(v)?;
            }
            Ok(())
        }
        Break(_) => Ok(()),
        Continue(_) => Ok(()),
        Raise(s) => {
            if let Some(exc) = &s.exc {
                // Allow `raise SystemExit` and `raise Exception(...)`-ish.
                match exc.as_ref() {
                    ast::Expr::Name(n) if n.id.as_str() == "SystemExit" => Ok(()),
                    ast::Expr::Call(c) => {
                        if let ast::Expr::Name(n) = c.func.as_ref() {
                            if n.id.as_str() == "Exception" {
                                for a in &c.args {
                                    validate_expr(a)?;
                                }
                                return Ok(());
                            }
                        }
                        Err(ReplError::ForbiddenSyntax("raise".into()))
                    }
                    _ => Err(ReplError::ForbiddenSyntax("raise".into())),
                }
            } else {
                Ok(())
            }
        }

        // Imports are treated as no-ops (or as bindings to pre-injected modules) by the evaluator.
        // This avoids spurious failures when the model "reflexively" writes `import ...`.
        Import(_) | ImportFrom(_) => Ok(()),
        While(_) | With(_) | ClassDef(_) | AsyncFunctionDef(_) | AsyncFor(_) | AsyncWith(_) => {
            Err(ReplError::ForbiddenSyntax(format!("{:?}", stmt)))
        }
        _ => Err(ReplError::ForbiddenSyntax(format!("{:?}", stmt))),
    }
}

fn validate_handler(h: &ast::ExceptHandler) -> Result<(), ReplError> {
    match h {
        ast::ExceptHandler::ExceptHandler(eh) => {
            if let Some(t) = &eh.type_ {
                match t.as_ref() {
                    ast::Expr::Name(n) if n.id.as_str() == "Exception" => {}
                    _ => return Err(ReplError::ForbiddenSyntax("except type".into())),
                }
            }
            if let Some(name) = &eh.name {
                validate_name(name.as_str())?;
            }
            for st in &eh.body {
                validate_stmt(st)?;
            }
            Ok(())
        }
    }
}

fn validate_args(args: &ast::Arguments) -> Result<(), ReplError> {
    if !args.posonlyargs.is_empty() || !args.kwonlyargs.is_empty() {
        return Err(ReplError::ForbiddenSyntax("posonly/kwonly args".into()));
    }
    if args.vararg.is_some() || args.kwarg.is_some() {
        return Err(ReplError::ForbiddenSyntax("*args/**kwargs".into()));
    }
    for a in &args.args {
        if a.default.is_some() {
            return Err(ReplError::ForbiddenSyntax("default args".into()));
        }
        validate_name(a.def.arg.as_str())?;
    }
    Ok(())
}

fn validate_expr(expr: &ast::Expr) -> Result<(), ReplError> {
    use ast::Expr::*;
    match expr {
        Constant(_) => Ok(()),
        Name(n) => validate_name(n.id.as_str()),
        BinOp(e) => {
            validate_expr(&e.left)?;
            validate_expr(&e.right)?;
            Ok(())
        }
        UnaryOp(e) => validate_expr(&e.operand),
        IfExp(e) => {
            validate_expr(&e.test)?;
            validate_expr(&e.body)?;
            validate_expr(&e.orelse)?;
            Ok(())
        }
        Compare(e) => {
            validate_expr(&e.left)?;
            for c in &e.comparators {
                validate_expr(c)?;
            }
            Ok(())
        }
        BoolOp(e) => {
            for v in &e.values {
                validate_expr(v)?;
            }
            Ok(())
        }
        Call(e) => {
            validate_expr(&e.func)?;
            for a in &e.args {
                validate_expr(a)?;
            }
            for k in &e.keywords {
                if let Some(arg) = &k.arg {
                    validate_name(arg.as_str())?;
                }
                validate_expr(&k.value)?;
            }
            Ok(())
        }
        Attribute(e) => {
            validate_expr(&e.value)?;
            validate_attr(e.attr.as_str())?;
            Ok(())
        }
        Subscript(e) => {
            validate_expr(&e.value)?;
            validate_expr(&e.slice)?;
            Ok(())
        }
        Slice(e) => {
            if let Some(v) = &e.lower {
                validate_expr(v)?;
            }
            if let Some(v) = &e.upper {
                validate_expr(v)?;
            }
            if let Some(v) = &e.step {
                validate_expr(v)?;
            }
            Ok(())
        }
        List(e) => {
            for v in &e.elts {
                validate_expr(v)?;
            }
            Ok(())
        }
        Dict(e) => {
            // Allow dict literals with string keys only.
            for k in &e.keys {
                match k {
                    Some(ast::Expr::Constant(c)) => {
                        // Only allow string keys
                        match &c.value {
                            ast::Constant::Str(_) => {}
                            _ => {
                                return Err(ReplError::ForbiddenSyntax(
                                    "dict key must be str literal".into(),
                                ))
                            }
                        }
                    }
                    None => return Err(ReplError::ForbiddenSyntax("dict unpack".into())),
                    _ => {
                        return Err(ReplError::ForbiddenSyntax(
                            "dict key must be str literal".into(),
                        ))
                    }
                }
            }
            for v in &e.values {
                validate_expr(v)?;
            }
            Ok(())
        }
        Tuple(e) => {
            for v in &e.elts {
                validate_expr(v)?;
            }
            Ok(())
        }
        ListComp(e) => {
            // Restrict to a single generator: [elt for name in iterable if cond]
            if e.generators.len() != 1 {
                return Err(ReplError::ForbiddenSyntax("listcomp generators".into()));
            }
            validate_expr(&e.elt)?;
            let gen = &e.generators[0];
            match &gen.target {
                ast::Expr::Name(n) => validate_name(n.id.as_str())?,
                _ => return Err(ReplError::ForbiddenSyntax("listcomp target".into())),
            }
            validate_expr(&gen.iter)?;
            for if_expr in &gen.ifs {
                validate_expr(if_expr)?;
            }
            if gen.is_async {
                return Err(ReplError::ForbiddenSyntax("async listcomp".into()));
            }
            Ok(())
        }
        // Not currently needed by observed surface
        _ => Err(ReplError::ForbiddenSyntax(format!("{:?}", expr))),
    }
}

fn validate_name(name: &str) -> Result<(), ReplError> {
    if name.starts_with('_') || name.contains("__") {
        return Err(ReplError::ForbiddenName(name.to_string()));
    }
    if FORBIDDEN_NAMES.iter().any(|&n| n == name) {
        return Err(ReplError::ForbiddenName(name.to_string()));
    }
    Ok(())
}

fn validate_attr(attr: &str) -> Result<(), ReplError> {
    if attr.starts_with('_') || attr.contains("__") {
        return Err(ReplError::ForbiddenName(attr.to_string()));
    }
    Ok(())
}
