Goal (incl. success criteria):
- Prepare repo for OSS release: add license, improve README (JP+EN), and remove/clarify unfinished CLI commands (e.g. `rlm_runner run`) to avoid confusion.

Constraints/Assumptions:
- Follow AGENTS.md instructions for this workspace.
- Write tests first for behavior changes (t_wada TDD).
- Maintain security guardrails (no file I/O, networking, subprocesses in interpreter).
- Python deps are vendored under `vendor/python`; run scripts with `PYTHONPATH=python:vendor/python`.
- Secrets: API keys are stored locally in `.env` (gitignored); never commit secrets.

Key decisions:
- Evaluate `graham_essays/small` with multiple match modes because RustRLM `retrieve` can return LLM-provided `snippet` (not always containing `ground_truth_context` even when the doc is correct).
- Match modes (CLI):
  - `strict`: whitespace-normalized substring (close to original script behavior)
  - `relaxed`: alnum-substring OR fuzzy similarity on alnum-normalized strings (SequenceMatcher; default threshold 0.72)
  - `doc_id`: reverse-map `ground_truth_context` -> corpus `doc_id` and compare by `doc_id` only
- Vector baselines:
  - LangChain: FAISS + OpenAIEmbeddings (model configurable; default `text-embedding-3-small`)
  - LlamaIndex: VectorStoreIndex + OpenAIEmbedding (same embedding model)

State:
- Repo has multiple worktrees, but evaluation/dev focus is `main` only.
- RustRLM server provides `POST /v1/retrieve` (LLM+REPL when `.env` contains `OPENAI_API_KEY`); run via `cargo run -p rlm_runner -- serve --port <PORT>`.
- `python/examples/compare_retrievers_graham_essays.py` supports `--match-mode` and prints `doc_id|text` for top1.

Done:
- Added eval helpers + TDD:
  - `python/rustrlm_client/eval/matching.py`
  - `python/tests/test_eval_matching.py`
- Updated graham essays comparison script:
  - `python/examples/compare_retrievers_graham_essays.py` (`--match-mode strict|relaxed|doc_id`, fixed limit counting, propagates `doc_id` through metadata)
- Ran eval (server port 8099, limit=5, top_k=3), outputs in `/tmp`:
  - strict: RustRLM=20%, LangChain-BM25=20%, LlamaIndex=20% (`/tmp/rustrlm_compare_graham_strict.txt`)
  - relaxed: RustRLM=40%, LangChain-BM25=40%, LlamaIndex=20% (`/tmp/rustrlm_compare_graham_relaxed.txt`)
  - doc_id: RustRLM=60%, LangChain-BM25=40%, LlamaIndex=20% (`/tmp/rustrlm_compare_graham_docid.txt`)
- Ran full eval (55 queries, top_k=3) and computed both match modes in one pass:
  - strict: RustRLM=14.55% (8/55), LangChain-BM25=45.45% (25/55), LlamaIndex-Simple=45.45% (25/55)
  - doc_id: RustRLM=58.18% (32/55), LangChain-BM25=49.09% (27/55), LlamaIndex-Simple=45.45% (25/55)
  - output: `/tmp/rustrlm_compare_graham_full_strict_docid.txt`
  - RustRLM debug_rlm_iterations sum=123 (avg 2.24 / query) => ~123 chat.completions calls for this run (json_repair not observed in this pass).
- Added vector retriever baselines to graham essays comparison script (same OpenAI embeddings model for both frameworks) and a `both` match mode to avoid double-running costly retrieval.
- Ran full eval (55 queries, top_k=3) with vector baselines, outputs in `/tmp`:
  - vector: `/tmp/rustrlm_compare_graham_full_vector.txt`
    - strict: RustRLM=20.00%, LangChain-Vector=76.36%, LlamaIndex-Vector=69.09%
    - doc_id: RustRLM=58.18%, LangChain-Vector=78.18%, LlamaIndex-Vector=70.91%
  - lexical (current code): `/tmp/rustrlm_compare_graham_full_lexical.txt`
    - strict: RustRLM=18.18%, LangChain-BM25=45.45%, LlamaIndex-SimpleKeyword=47.27%
    - doc_id: RustRLM=58.18%, LangChain-BM25=49.09%, LlamaIndex-SimpleKeyword=47.27%
- Committed main changes (2 commits):
  - `5a71631` Add strict/relaxed/doc_id match modes for graham essays eval
  - `f28c804` Add eval matching helpers and tests
- Deleted unused worktrees/branches (discarding their uncommitted changes): `.worktrees/llm-retrieve`, `.worktrees/inprocess-retrieval`, `feature/llm-retrieve`, `feature/inprocess-retrieval`.
- Estimated OpenAI call volume for graham essays:
  - sample (first 5 queries): total `debug_rlm_iterations`=12 => ~12 chat.completions calls (avg 2.4 calls/query)
  - dataset size: 55 queries => ~132 calls for a full run (single pass)
- OSS prep:
  - Confirmed referenced unofficial implementation (`upstream/recursive-llm`) uses MIT license.
  - Added `LICENSE` (MIT), split README into EN translation + JA canonical (`README.md` + `README.ja.md`), aligned EN content to JP canonical (no extra Security/Artifacts sections), added paper/implementation references, removed unfinished `rlm_runner run`.
- Added GitHub Actions CI:
  - `.github/workflows/ci.yml` runs Rust (fmt/clippy/test) + Python unit tests on PRs and pushes to `main`.

Now:
- If desired: configure branch protection on GitHub to require CI checks before merge.

Next:
- If needed: add an “explain” output mode that prints retrieved top-k doc_ids per retriever and highlights why strict/relaxed/doc_id differs.
- If goal is higher doc_id hit rate: tune RustRLM retrieve prompt and/or change response to always return full doc text + spans (instead of snippet-only) for RAG drop-in.

Open questions (UNCONFIRMED if needed):
- None.

Working set (files/ids/commands):
- `python/examples/compare_retrievers_graham_essays.py`
- `python/rustrlm_client/eval/matching.py`
- `python/tests/test_eval_matching.py`
- `/tmp/rustrlm_compare_graham_strict.txt`
- `/tmp/rustrlm_compare_graham_relaxed.txt`
- `/tmp/rustrlm_compare_graham_docid.txt`
- `/tmp/rustrlm_compare_graham_full_vector.txt`
- `/tmp/rustrlm_compare_graham_full_lexical.txt`
