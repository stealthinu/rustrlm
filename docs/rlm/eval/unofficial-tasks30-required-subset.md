# 必要サブセット（暫定）: 非公式ベースライン 30タスク実測から

対象ログ:
- `extracted/runs/unofficial_tasks30_transcript.jsonl`
- 集計: `extracted/runs/unofficial_tasks30_repl_analysis.json` / `docs/rlm/eval/unofficial-tasks30-repl-features.md`

ここでは「この30タスクで **実際にREPLに投入され、かつ成功した** スニペット」から、
Rust実装で必要になりそうな機能を allowlist 形式で抽象化する。

## 1) 入力/環境（REPLが提供する変数）
- `context: str`
- `query: str`
- `recursive_llm(sub_query: str, sub_context: str) -> str`（サブLM呼び出し）
- `re`（正規表現モジュール; import無しで使用）

## 2) 文（Statements）
成功スニペットで観測:
- 代入: `x = ...`
- if: `if cond: ...`（`if not m: ...` など）
- pass: `pass`
- 式文: `expr` / `print(...)`

## 3) 式（Expressions）
成功スニペットで観測:
- 関数呼び出し: `print(...)`, `max(...)`
- 属性参照/メソッド呼び出し:
  - `re.search(...)`
  - `m.group(1)`（match object）
  - `query.strip()`
  - `context.lower()`
  - `something.find(...)`
- 条件式: `a if cond else b`
- 二項演算: `|`（例: `re.IGNORECASE | re.DOTALL` のフラグ結合）
- 定数: 文字列/数値

## 4) 正規表現（re）
成功スニペットで観測:
- `re.search(pattern, context, flags=...)`
- フラグ: `re.IGNORECASE`, `re.DOTALL`
- 戻り値（Match）: `.group(n)` を使用

## 5) 出力/終了プロトコル
- REPL側の観測可能な出力は `print(...)`（stdout）中心
- RLMの停止は `FINAL("...")` で行う（※ `FINAL` はREPLの関数ではない/REPL内で実行しない）

## 6) 失敗として観測されたが出現した機能（要検討）
以下はログ上出現したが、非公式REPLでは import 禁止などで失敗したもの:
- `import base64, zlib, binascii`（`__import__ not found`）
- for-loop / 関数定義 / comprehension を含む「純Python Base64デコーダ」試行

補足（重要）:
- `unofficial_tasks30_repl_analysis.json` の `repl.top_snippets[].code` に `import base64, zlib...` が出てくる場合、
  それは **REPLに投入された入力**（= 実行対象）である。該当スニペットの `ok=0` で、実際に失敗している。
- 非公式実装は `import` を無視しない。失敗した場合は `Error: Execution error: ...` を会話履歴に追加して、
  次のイテレーションに進む（そこで終了ではない。最大反復 `max_iterations` 到達で打ち切り）。
- 例外的に、LLM出力に `print(\"... import base64 ...\")` のように「文字列として import が含まれる」ケースは成功する。

方針（暫定）:
- import は全面禁止（既決）
- それでも必要なら「`base64_decode(...)` のような安全なビルトイン/組み込み関数」を追加する方が、
  REPLの表面積を増やさずに済む（ただしRLM文字列処理スコープに入れるか要判断）

## 7) 追加実験: base64/zlib を“事前注入”した場合に増える機能（参考）
「import失敗タスクだけを再実行し、`base64/binascii/zlib(安全)` を globals に注入」した場合の差分を別途測定した:
- まとめ: `docs/rlm/eval/unofficial-importfail-rerun-summary.md`

増える傾向（要点）:
- 関数/属性呼び出し: `base64.b64decode`, `zlib.decompress`, `binascii.hexlify`, `re.findall`, `*.decode`
- 構文: try/except, for, slice/subscript が増える

この差分は「サブセット仕様に base64/zlib を含めるか」を判断する材料として使う。
