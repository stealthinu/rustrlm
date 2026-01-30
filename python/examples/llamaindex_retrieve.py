import os
import sys

from rustrlm_client import RustRLMClient

try:
    from rustrlm_client.integrations.llamaindex import RustRLMRetriever
except Exception as exc:
    raise SystemExit(
        "LlamaIndex is not installed. Install llama-index to run this example."
    ) from exc

from sample_docs import PG_DOCS


def main() -> None:
    query = (
        sys.argv[1]
        if len(sys.argv) > 1
        else "Why do makers need long blocks of time?"
    )
    base_url = os.environ.get("RUSTRLM_BASE_URL", "http://127.0.0.1:8080")

    client = RustRLMClient(base_url=base_url)
    retriever = RustRLMRetriever(
        client,
        documents=PG_DOCS,
        options={"top_k": 2, "include_spans": True},
    )

    nodes = retriever.retrieve(query)

    print("# Query")
    print(query)
    print("\n# Retrieved Nodes")
    for i, n in enumerate(nodes, start=1):
        title = n.node.metadata.get("title") or n.node.metadata.get("source")
        print(f"[{i}] score={n.score} title={title}\n{n.node.text}\n")

    context = "\n\n".join(n.node.text for n in nodes)
    print("# Context (RAG replacement input)")
    print(context)


if __name__ == "__main__":
    main()
