# 非公式実装メモ（ysz/recursive-llm）

このドキュメントは、非公式実装 `ysz/recursive-llm` のコード/テストから読み取れる
「REPL（コード実行環境）のI/Oプロトコル」と「実装上の前提」を抜き出すためのメモです。

## 対象リビジョン（再現性）
- repo: https://github.com/ysz/recursive-llm
- commit: 2fb46cc59e64cddc0768ce0bf428138dab3016eb
  - message: `initial RLM implementation`

## REPL実装の概要
- 実行器: `REPLExecutor`（`upstream/recursive-llm/src/rlm/repl.py`）
- RestrictedPython を使い、制限付きで Python コードを実行する:
  - コンパイル: `compile_restricted_exec(code)`
  - 実行: `exec(byte_code.code, restricted_globals, env)`
- stdout を捕捉して「REPL出力」として返す。
- 最終行が「単純式」っぽい場合は `eval(last_line, ...)` を試し、その値を出力に追記する
  - キーワード/代入が含まれる場合は除外（`=`, `import`, `def`, `class`, `if`, `for`, `while`, `with`）

## コードフェンスの扱い
- `execute` は、LLMがコードを ```python か ``` で囲んだ場合に、それを剥がして実行する
  - ` ```repl ` 専用ではない（論文/公式とは微妙に違う）

## import と標準ライブラリ allowlist
- `_build_globals` 内で、以下のモジュール/型を「環境に直接注入」している:
  - `re`, `json`, `math`
  - `datetime`, `timedelta`
  - `Counter`, `defaultdict`
- テストでは `import os` が禁止であることを確認している:
  - `upstream/recursive-llm/tests/test_repl.py`

※ `import re` のような文が実際に通るかは、RestrictedPython側の制約に依存するため、
実際に `execute` で流して挙動（許可/拒否）を確認してから、互換サブセットの仕様に反映する。

### 実測（論文コーパスに対する実行）
- `import re` は失敗（`__import__ not found`）
- `re` は import 無しで使える（globals に注入されているため）
  - 根拠: `docs/research/unofficial-baseline-run.md`

## LLM向けシステムプロンプト（REPLの前提宣言）
非公式実装は「paper-style minimal prompt」として、環境に `re` が既にあることを明示している:
- `upstream/recursive-llm/src/rlm/prompts.py` の `build_system_prompt(...)`
  - `re: already imported regex module (use re.findall, re.search, etc.)`

このため、非公式実装の“想定”は `import re` ではなく「import無しで `re` を使う」寄りになっている。

## 注意: comprehension 内の名前解決に癖がある（重要）
RestrictedPython + `exec(globals, locals)` の組み合わせにより、
list comprehension の「式部分」に出てくる外部変数が `locals` 側だと NameError になるケースがある。

例:
- OK: `[c for c in context if c.isupper()]`（`context` が iterable clause 側に出てくる）
- NG: `[context[i:i+10] for i in range(...)]`（`context` が式側にのみ出てくる）→ `name 'context' is not defined`

この癖は、論文に近い chunking 戦略（`context[i:j]`）をそのまま書くと失敗し得るため、
互換サブセット仕様を決める際に「この挙動を互換として採用するか / 直すか」を明示する必要がある。

## FINAL / FINAL_VAR の解釈
- `FINAL(...)` / `FINAL_VAR(...)` の抽出は `upstream/recursive-llm/src/rlm/parser.py`。
- `FINAL(...)` は引用符（`"..."`, `'...'`, `'''...'''`, `\"\"\"...\"\"\"`）を前提に抽出する。
- `FINAL_VAR(x)` は `env[x]` を文字列化して返す（`x` は `\\w+` の識別子のみ）。
