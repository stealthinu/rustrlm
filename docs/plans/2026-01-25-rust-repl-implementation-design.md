# Rust実装設計: Python文字列REPLサブセット（CLI）

目的:
- `ysz/recursive-llm`（修正後の観測用REPL）が実際に使っていたPythonコード表面を、Rustで安全・決定的に再現する。
- 最終的には非公式実装のREPL部分をこのCLIに置換して検証できること。

## 全体アーキテクチャ
- Rust側は「Pythonコードをパース→allowlist検証→評価器で実行→stdout相当を返す」だけを担う。
- importは禁止。必要なモジュール/関数は「globals注入」で提供する:
  - `re`, `base64`, `binascii`, `zlib`（安全版）
  - `context: str`, `query: str`
- 終了はREPL内部の関数ではなく、上位が `FINAL("...")` / `FINAL_VAR(x)` を構文的に抽出する（既存の非公式実装に合わせる）。

## ディレクトリ構成（案）
- `src/main.rs`:
  - CLI（JSON入出力）を提供。Python側から呼び出して置換しやすくする。
- `src/lib.rs`:
  - REPLエンジン公開API。
- `src/repl/mod.rs`:
  - `ReplEngine`（エントリポイント）/ `ExecRequest` / `ExecResponse`
- `src/repl/parse.rs`:
  - RustPython ParserでASTへ変換。
- `src/repl/allowlist.rs`:
  - ASTを走査し、許可ノード/許可名/許可属性のみを通す。
- `src/repl/eval.rs`:
  - ステートメント/式評価（小さい関数に分割）。
- `src/repl/value.rs`:
  - `Value`（`Str/Bytes/Int/Bool/None/List/Match/Func/Module` など）。
- `src/repl/builtins.rs`:
  - `print/len/max` と、modules注入（`re/base64/binascii/zlib`）を組み立て。
- `src/repl/modules/`:
  - `re_mod.rs`, `base64_mod.rs`, `binascii_mod.rs`, `zlib_mod.rs`
- `src/error.rs`:
  - 構造化エラー（決定的に表示）。

## 主要struct/メソッド設計
### `ReplEngine`
- `new(config: ReplConfig) -> Self`
- `exec(req: ExecRequest) -> ExecResponse`
  - `parse(req.code)` → `validate(ast)` → `eval(ast, env)` → `stdout` を `output` に格納

### `Env`
- `globals: HashMap<String, Value>`（注入済み: `context/query/re/...`）
- `locals_stack: Vec<HashMap<String, Value>>`（関数呼び出し用）
- `get(name) / set(name, value)`（スコープ規則は「ローカル優先、なければグローバル」）

### `AllowlistValidator`
- `validate(program_ast) -> Result<()>`
  - 禁止: `import/with/while/class/lambda/yield/dunder/getattr/globals/locals` 等
  - 許可: `Assign/If/Try/For/FunctionDef/Return/Expr/Pass` と観測済み式

### `Evaluator`
- `exec_stmt(stmt) -> Result<()>`
- `eval_expr(expr) -> Result<Value>`
- `call(func_value, args, kwargs) -> Result<Value>`
- stdoutは `PrintSink` に集約し、最大長で切り詰める

## CLIプロトコル（置換しやすさ優先）
stdin: JSON 1個
```json
{"context":"...","query":"...","code":"...","max_output_chars":2000}
```
stdout: JSON 1個
```json
{"ok":true,"output":"...","error":null}
```

## テスト戦略（t_wada）
- まず「システムテスト」10件（代表REPLスニペット）を固定して赤にする。
- 次に最小の単位（print/re.search等）から小さく緑にしていき、リファクタ。
- 仕様はテストが唯一の真実。境界条件（出力上限、zlib上限、禁止構文）を優先してテスト化する。

