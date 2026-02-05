# RustRLM（日本語 / Canonical）

RustRLM は **Recursive Language Models (RLM)** に着想を得た Rust 実装と、RLM が安全にドキュメントを扱うための
**制限付きの Python-REPL 互換サブセット**（Rust実装）を同一リポジトリで開発するプロジェクトです。

含まれるもの:
- Retriever 置き換え用途の Retrieval API（`POST /v1/retrieve`）を提供する Rust サーバ
- 制限付き Python REPL サブセット（allowlist方式、ファイルI/O/ネットワーク/サブプロセス禁止）
- Python クライアント + 比較/評価スクリプト（LangChain / LlamaIndex ベースライン含む）

English（翻訳）: `README.md`（一次情報はこの日本語READMEです）

ライセンス: MIT（`LICENSE` を参照）

---

## 必要環境
- Rust（stable）+ Cargo
- Python 3
- LLM を使う場合: `OPENAI_API_KEY`（`.env` に保存推奨）

## Workspace Layout
- `crates/python_string_repl`: 制限付き Python REPL サブセット（library）
- `crates/python_string_repl_cli`: REPL CLI（bin `python_string_repl`, JSON in/out）
- `crates/rlm_runner`: Rust サーバ / retrieval 実装（bin `rlm_runner`）
- `python/rustrlm_client`: Retrieval API 用の薄い Python クライアント

## Quickstart（Rustサーバ）
このリポジトリは project-local な `CARGO_HOME` を使えます（CI/sandbox向け）:

```bash
mkdir -p .cargo-home
export CARGO_HOME=$PWD/.cargo-home
```

サーバ起動:

```bash
export OPENAI_API_KEY=...   # もしくは .env に書く
cargo run -p rlm_runner -- serve --host 127.0.0.1 --port 8080
```

ヘルスチェック:

```bash
curl -s http://127.0.0.1:8080/v1/health
```

LLM呼び出しを無効化（決定的な fallback-only モード）:

```bash
export RUSTRLM_DISABLE_LLM=1
cargo run -p rlm_runner -- serve --host 127.0.0.1 --port 8080
```

## Python 依存関係（例/評価用）
venv を前提にしないため、依存は `vendor/python` に入れます:

```bash
python3 -m pip install -r requirements-vendor.txt --target vendor/python
```

スクリプト実行時は:

```bash
export PYTHONPATH=python:vendor/python
```

## 例: graham_essays/small 評価（RustRLM vs LangChain vs LlamaIndex）
サーバ起動（上記）後に:

```bash
export RUSTRLM_BASE_URL=http://127.0.0.1:8080
python3 python/examples/compare_retrievers_graham_essays.py \
  --top-k 3 --match-mode both \
  --langchain-backend vector --llamaindex-backend vector \
  --embedding-model text-embedding-3-small
```

## REPL CLI（JSON in/out）
stdin: 1つのJSON（`context/query/code`）:

```bash
cargo build -p python_string_repl_cli
./target/debug/python_string_repl <<'JSON'
{"context":"Hello WORLD","query":"  world  ","code":"s = query.strip()\nidx = context.lower().find(s.lower())\nprint(idx)\n"}
JSON
```

stdout: 1つのJSON（`ok/output/error/state`）

## Development / Contributing
Rust:

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Python（unit tests）:

```bash
PYTHONPATH=python:vendor/python python3 -m unittest -v \
  python.tests.test_rustrlm_client \
  python.tests.test_eval_matching
```

## 参照（論文・実装）
- 論文: [Recursive Language Models](https://arxiv.org/abs/2512.24601)（arXiv:2512.24601）
- ブログ: [Recursive Language Models | Alex L. Zhang](https://alexzhang13.github.io/blog/2025/rlm/)
- 公式実装:
  - [alexzhang13/rlm](https://github.com/alexzhang13/rlm)
  - [alexzhang13/rlm-minimal](https://github.com/alexzhang13/rlm-minimal)
- 非公式実装:
  - [ysz/recursive-llm](https://github.com/ysz/recursive-llm)
