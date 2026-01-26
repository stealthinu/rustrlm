from typing import Any, Dict, List, Optional

try:
    from llama_index.core.schema import NodeWithScore, TextNode
except Exception as e:  # pragma: no cover - optional dependency
    raise ImportError(
        "llama-index is not installed. Install llama-index to use this adapter."
    ) from e

from rustrlm_client import RustRLMClient


class RustRLMRetriever:
    def __init__(self, client: RustRLMClient, documents: List[Dict[str, Any]], options: Optional[Dict[str, Any]] = None):
        self.client = client
        self.documents = documents
        self.options = options

    def retrieve(self, query: str) -> List[NodeWithScore]:
        res = self.client.retrieve(query, self.documents, self.options)
        out: List[NodeWithScore] = []
        for r in res.get("results", []):
            meta = {"doc_id": r.get("doc_id"), "score": r.get("score"), "spans": r.get("spans")}
            m = r.get("metadata") or {}
            if isinstance(m, dict):
                meta.update(m)
            node = TextNode(text=r.get("text", ""), metadata=meta)
            out.append(NodeWithScore(node=node, score=float(r.get("score", 0.0))))
        return out
