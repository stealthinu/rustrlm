from typing import Any, Dict, List, Optional

try:
    from langchain_core.documents import Document
    from langchain_core.retrievers import BaseRetriever
except Exception as e:  # pragma: no cover - optional dependency
    raise ImportError(
        "LangChain is not installed. Install langchain-core to use this adapter."
    ) from e

from rustrlm_client import RustRLMClient


class RustRLMRetriever(BaseRetriever):
    client: RustRLMClient
    options: Optional[Dict[str, Any]] = None

    def __init__(self, client: RustRLMClient, options: Optional[Dict[str, Any]] = None):
        super().__init__()
        self.client = client
        self.options = options

    def _get_relevant_documents(self, query: str) -> List[Document]:
        # Expect caller to pass inline documents in options
        if not self.options or "documents" not in self.options:
            raise ValueError("documents must be provided in options for inline retrieval")
        documents = self.options["documents"]
        res = self.client.retrieve(query, documents, self.options.get("retrieve_options"))
        out: List[Document] = []
        for r in res.get("results", []):
            meta = {
                "doc_id": r.get("doc_id"),
                "score": r.get("score"),
                "spans": r.get("spans"),
                "trace_id": res.get("trace_id"),
            }
            m = r.get("metadata") or {}
            meta.update(m if isinstance(m, dict) else {"metadata": m})
            out.append(Document(page_content=r.get("text", ""), metadata=meta))
        return out
