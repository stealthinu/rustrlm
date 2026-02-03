import argparse
import os
import re
from typing import Dict, List, Tuple

# Avoid writing under /home/stealth/.local and disable telemetry
os.environ.setdefault("CONTINUOUS_EVAL_DO_NOT_TRACK", "true")
os.environ.setdefault("XDG_DATA_HOME", "/tmp/continuous_eval")

REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
os.chdir(REPO_ROOT)

from continuous_eval.data_downloader import example_data_downloader  # noqa: E402

from rustrlm_client import RustRLMClient  # noqa: E402
from rustrlm_client.eval.matching import (  # noqa: E402
    ground_truth_to_doc_ids,
    hit_doc_id,
    hit_text_relaxed,
    hit_text_ws_substring,
)

try:
    from langchain_core.documents import Document as LCDocument  # noqa: E402
    from langchain_community.retrievers import BM25Retriever  # noqa: E402
except Exception as exc:  # pragma: no cover - optional dependency
    raise SystemExit(
        "langchain-community is required for this example."
    ) from exc

try:
    from llama_index.core.indices.keyword_table.simple_base import (  # noqa: E402
        SimpleKeywordTableIndex,
    )
    from llama_index.core.schema import Document as LIDocument  # noqa: E402
except Exception as exc:  # pragma: no cover - optional dependency
    raise SystemExit("llama-index is required for this example.") from exc


def split_paragraphs(text: str) -> List[str]:
    parts = [p.strip() for p in re.split(r"\n\s*\n", text) if p.strip()]
    return parts


def load_corpus() -> Tuple[List[Dict[str, str]], List[LCDocument], List[LIDocument]]:
    txt_dir = example_data_downloader("graham_essays/small/txt")
    docs_rustrlm: List[Dict[str, str]] = []
    docs_langchain: List[LCDocument] = []
    docs_llama: List[LIDocument] = []

    for fname in sorted(os.listdir(txt_dir)):
        path = os.path.join(txt_dir, fname)
        if not os.path.isfile(path) or not fname.endswith(".txt"):
            continue
        with open(path, "r", encoding="utf-8", errors="ignore") as f:
            content = f.read()
        for i, para in enumerate(split_paragraphs(content)):
            doc_id = f"{fname}:{i}"
            meta = {"source": fname, "doc_id": doc_id}
            docs_rustrlm.append({"id": doc_id, "text": para, "metadata": meta})
            docs_langchain.append(LCDocument(page_content=para, metadata=meta))
            docs_llama.append(LIDocument(text=para, metadata=meta, id_=doc_id))

    return docs_rustrlm, docs_langchain, docs_llama


def retrieve_rustrlm(
    client: RustRLMClient, query: str, docs: List[Dict[str, str]], top_k: int
) -> List[Tuple[str, str]]:
    res = client.retrieve(
        query,
        docs,
        {"top_k": top_k, "include_spans": False, "max_chunk_chars": 800},
    )
    out: List[Tuple[str, str]] = []
    for r in res.get("results", [])[:top_k]:
        doc_id = r.get("doc_id", "") or ""
        text = r.get("text", "") or ""
        out.append((doc_id, text))
    return out


def retrieve_langchain(retriever: BM25Retriever, query: str, top_k: int) -> List[Tuple[str, str]]:
    if hasattr(retriever, "get_relevant_documents"):
        docs = retriever.get_relevant_documents(query)
    else:
        docs = retriever.invoke(query)
    out: List[Tuple[str, str]] = []
    for d in docs[:top_k]:
        doc_id = ""
        if getattr(d, "metadata", None):
            doc_id = d.metadata.get("doc_id", "") or ""
        out.append((doc_id, d.page_content))
    return out


def retrieve_llamaindex(retriever, query: str, top_k: int) -> List[Tuple[str, str]]:
    nodes = retriever.retrieve(query)
    out: List[Tuple[str, str]] = []
    for n in nodes[:top_k]:
        node = getattr(n, "node", n)
        doc_id = ""
        meta = getattr(node, "metadata", None)
        if isinstance(meta, dict):
            doc_id = meta.get("doc_id", "") or ""
        if hasattr(node, "get_content"):
            out.append((doc_id, node.get_content()))
        else:
            out.append((doc_id, getattr(node, "text", "")))
    return out


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--top-k", type=int, default=3)
    parser.add_argument("--show", type=int, default=5, help="number of queries to print")
    parser.add_argument("--limit", type=int, default=0, help="limit number of queries")
    parser.add_argument(
        "--match-mode",
        choices=["strict", "relaxed", "doc_id"],
        default="strict",
        help="how to count a hit: strict=text ws-substring, relaxed=text fuzzy, doc_id=ground-truth doc_id match",
    )
    parser.add_argument("--base-url", default=os.environ.get("RUSTRLM_BASE_URL", "http://127.0.0.1:8080"))
    parser.add_argument("--timeout", type=int, default=180, help="HTTP timeout seconds for RustRLMClient")
    args = parser.parse_args()

    dataset = example_data_downloader("graham_essays/small/dataset")
    docs_rustrlm, docs_langchain, docs_llama = load_corpus()

    client = RustRLMClient(base_url=args.base_url, timeout=args.timeout)
    lc_retriever = BM25Retriever.from_documents(docs_langchain)
    # BM25Retriever uses `k` to control output size.
    if hasattr(lc_retriever, "k"):
        lc_retriever.k = args.top_k
    li_index = SimpleKeywordTableIndex.from_documents(docs_llama, show_progress=False)
    li_retriever = li_index.as_retriever(retriever_mode="simple")

    corpus_tuples = [(d["id"], d["text"]) for d in docs_rustrlm]

    total = 0
    hits_rustrlm = 0
    hits_langchain = 0
    hits_llama = 0

    rows = []

    for row in dataset._data:
        if args.limit and total >= args.limit:
            break
        total += 1
        question = row["question"]
        gt_contexts = row.get("ground_truth_context", []) or []

        rustrlm_results = retrieve_rustrlm(client, question, docs_rustrlm, args.top_k)
        lc_results = retrieve_langchain(lc_retriever, question, args.top_k)
        li_results = retrieve_llamaindex(li_retriever, question, args.top_k)

        rustrlm_doc_ids = [doc_id for doc_id, _ in rustrlm_results if doc_id]
        lc_doc_ids = [doc_id for doc_id, _ in lc_results if doc_id]
        li_doc_ids = [doc_id for doc_id, _ in li_results if doc_id]

        rustrlm_texts = [t for _, t in rustrlm_results]
        lc_texts = [t for _, t in lc_results]
        li_texts = [t for _, t in li_results]

        if args.match_mode == "doc_id":
            gt_doc_ids = ground_truth_to_doc_ids(corpus_tuples, gt_contexts)
            hit_r = hit_doc_id(rustrlm_doc_ids, gt_doc_ids)
            hit_lc = hit_doc_id(lc_doc_ids, gt_doc_ids)
            hit_li = hit_doc_id(li_doc_ids, gt_doc_ids)
        elif args.match_mode == "relaxed":
            hit_r = hit_text_relaxed(rustrlm_texts, gt_contexts)
            hit_lc = hit_text_relaxed(lc_texts, gt_contexts)
            hit_li = hit_text_relaxed(li_texts, gt_contexts)
            gt_doc_ids = set()
        else:
            hit_r = hit_text_ws_substring(rustrlm_texts, gt_contexts)
            hit_lc = hit_text_ws_substring(lc_texts, gt_contexts)
            hit_li = hit_text_ws_substring(li_texts, gt_contexts)
            gt_doc_ids = set()

        hits_rustrlm += 1 if hit_r else 0
        hits_langchain += 1 if hit_lc else 0
        hits_llama += 1 if hit_li else 0

        rows.append(
            {
                "question": question,
                "gt": gt_contexts[0] if gt_contexts else "",
                "gt_doc_ids": sorted(gt_doc_ids) if gt_doc_ids else [],
                "rustrlm": rustrlm_results[: args.top_k],
                "langchain": lc_results[: args.top_k],
                "llamaindex": li_results[: args.top_k],
                "hit": {"rustrlm": hit_r, "langchain": hit_lc, "llamaindex": hit_li},
            }
        )

    print("# Comparison (RustRLM vs LangChain BM25 vs LlamaIndex SimpleKeyword)")
    print(f"Match mode: {args.match_mode}")
    print(f"Total queries: {total}")
    print(
        "Hit@{k}: RustRLM={r:.2%}, LangChain-BM25={lc:.2%}, LlamaIndex-Simple={li:.2%}".format(
            k=args.top_k,
            r=(hits_rustrlm / total) if total else 0.0,
            lc=(hits_langchain / total) if total else 0.0,
            li=(hits_llama / total) if total else 0.0,
        )
    )

    print("\n# Sample queries")
    for i, row in enumerate(rows[: args.show], start=1):
        print(f"\n[{i}] Q: {row['question']}")
        if row["gt"]:
            print(f"GT: {row['gt']}")
        if row["gt_doc_ids"]:
            print("GT doc_ids:", ", ".join(row["gt_doc_ids"]))
        print("RustRLM hit:", row["hit"]["rustrlm"])
        print("LangChain-BM25 hit:", row["hit"]["langchain"])
        print("LlamaIndex-Simple hit:", row["hit"]["llamaindex"])
        print("- RustRLM top1:", row["rustrlm"][0][0] if row["rustrlm"] else "", "|", row["rustrlm"][0][1] if row["rustrlm"] else "")
        print("- LangChain top1:", row["langchain"][0][0] if row["langchain"] else "", "|", row["langchain"][0][1] if row["langchain"] else "")
        print("- LlamaIndex top1:", row["llamaindex"][0][0] if row["llamaindex"] else "", "|", row["llamaindex"][0][1] if row["llamaindex"] else "")


if __name__ == "__main__":
    main()
