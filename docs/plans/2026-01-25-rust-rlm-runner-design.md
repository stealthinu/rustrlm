# Rust版RLMランナー設計（REPLライブラリ分離）

## 目的（成功条件）
- 非公式実装 `upstream/recursive-llm` の「RLMオーケストレーション部分」をRustで再実装する。
- 既存の Rust REPLサブセット（`python_string_repl`）は **独立ライブラリ** として切り出し、RLMランナーは **別クレート/別バイナリ** として分離する。
- 実行ログ/トランスクリプトをJSONLで出力し、現状の解析ツール（`tools/analyze_repl_transcript.py` 等）で読める形式を維持する。
- 開発はTDD（t_wada流）で進め、まずテスト→最小実装→リファクタの順で進める。

## 非目標（この段階ではやらない）
- LiteLLM互換や複数プロバイダ対応はしない（OpenAI API固定）。
- データセット全量の同梱はしない（既存のローカル取得物 `upstream/bench_datasets` を参照）。
- REPL自体にネットワーク機能を追加しない（LLM呼び出しはRLMランナー側）。

## クレート構成（Cargo workspace）
```
./Cargo.toml (workspace)
./crates/
  python_string_repl/        # ライブラリ（安全なPythonサブセットREPLエンジン）
  python_string_repl_cli/    # バイナリ（stdin JSON -> stdout JSON）。bin名は python_string_repl を維持
  rlm_runner/                # バイナリ（RLM評価・ログ収集・OpenAI呼び出し）
```

### crates/python_string_repl（lib）
- 既存の `ReplEngine/ExecRequest/ExecResponse/ReplConfig` を公開APIとして維持。
- RLMとは独立。ネットワーク・ファイルIOを行わない。
- 振る舞いは「非公式baselineのREPL互換サブセット」に寄せる（`No code to execute`、echo-last-expr、出力トランケーション等）。

### crates/python_string_repl_cli（bin: python_string_repl）
- 既存のCLIプロトコル（stdin 1 JSON / stdout 1 JSON）を維持。
- `python_string_repl` ライブラリを呼ぶだけの薄いラッパ。
- 既存の `upstream/recursive-llm` からの置換検証で使えるよう、バイナリ名と出力形式を壊さない。

### crates/rlm_runner（bin）
- OpenAI API固定（`OPENAI_API_KEY` を `.env` から読む）。
- rootモデル: `gpt-5.2`、recursiveモデル: `gpt-5-mini` をデフォルトにする。
- CLIで `--dataset/--task-count/--out-jsonl/--transcript-jsonl` を受け取り、指定タスクを実行する。

## CLI仕様（rlm_runner）
### 例
```bash
CARGO_HOME=$PWD/.cargo-home cargo run -p rlm_runner -- \\
  run \\
  --dataset browsecomp_plus \\
  --task-count 30 \\
  --seed 0 \\
  --out-jsonl extracted/runs/rust_rlm_tasks30.jsonl \\
  --transcript-jsonl extracted/runs/rust_rlm_tasks30_transcript.jsonl
```

### 主なフラグ（案）
- `run` サブコマンド
  - `--dataset`（複数指定可。例: browsecomp_plus,longbench_v2_codeqa,oolong_synth_small,s_niah）
  - `--task-count`（合計件数）
  - `--seed`（タスク抽出の決定性）
  - `--max-depth`（デフォルト: 5）
  - `--max-iterations`（デフォルト: 20）
  - `--max-output-chars`（REPL出力上限。デフォルト: 2000）
  - `--out-jsonl`（結果）
  - `--transcript-jsonl`（LLM/REPLのイベントログ）

## トランスクリプト仕様（互換重視）
現状の `tools/analyze_repl_transcript.py` が読めるよう、以下のイベントをJSONLで出す：
- `task_start`（dataset/task_id/query/context_len/model等）
- `llm_response`（depth/iteration/model_selected/content/elapsed_ms）
- `final_parsed`（answer）
- `repl_input`（code）
- `repl_output` or `repl_error`（output or error）
- `task_end`（ok/error/answer_snippet/stats）

## FINAL仕様（非公式実装に合わせる）
- `FINAL("...")`：引用符に入ったリテラルだけを抽出して答えにする（実行しない）。
- `FINAL_VAR(name)`：REPL環境にある変数 `name` の `str()` を答えにする。
- それ以外（例: `FINAL(ans)`）は答えとして扱わない（=未確定のままループ継続）。

## 実装方針（TDD）
1. まず `FINAL/FINAL_VAR` パーサとイベントロガーをテストで固める。
2. 次に「LLMクライアント（OpenAI Responses API）」はI/O境界として薄くし、コアはFakeでテスト可能にする。
3. REPLは `python_string_repl` ライブラリを直接呼び（CLI subprocessは使わない）、状態を保持して繰り返す。
4. 最後にデータセットローダ（特にBrowseComp+ Parquetのlist<struct>）を追加し、`--dataset/--task-count` を実働させる。

