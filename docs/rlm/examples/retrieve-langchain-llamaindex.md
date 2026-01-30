# RustRLM `retrieve` サンプル（LangChain / LlamaIndex）

このサンプルは **RustRLM の `retrieve` を RAG の Retriever 置き換えとして使える** ことを、
LangChain / LlamaIndex それぞれで最小コードで示します。

- 文書は **inline documents**（リクエストに同梱）
- サンプル文書は **Paul Graham エッセイの短い要約（パラフレーズ）**
- 取得結果をそのまま **context 文字列**にして LLM へ渡す想定
- `retrieve` 自体は **LLM+REPL 方式**で動作（OpenAI APIを使用）

## 前提

1) RustRLM サーバ起動
```bash
CARGO_HOME=$PWD/.cargo-home cargo run -p rlm_runner -- serve --port 8080
```

2) OpenAI API キー
```bash
export OPENAI_API_KEY=...
```

3) Python 依存（任意）
- LangChain: `langchain-core`
- LlamaIndex: `llama-index`

## サンプル文書（Paul Graham エッセイ要約）
```python
PG_DOCS = [
    {
        "id": "pg-01",
        "text": (
            "Do Things that Don't Scale: early startups often win by doing manual, "
            "unscalable work to learn quickly and delight a tiny set of users; "
            "automation comes after the insights are clear."
        ),
        "metadata": {
            "source": "Paul Graham essay (paraphrase)",
            "title": "Do Things that Don't Scale",
        },
    },
    {
        "id": "pg-02",
        "text": (
            "Maker's Schedule, Manager's Schedule: makers need long, uninterrupted "
            "blocks for deep work, while managers operate in meeting-sized chunks; "
            "mixing the two harms maker productivity."
        ),
        "metadata": {
            "source": "Paul Graham essay (paraphrase)",
            "title": "Maker's Schedule, Manager's Schedule",
        },
    },
    {
        "id": "pg-03",
        "text": (
            "Hackers & Painters: good programming resembles design and craft; "
            "great work often comes from taste, iteration, and a willingness to refactor."
        ),
        "metadata": {
            "source": "Paul Graham essay (paraphrase)",
            "title": "Hackers & Painters",
        },
    },
    {
        "id": "pg-04",
        "text": (
            "Startup Ideas: the best startup ideas often start as something the founders "
            "themselves want, with a small but passionate initial market."
        ),
        "metadata": {
            "source": "Paul Graham essay (paraphrase)",
            "title": "How to Get Startup Ideas",
        },
    },
]
```

---

## LangChain サンプル

実行スクリプト: `python/examples/langchain_retrieve.py`

```python
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

    context = "\n\n".join(d.page_content for d in docs)
    print(context)


if __name__ == "__main__":
    main()
```

実行:
```bash
PYTHONPATH=python python3 python/examples/langchain_retrieve.py "How should an early startup approach scaling?"
```

---

## LlamaIndex サンプル

実行スクリプト: `python/examples/llamaindex_retrieve.py`

```python
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
    context = "\n\n".join(n.node.text for n in nodes)
    print(context)


if __name__ == "__main__":
    main()
```

実行:
```bash
PYTHONPATH=python python3 python/examples/llamaindex_retrieve.py "Why do makers need long blocks of time?"
```

---

## 置き換えポイント（RAG → RustRLM retrieve）
- RAG の **Retriever** を RustRLM に差し替えるだけで、
  `context` 生成を同様に行える。
- RustRLM の `retrieve` は **LLM依存** のため、同じ入力でも揺らぎがあり得る。
- 文書は inline で渡すため、既存の DB/索引をそのまま使う場合は
  **取得済み文書を `documents` に注入** すればよい。

必要に応じて `top_k` や `max_chunk_chars` を調整して、
**“RAGのRetriever置き換え”** を段階的に検証できます。
