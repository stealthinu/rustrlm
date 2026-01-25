# 公式実装メモ（alexzhang13/rlm）

このドキュメントは、公式実装 `alexzhang13/rlm` のコード/テストから読み取れる
「REPL（コード実行環境）のI/Oプロトコル」と「実装上の前提」を抜き出すためのメモです。

## 対象リビジョン（再現性）
- repo: https://github.com/alexzhang13/rlm
- commit: 6eb5f6be87eec214bd6b75b23f8dff60d9242f6c
  - message: `feat: add Daytona sandbox support (#36)`

## REPLコードブロックの検出（LLM出力→実行対象）
- LLMの出力テキストから、次の形式のコードブロックを抽出して実行する:
  - フェンス: ```repl ... ```
  - 正規表現（実装）: `r\"```repl\\s*\\n(.*?)\\n```\"`
  - 対象ファイル: `upstream/rlm/rlm/utils/parsing.py`

## 最終回答の表現（LLM出力→final answer）
- `FINAL(...)` または `FINAL_VAR(...)` を「行頭」で検出する。
  - `FINAL_VAR(...)` が優先される（環境がないと `None` になる仕様）。
  - `FINAL_VAR(x)` は環境側で `print(FINAL_VAR('x'))` を実行して値を回収する。
  - 対象ファイル: `upstream/rlm/rlm/utils/parsing.py`

## 実行結果のプロンプトへの戻し方（REPL output の整形）
- 直近のiterationで実行したコードは、次の形で次回プロンプトに足される:
  - `Code executed:\n```python\n{code}\n```\n\nREPL output:\n{result}`
- `result` は stdout/stderr を含み、さらに「変数名一覧」を含めることがある:
  - `REPL variables: ['a', 'b', ...]`
  - 対象ファイル: `upstream/rlm/rlm/utils/parsing.py`

## LocalREPL の挙動（重要）
`LocalREPL` は「同一プロセス内で `exec` を実行する」実装で、stdout/stderrを捕捉する。

- 実行: `exec(code, combined, combined)`（`combined` は globals+locals をマージ）
- 例外: `stderr += \"\\n{ExceptionName}: {message}\"` の形式で追記
- 状態: `self.locals` に変数が永続化される（キーが `globals` に含まれず `_` で始まらないもの）
- 対象ファイル: `upstream/rlm/rlm/environments/local_repl.py`

## persistent=True のときの「複数ターン状態」
LocalREPL は「context/history のバージョニング」を持つ（RLMのpersistentセッション用）。

- context:
  - `context_0`, `context_1`, ... を追加し、`context` は常に `context_0` の別名
  - `get_context_count()` で数を返す
- history:
  - `history_0`, `history_1`, ... を追加し、`history` は常に `history_0` の別名
  - `add_history` は deep copy を保存（参照ではない）
  - `get_history_count()` で数を返す
- 期待仕様はテストにまとまっている:
  - `upstream/rlm/tests/test_local_repl_persistent.py`
  - `upstream/rlm/tests/test_multi_turn_integration.py`

## セキュリティ上の観点（要注意）
公式LocalREPLは「安全 builtins」をうたうが、現状の allowlist に以下が含まれている:
- `__import__`
- `open`

このワークスペース（Rust実装）の制約では、ファイルI/Oやimportは許可しないため、
この差分は「抽出結果（公式の挙動）」として記録した上で、Rust側の仕様では明示的に禁止する必要がある。

