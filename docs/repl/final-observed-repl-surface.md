# 最終観測: REPLで実際に使われた機能一覧（union）

このドキュメントは「実際にREPLへ投入され、成功したコード」をAST集計した結果から、
観測されたREPL表面（構文/呼び出し）を **union** としてまとめたもの。

対象ソース:
- ベースライン 30タスク: `extracted/runs/unofficial_tasks30_repl_analysis.json`
- import失敗タスク再実行（base64/zlib注入 + ガード追加）: `extracted/runs/unofficial_importfail_rerun_repl_analysis.json`
- union生データ: `extracted/runs/final_repl_feature_union.json`

注意:
- ここに列挙するのは「観測されたもの」であり「許可すべき最小セット」とは一致しない場合がある
  （例: base64/zlib注入で観測できるようになったが、Rust最終仕様に採用するかは別判断）。
- `FINAL("...")` / `FINAL_VAR(x)` は REPLで実行される関数ではなく、応答テキストから構文的に抽出する終了プロトコル。

## 1) REPLに提供される環境（前提）
非公式実装（＋観測用注入）での前提:
- 変数: `context: str`, `query: str`
- 関数: `recursive_llm(sub_query: str, sub_context: str) -> str`（サブLM）
- モジュール（globals注入で import 不要）:
  - 常時: `re`, `json`, `math`, `datetime`, `timedelta`, `Counter`, `defaultdict`
  - 追加実験: `base64`, `binascii`, `zlib`（`zlib.decompress` のみ、出力上限つき）

## 2) 観測された関数呼び出し（call_names）
上位（union、成功スニペットのみ）:
- `print`（最頻）
- `len`
- `max`（少数）

## 3) 観測された属性/メソッド呼び出し（attr_calls）
観測された主要パターン:
- 正規表現:
  - `re.search(...)`
  - `re.findall(...)`
  - `m.group(n)`（match）
  - `flags=re.IGNORECASE | re.DOTALL`
- 文字列:
  - `query.strip()`
  - `context.lower()`
  - `context.find(...)` / `something.find(...)`
- 追加実験（base64/zlib注入）で増えたもの:
  - `base64.b64decode(...)`
  - `binascii.hexlify(...)`
  - `zlib.decompress(...)`（安全版）
  - `*.decode(...)`（bytes->str）

## 4) 観測された構文/ASTノード（node_types）
主要なもの（成功スニペットのみ、union）:
- 基本: `Assign`, `Expr`, `If`, `IfExp`, `Pass`
- 呼び出し/参照: `Call`, `Name`, `Attribute`, `Constant`
- 演算: `BinOp`, `BitOr`, `UnaryOp`, `Compare`, `Not`, `Is`
- 追加実験（base64/zlib注入）で増えたもの:
  - 例外処理: `Try`, `ExceptHandler`
  - ループ: `For`
  - 添字/スライス: `Subscript`, `Slice`
  - 関数定義: `FunctionDef`（少数）
  - `Return`

補足:
- `import` は成功スニペットからは観測されない（非公式実装では `__import__` が無いため）。

