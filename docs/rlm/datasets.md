# 評価データ取得先（調査ログ）

目的: RLM論文（2512.24601）で言及される評価タスクについて、入手先（GitHub/HuggingFace等）を特定し、
ローカルに取得・固定（再現性）する。

## BrowseComp-Plus
- プロジェクトサイト: `https://texttron.github.io/BrowseComp-Plus/`
  - GitHub: `https://github.com/texttron/BrowseComp-Plus`
  - Hugging Face datasets:
    - `Tevatron/browsecomp-plus`（タスク/QA側; parquet）
    - `Tevatron/browsecomp-plus-corpus`（コーパス側; parquet）

ローカル取得（実施済み）:
- `upstream/bench_datasets/hf/Tevatron__browsecomp-plus`
- `upstream/bench_datasets/hf/Tevatron__browsecomp-plus-corpus`

固定情報（Hugging Face / dataset repo HEAD; 取得日: 2026-01-24）:
- `Tevatron/browsecomp-plus`: `sha=144cff8e35b5eaef7e526346aa60774a9deb941f`（last_modified: 2025-12-20）
- `Tevatron/browsecomp-plus-corpus`: `sha=b27b02bc3e45511b8b82a13e6f90ce761df726f6`（last_modified: 2025-08-23）

## OOLONG
RLM論文は OOLONG の `trec_coarse` split を使うと記述している。

Hugging Face 上で “oolong” を検索し、公式と思われる公開データセットを確認:
- `oolongbench/oolong-synth`
- `oolongbench/oolong-real`

ただし、これらはサイズが非常に大きい（合計 tens of GB）ため、現時点では「より小さい派生データ」で
まず互換性/仕様抽出を進める（完全取得は別途）。

取得（小サイズ・実施済み）:
- `tonychenxyz/oolong-synth-1k-16k`（~96MB）
- `tonychenxyz/oolong-synth-32k-128k`（~802MB）

ローカル取得先:
- `upstream/bench_datasets/hf/tonychenxyz__oolong-synth-1k-16k`
- `upstream/bench_datasets/hf/tonychenxyz__oolong-synth-32k-128k`

固定情報（Hugging Face / dataset repo HEAD; 取得日: 2026-01-24）:
- `tonychenxyz/oolong-synth-1k-16k`: `sha=51a37432e94dd4b87f7c38b618c6f72a47f1cd94`（last_modified: 2026-01-21）
- `tonychenxyz/oolong-synth-32k-128k`: `sha=90b4e54e8b6473fd51a9a2c1fdec6b4ba131f41d`（last_modified: 2026-01-21）
- `oolongbench/oolong-synth`: `sha=49898a421f4b14f2c9cae084d2d270f930ff4c90`（last_modified: 2025-11-05; size目安: ~12.4GB）
- `oolongbench/oolong-real`: `sha=6bc9ef04866fcf005c9749b70649be69dd37fffb`（last_modified: 2025-11-05; size目安: ~20.0GB）

## LongBench-v2 CodeQA
- LongBench v2 paper PDF から LongBench repo を確認:
  - `https://github.com/THUDM/LongBench`
  - `https://longbench2.github.io`
- Hugging Face datasets:
  - `zai-org/LongBench-v2`（data.json; ~465MB）

ローカル取得（実施済み）:
- `upstream/bench_datasets/hf/zai-org__LongBench-v2`

固定情報（Hugging Face / dataset repo HEAD; 取得日: 2026-01-24）:
- `zai-org/LongBench-v2`: `sha=2b48e494f2c7a2f0af81aae178e05c7e1dde0fe9`（last_modified: 2024-12-20）

## S-NIAH（RULER由来）
- RULER repo:
  - `https://github.com/hsiehjackson/RULER`
- RLM論文の S-NIAH は RULER の single NIAH を元にした 50 タスク、と記述。
  - RULER repo には synthetic task 生成スクリプトがあるため、ここから再現する方向で進める。

ローカル取得（実施済み）:
- `upstream/RULER`（commit: `ab17b7853df4e0a30b78cd5d2b463ac7dff6ee13`）
