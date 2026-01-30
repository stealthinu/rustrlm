import argparse
import os
import re
import sys
from typing import Dict, Iterable, List, Tuple

# Avoid writing under /home/stealth/.local and disable telemetry
os.environ.setdefault("CONTINUOUS_EVAL_DO_NOT_TRACK", "true")
os.environ.setdefault("XDG_DATA_HOME", "/tmp/continuous_eval")

REPO_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
os.chdir(REPO_ROOT)

from continuous_eval.data_downloader import example_data_downloader  # noqa: E402

from rustrlm_client import RustRLMClient  # noqa: E402

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


def normalize_text(text: str) -> str:
    return " ".join(text.lower().split())


def ground_truth_hit(retrieved_texts: Iterable[str], ground_truths: List[str]) -> bool:
    normalized_texts = [normalize_text(t) for t in retrieved_texts]
    for gt in ground_truths:
        gt_norm = normalize_text(gt)
        if not gt_norm:
            continue
        for t_norm in normalized_texts:
            if gt_norm in t_norm:
                return True
    return False


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
            meta = {"source": fname}
            docs_rustrlm.append({"id": doc_id, "text": para, "metadata": meta})
            docs_langchain.append(LCDocument(page_content=para, metadata=meta))
            docs_llama.append(LIDocument(text=para, metadata=meta, id_=doc_id))

    return docs_rustrlm, docs_langchain, docs_llama


def retrieve_rustrlm(client: RustRLMClient, query: str, docs: List[Dict[str, str]], top_k: int) -> List[str]:
    res = client.retrieve(
        query,
        docs,
        {"top_k": top_k, "include_spans": False, "max_chunk_chars": 800},
    )
    return [r.get("text", "") for r in res.get("results", [])]


def retrieve_langchain(retriever: BM25Retriever, query: str) -> List[str]:
    if hasattr(retriever, "get_relevant_documents"):
        docs = retriever.get_relevant_documents(query)
    else:
        docs = retriever.invoke(query)
    return [d.page_content for d in docs]


def retrieve_llamaindex(retriever, query: str) -> List[str]:
    nodes = retriever.retrieve(query)
    out: List[str] = []
    for n in nodes:
        node = getattr(n, "node", n)
        if hasattr(node, "get_content"):
            out.append(node.get_content())
        else:
            out.append(getattr(node, "text", ""))
    return out


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--top-k", type=int, default=3)
    parser.add_argument("--show", type=int, default=5, help="number of queries to print")
    parser.add_argument("--limit", type=int, default=0, help="limit number of queries")
    parser.add_argument("--base-url", default=os.environ.get("RUSTRLM_BASE_URL", "http://127.0.0.1:8080"))
    parser.add_argument("--timeout", type=int, default=180, help="HTTP timeout seconds for RustRLMClient")
    args = parser.parse_args()

    dataset = example_data_downloader("graham_essays/small/dataset")
    docs_rustrlm, docs_langchain, docs_llama = load_corpus()

    client = RustRLMClient(base_url=args.base_url, timeout=args.timeout)
    lc_retriever = BM25Retriever.from_documents(docs_langchain)
    li_index = SimpleKeywordTableIndex.from_documents(docs_llama, show_progress=False)
    li_retriever = li_index.as_retriever(retriever_mode="simple")

    total = 0
    hits_rustrlm = 0
    hits_langchain = 0
    hits_llama = 0

    rows = []

    for row in dataset._data:
        total += 1
        if args.limit and total > args.limit:
            break
        question = row["question"]
        gt_contexts = row.get("ground_truth_context", []) or []

        rustrlm_texts = retrieve_rustrlm(client, question, docs_rustrlm, args.top_k)
        lc_texts = retrieve_langchain(lc_retriever, question)
        li_texts = retrieve_llamaindex(li_retriever, question)

        hit_r = ground_truth_hit(rustrlm_texts, gt_contexts)
        hit_lc = ground_truth_hit(lc_texts, gt_contexts)
        hit_li = ground_truth_hit(li_texts, gt_contexts)

        hits_rustrlm += 1 if hit_r else 0
        hits_langchain += 1 if hit_lc else 0
        hits_llama += 1 if hit_li else 0

        rows.append(
            {
                "question": question,
                "gt": gt_contexts[0] if gt_contexts else "",
                "rustrlm": rustrlm_texts[: args.top_k],
                "langchain": lc_texts[: args.top_k],
                "llamaindex": li_texts[: args.top_k],
                "hit": {"rustrlm": hit_r, "langchain": hit_lc, "llamaindex": hit_li},
            }
        )

    print("# Comparison (RustRLM vs LangChain BM25 vs LlamaIndex SimpleKeyword)")
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
        print("RustRLM hit:", row["hit"]["rustrlm"])
        print("LangChain-BM25 hit:", row["hit"]["langchain"])
        print("LlamaIndex-Simple hit:", row["hit"]["llamaindex"])
        print("- RustRLM top1:", row["rustrlm"][0] if row["rustrlm"] else "")
        print("- LangChain top1:", row["langchain"][0] if row["langchain"] else "")
        print("- LlamaIndex top1:", row["llamaindex"][0] if row["llamaindex"] else "")


if __name__ == "__main__":
    main()
