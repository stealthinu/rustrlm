# TODO

このリポジトリは「安全・決定的・文字列操作に特化した Python-REPL 互換サブセット（Rust実装）」の仕様策定と、
その根拠（論文/実装/テストデータ）を整理するための作業用リポジトリです。

対象は "Recursive Language Models" (arXiv:2512.24601) と、その公式/非公式実装です。

## 未決定事項
- [x] 振る舞い抽出の一次ベースライン（優先）:
      - 非公式: `ysz/recursive-llm`（まずこれを動かして抽出する）
      - 参考: `alexzhang13/rlm`（参照点として保持）
- [ ] 「テスト」の正: 論文付属データ/公開ベンチマーク/実装内テスト どれを優先するか（運用ルール）
- [ ] importの許可範囲（方針）: 実測（ベースライン実行）で必要性を確認してから決める
      - [x] base64/zlib/binascii は「importが失敗」するが、globals注入で観測可能になる（差分実測あり）

## ソース取得と固定（再現性）
- [ ] 各repoのURLと、後で参照する「コミットSHA/タグ」を記録する
- [ ] 論文のPDF/HTMLをローカルに保存し、オフラインでも参照できるようにする
- [ ] 論文で使われるテストデータ/評価タスクを特定する
      - 論文の付録/補足資料を確認
      - 公式repo内の `data/`, `eval/`, `datasets/`, `benchmarks/` を確認
      - [x] arXiv HTML から埋め込み listing をデコードしてローカル化（`docs/rlm/paper-artifact-extraction.md`）
      - [x] RLM論文 Appendix E.1 の OOLONG-Pairs task 文を抽出（`extracted/paper/eval_artifacts.json`）
      - [x] ベンチマーク側（BrowseComp+, OOLONG, LongBench-v2, RULER）の配布先を整理し、取得を進める（`docs/rlm/eval/dataset-sources.md`）

## REPL振る舞いの抽出
- [ ] 抽出対象の「REPLプロトコル」を定義する（入力/出力/状態）:
      - 入力形式（単一行の式評価 vs 複数行のプログラム）
      - 許可する構文（式のみ? 代入? 制御構文?）
      - 出力（reprかprintか、改行、stdout/stderr）
      - エラー表現（例外文字列、トレースバック、決定的フォーマット）
      - 状態の持ち越し（globals/locals）
      - リソース制限（最大出力、最大ステップ、最大文字列サイズなど）
- [ ] 同一のテストケースをベースライン実装に流し、トランスクリプトを収集するハーネスを作る
- [ ] 差分比較用に正規化（空白/改行/プラットフォーム差）ルールを決める
- [ ] 仕様の根拠になる最小の「ゴールデントランスクリプト」を作る

## 進捗メモ（抜き出し済み）
- [x] 公式repoにて、` ```repl ` のコードブロック抽出、`FINAL/FINAL_VAR`、LocalREPLのstdout/stderr/例外フォーマットを確認
      - 根拠: `docs/rlm/official-implementation-notes.md`
- [x] 論文HTMLから listing と ```repl コードを抽出し、静的解析用のコーパスを作成
      - 根拠: `docs/rlm/paper-artifact-extraction.md`

## 非公式実装（一次ベースライン）を動かす
- [x] `upstream/recursive-llm` をクローンしてコミットSHAを固定する（`2fb46cc59e64cddc0768ce0bf428138dab3016eb`）
- [x] 依存関係/実行方法（README）を読み、最小で動く例を作る（ローカル実行は `PYTHONPATH=vendor/python` 前提）
- [ ] 「テストデータ」として何を流すか決めてハーネス化する
      - まずは `extracted/paper/repl_blocks/*.py`（論文プロンプト例）から開始
- [x] 論文コーパス（repl blocks）を非公式REPLに流して、成功/失敗とエラー形状を観測
      - 根拠: `docs/rlm/unofficial-baseline-run.md`
      - メモ: `import re` は `__import__ not found` で失敗（`re` はimport無しで使える）
- [x] 非公式実装のテストを実行して現状を記録（integrationに1件failあり）
      - 根拠: `docs/rlm/unofficial-test-status.md`

## 評価データ取得（一次目的）
- [x] BrowseComp-Plus（HF: `Tevatron/browsecomp-plus`, `Tevatron/browsecomp-plus-corpus`）をローカル取得
- [x] LongBench-v2（HF: `zai-org/LongBench-v2`）をローカル取得
- [x] OOLONG（小サイズ派生; HF: `tonychenxyz/oolong-synth-1k-16k`, `tonychenxyz/oolong-synth-32k-128k`）をローカル取得
- [ ] OOLONG（公式; HF: `oolongbench/oolong-synth`, `oolongbench/oolong-real`）の完全取得可否を評価（容量が大きい）
- [x] RULER から S-NIAH 相当の 50タスクを生成/固定する（seed/設定込み; `extracted/eval/s_niah.jsonl`）

## 「使われたREPL」抽出（評価データ駆動）
- [x] 各評価データから代表サンプル（小）を作り、非公式実装のREPLExecutor上で実行可能な“典型操作”を回すハーネスを作る
      - ハーネス: `tools/repl_probe_runner.py`
      - ログ: `extracted/runs/repl_probes.jsonl`
      - 結果メモ: `docs/rlm/eval/repl-probe-results.md`
- [x] 非公式ベースラインを30タスク規模で回し、REPL入力/出力のトランスクリプトを保存する
      - ランナー: `tools/run_unofficial_rlm_logged_eval.py`
      - タスク結果: `extracted/runs/unofficial_tasks30_logged.jsonl`
      - トランスクリプト: `extracted/runs/unofficial_tasks30_transcript.jsonl`
      - 集計: `extracted/runs/unofficial_tasks30_repl_analysis.json`
      - メモ: `docs/rlm/eval/unofficial-tasks30-repl-log.md`
- [x] 実行ログから、必要な構文/型/関数（reの扱い含む）と、非公式実装の癖を整理する（暫定）
      - `docs/rlm/eval/unofficial-tasks30-required-subset.md`
- [ ] その結果をもとに、Rustサブセット仕様（allowlist）を文書化する（`docs/plans/`）
      - [x] base64/zlib 注入時に増えるREPL機能の差分を実測（import失敗タスク再実行）
            - `docs/rlm/eval/unofficial-importfail-rerun-summary.md`

## Rustプロジェクト側の仕様とテスト
- [ ] `docs/plans/` に設計ドキュメントを書く:
      - 対象の表面構文サブセット
      - 許可する操作と厳密な意味論
      - セキュリティモデル（明示的な allowlist / denylist）
      - 制限（深さ/サイズ/時間）と、決定的な失敗モード
- [ ] テストファースト（TDD）で仕様を固める:
      - パーサ（受理/拒否する構文）
      - 評価器（文字列操作、必要なら index/slice）
      - エラー（型エラー、構文エラー、境界）
      - 決定性（同じ入力は同じ出力）

## 仕様ドラフト（作成済み）
- [x] 最終観測: REPLで実際に使われた機能一覧（union）
      - `docs/rlm/eval/final-observed-repl-surface.md`
- [x] Rustサブセット設計（ドラフト）
      - `docs/plans/2026-01-25-rlm-repl-subset-design.md`
