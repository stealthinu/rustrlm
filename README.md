# RustRLM

RustRLM is a Rust implementation inspired by **Recursive Language Models (RLM)**, plus a **restricted, deterministic Python-REPL-compatible subset** used to safely manipulate strings and inspect documents.

It includes:
- A Rust HTTP server exposing a small Retrieval API (`POST /v1/retrieve`) that can be used as a retriever replacement.
- A restricted Python REPL subset (Rust, allowlist-based; no file I/O / networking / subprocesses).
- A thin Python client + runnable evaluation scripts (LangChain / LlamaIndex baselines included).

License: MIT (see `LICENSE`).

---

# RustRLM（日本語）

RustRLM は **Recursive Language Models (RLM)** に着想を得た Rust 実装と、RLM が安全にドキュメントを扱うための
**制限付き Python REPL サブセット**（Rust実装）を同一リポジトリで開発するプロジェクトです。

含まれるもの:
- Retrieval API（`POST /v1/retrieve`）を提供する Rust サーバ
- 安全第一の Python REPL サブセット（allowlist方式、ファイルI/O/ネットワーク/サブプロセス禁止）
- Python クライアント + 比較/評価スクリプト（LangChain / LlamaIndex ベースライン含む）

ライセンス: MIT（`LICENSE` を参照）

---

## Security / Secrets（重要）
- APIキーは **`.env` に保存**し、**絶対に git にコミットしない**（`.gitignore` 済み）。
- Issue やチャットにキーを貼らないでください。
- `.env` の例は `.env.example` を参照してください。

## Workspace Layout
- `crates/python_string_repl`: restricted Python REPL subset (library)
- `crates/python_string_repl_cli`: REPL CLI (bin `python_string_repl`, JSON in/out)
- `crates/rlm_runner`: Rust server / retrieval implementation (bin `rlm_runner`)
- `python/rustrlm_client`: thin Python client for the Retrieval API

## Quickstart (Rust server)
This repo uses a project-local `CARGO_HOME` (keeps CI/sandbox happy):
```bash
mkdir -p .cargo-home
export CARGO_HOME=$PWD/.cargo-home
```

Run the server:
```bash
export OPENAI_API_KEY=...   # or put it into .env
cargo run -p rlm_runner -- serve --host 127.0.0.1 --port 8080
```

Health check:
```bash
curl -s http://127.0.0.1:8080/v1/health
```

Disable LLM calls (deterministic fallback-only mode):
```bash
export RUSTRLM_DISABLE_LLM=1
cargo run -p rlm_runner -- serve --host 127.0.0.1 --port 8080
```

## Python Dependencies (for examples/evals)
We don't assume a usable venv here. Install deps into `vendor/python`:
```bash
python3 -m pip install -r requirements-vendor.txt --target vendor/python
```

Run scripts with:
```bash
export PYTHONPATH=python:vendor/python
```

## Example: graham_essays/small evaluation (RustRLM vs LangChain vs LlamaIndex)
Start the server (see above), then:
```bash
export RUSTRLM_BASE_URL=http://127.0.0.1:8080
python3 python/examples/compare_retrievers_graham_essays.py \
  --top-k 3 --match-mode both \
  --langchain-backend vector --llamaindex-backend vector \
  --embedding-model text-embedding-3-small
```

Notes:
- Vector baselines use OpenAI embeddings and will incur API cost.
- `match-mode both` computes both `strict` and `doc_id` in one run to avoid double-running RustRLM retrieval.

## REPL CLI (JSON in/out)
stdin: one JSON (`context/query/code`)
```bash
cargo build -p python_string_repl_cli
./target/debug/python_string_repl <<'JSON'
{"context":"Hello WORLD","query":"  world  ","code":"s = query.strip()\nidx = context.lower().find(s.lower())\nprint(idx)\n"}
JSON
```
stdout: one JSON (`ok/output/error/state`)

## Development / Contributing
Rust:
```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

Python (unit tests):
```bash
PYTHONPATH=python:vendor/python python3 -m unittest -v \
  python.tests.test_rustrlm_client \
  python.tests.test_eval_matching
```

## Large Local Artifacts
`upstream/` (external repos), `extracted/` (logs/artifacts), and `vendor/python/` (vendored deps) are intentionally gitignored.
For design/progress notes, see `docs/` and `CONTINUITY.md`.
