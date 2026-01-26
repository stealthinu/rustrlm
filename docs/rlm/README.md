# RustRLM ドキュメント

RustRLM は、Recursive Language Models (RLM) の実験・評価・置換検証をRustで行うためのプロジェクトです。
このリポジトリには、RLM本体に加えて「安全・決定的・軽量」な Python REPL サブセット（Rust実装）が内包されています。

## 目的
- RLMの制御ループ（depth/iteration/recursive calls）をRustで再実装し、再現性のあるログ（JSONL）を出す
- 内包REPLサブセットは allowlist 方式で安全に動かし、観測結果から必要機能だけをTDDで追加する

## 主要ドキュメント
- データセット入手: `docs/rlm/datasets.md`
- 設計（RLMランナー）: `docs/rlm/plans/2026-01-25-rust-rlm-runner-design.md`

