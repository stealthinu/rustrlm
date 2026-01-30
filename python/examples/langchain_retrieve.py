import os
import sys

from rustrlm_client import RustRLMClient

try:
    from rustrlm_client.integrations.langchain import RustRLMRetriever
except Exception as exc:
    raise SystemExit(
        "LangChain is not installed. Install langchain-core to run this example."
    ) from exc

from sample_docs import PG_DOCS


def main() -> None:
    query = (
        sys.argv[1]
        if len(sys.argv) > 1
        else "How should an early startup approach scaling?"
    )
    base_url = os.environ.get("RUSTRLM_BASE_URL", "http://127.0.0.1:8080")

    client = RustRLMClient(base_url=base_url)
    retriever = RustRLMRetriever(
        client,
        options={
            "documents": PG_DOCS,
            "retrieve_options": {"top_k": 2, "include_spans": True},
        },
    )

    if hasattr(retriever, "get_relevant_documents"):
        docs = retriever.get_relevant_documents(query)
    else:
        docs = retriever.invoke(query)

    print("# Query")
    print(query)
    print("\n# Retrieved Documents")
    for i, d in enumerate(docs, start=1):
        score = d.metadata.get("score")
        title = d.metadata.get("title") or d.metadata.get("source")
        print(f"[{i}] score={score} title={title}\n{d.page_content}\n")

    context = "\n\n".join(d.page_content for d in docs)
    print("# Context (RAG replacement input)")
    print(context)


if __name__ == "__main__":
    main()
