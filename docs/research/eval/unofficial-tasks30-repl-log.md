# 非公式ベースライン: 30タスク実行ログ（REPLトランスクリプト）

この実行は「ベンチマークを解く」ことよりも、RLMが前提とする **Python-REPL互換サブセット** を
実測で抽出するためのログ取りが目的。

## 実行条件（要点）
- root model: `gpt-5.2`
- recursive model: `gpt-5-mini`
- task数: 30（BrowseComp+ 15 / CodeQA 5 / OOLONG(small) 5 / S-NIAH 5）
- max_depth=5, max_iterations=20
- system prompt は「コードのみ/ASCIIのみ」を追加して、非コード出力でREPLが壊れるのを抑制（ログ抽出の安定化）

## 出力
- タスク結果（1行=1タスク）:
  - `extracted/runs/unofficial_tasks30_logged.jsonl`
- REPLトランスクリプト（1行=イベント; LLM応答/REPL入力/REPL出力/FINAL等）:
  - `extracted/runs/unofficial_tasks30_transcript.jsonl`
- トランスクリプトからの集計（AST特徴/頻出スニペット）:
  - `extracted/runs/unofficial_tasks30_repl_analysis.json`
  - `docs/research/eval/unofficial-tasks30-repl-features.md`
