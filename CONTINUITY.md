Goal (incl. success criteria):
- (Main only) Make `graham_essays/small` retrieval comparisons reproducible and interpretable by adding explicit hit-eval modes (named “strict/relaxed/doc_id”) and printing enough context to debug misses.

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

Now:
- Explain per-query failures for `limit=5` using the new modes (most strict misses are snippet/escaping artifacts; remaining doc_id misses are true retrieval errors).
- Commit main-branch changes (eval match modes + tests) and delete unused feature worktrees/branches.

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
