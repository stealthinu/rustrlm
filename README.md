# RustRLM

RustRLM is a Rust implementation inspired by **Recursive Language Models (RLM)**, plus a **restricted, deterministic Python-REPL-compatible subset** used to safely manipulate strings and inspect documents.

It includes:
- A Rust HTTP server exposing a small Retrieval API (`POST /v1/retrieve`) that can be used as a retriever replacement.
- A restricted Python REPL subset (Rust, allowlist-based; no file I/O / networking / subprocesses).
- A thin Python client + runnable evaluation scripts (LangChain / LlamaIndex baselines included).

**Japanese (canonical)**: `README.ja.md`  
**English**: this file (translation)

License: MIT (see `LICENSE`).

---

## Prerequisites
- Rust (stable) + Cargo
- Python 3
- If you enable LLM calls: `OPENAI_API_KEY` (recommended: store it in `.env`)

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

## References
- Paper: [Recursive Language Models](https://arxiv.org/abs/2512.24601) (arXiv:2512.24601)
- Blog: [Recursive Language Models | Alex L. Zhang](https://alexzhang13.github.io/blog/2025/rlm/)
- Official implementations:
  - [alexzhang13/rlm](https://github.com/alexzhang13/rlm)
  - [alexzhang13/rlm-minimal](https://github.com/alexzhang13/rlm-minimal)
- Unofficial implementation:
  - [ysz/recursive-llm](https://github.com/ysz/recursive-llm)
