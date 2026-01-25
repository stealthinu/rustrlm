# import失敗タスクの再実行（base64/zlib注入）: 観測されたREPL差分

目的:
- 30タスク実行ログで `__import__ not found` により失敗していたタスクだけを再実行し、
  `base64` / `binascii` / `zlib`（安全版）を *import不要* で使えるようにした場合に
  「REPLで実際に呼ばれる関数/命令」がどれだけ増えるかを実測する。

## 実行の変更点（観測優先）
本リポジトリのランナーが、非公式実装を in-process monkeypatch して以下を追加した:
- `restricted_globals` に `base64`, `binascii` を注入（import不要）
- `restricted_globals` に `zlib` を注入（`zlib.decompress` のみ提供、出力は上限付き）
- RestrictedPython変換が要求するガードを追加:
  - `_write_`（代入/代入系の変換で必要）
  - `_inplacevar_`（`x += y` などで必要）
- system prompt に「import禁止」「FINAL_VARを使う」等を明示

注: `import base64` 自体を許可したわけではない（`__import__` は引き続き無い）。

## 対象タスク（9件）
「以前のトランスクリプトで `Execution error: __import__ not found` が出たタスク」だけを対象にした:
- browsecomp_plus: 19, 23, 45, 67
- longbench_v2_codeqa: 66ece545821e116aacb1dd77, 66ecf139821e116aacb1e0e1, 66f3fd3a821e116aacb30533, 66fb77e7bb02136c067c7db1
- s_niah: 10

## 実行コマンド/ログ
- 再実行:
  - `tools/run_unofficial_rlm_logged_eval.py` に `--only-import-errors-from-transcript` と `--inject-b64zlib` を付与
- 出力:
  - 再実行タスク結果: `extracted/runs/unofficial_importfail_rerun_logged.jsonl`（9行）
  - 再実行トランスクリプト: `extracted/runs/unofficial_importfail_rerun_transcript.jsonl`
  - 再実行集計: `extracted/runs/unofficial_importfail_rerun_repl_analysis.json`
- 比較用（旧30タスクから該当9件だけ抜き出し）:
  - `extracted/runs/unofficial_importfail_baseline_transcript.jsonl`
  - `extracted/runs/unofficial_importfail_baseline_repl_analysis.json`
- 差分（AST特徴）:
  - `extracted/runs/unofficial_importfail_repl_diff.json`

結果:
- 9/9 タスクが `ok=true`（少なくともRLMとして終了できた）

## 観測された「増えた」REPL機能（差分）
差分の定義:
- ベースライン（旧runの該当9タスク） vs 再実行（base64/zlib注入）の
  `ast_features` を比較し、「新しく出現した」または「増加した」ものを列挙。

### 追加で観測された主な属性呼び出し（attr_calls）
- `base64.b64decode`
- `binascii.hexlify`
- `zlib.decompress`（安全版の `zlib` オブジェクト）
- `re.findall`（これまで主に `re.search` だったが、findallも出現）
- `*.decode`（例: `raw.decode`, `outb.decode`, `b.decode`, `decomp.decode`）
- `context.find`（以前は `find` 単体が多かった）

### 追加で観測された主な関数呼び出し（call_names）
- `len`
- （再実行では `print` の回数が増加）

### 追加で観測された主な構文/ASTノード（node_types）
- 例外処理: `Try`, `ExceptHandler`
- ループ: `For`
- スライス/添字: `Subscript`, `Slice`
- 比較/条件: `Compare`, `Is`, `If`
- 演算の増加: `Add`, `Sub`, `Mod`, `BinOp`, `UnaryOp`, `USub`
- タプル/引数: `Tuple`, `arg`, `Return`

解釈:
- `base64/zlib` を使えるようにしたことで「デコード→展開→文字列化」の典型パターンが走り、
  try/except・slice・decode系メソッドが増えた。

