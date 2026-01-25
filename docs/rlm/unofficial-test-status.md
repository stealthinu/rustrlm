# 非公式実装のテスト状況（ローカル観測）

目的: `ysz/recursive-llm` を「一次ベースライン」として使うために、テストを回して現状の健全性と、
REPL仕様に関わる不一致/不具合を把握する。

## 対象
- `ysz/recursive-llm` @ `2fb46cc59e64cddc0768ce0bf428138dab3016eb`

## ローカル実行メモ
このワークスペースでは `venv` が作れないため、依存は `vendor/python` に `pip --target` で入れている。

## テスト結果
- `tests/test_repl.py`: pass（13/13）
- `tests/test_parser.py`: pass（11/11）
- `tests/test_core.py`: pass（11/11）
- `tests/test_integration.py`: fail（1 fail / 7 pass）

### 失敗内容（要点）
- `test_chunk_strategy` が失敗:
  - 失敗要因: REPLで実行される list comprehension 内で `context[i:i+10]` のように
    外側変数 `context` を式側で参照すると NameError になる
  - その結果 `num_chunks` が定義されず、`FINAL_VAR(num_chunks)` も解釈できずループが継続してしまう

この挙動は「実装上の癖/不具合」で、論文の想定戦略（chunking）と相性が悪い可能性がある。
仕様化に際しては、この挙動を互換として採用するか、Rust実装側で「直した挙動」にするかを決める必要がある。

補足:
- 同様の NameError は、実データを使った簡易プローブでも再現している（`docs/rlm/eval/repl-probe-results.md`）。
