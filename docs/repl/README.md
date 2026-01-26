# Python REPL サブセット（内包ライブラリ）

RustRLM は、コンテキスト処理を「プロンプトに全部入れる」のではなく、Python REPL 風の環境で `context` / `query` を変数として扱います。
このディレクトリは、そのために内包している **軽量・決定的・安全** な Python REPL サブセット（Rust実装）の仕様と観測結果をまとめます。

## 主要ドキュメント
- 観測されたREPLサーフェス（union）: `docs/repl/final-observed-repl-surface.md`
- 設計（サブセット仕様）: `docs/repl/plans/2026-01-25-rlm-repl-subset-design.md`
- 設計（Rust実装）: `docs/repl/plans/2026-01-25-rust-repl-implementation-design.md`

