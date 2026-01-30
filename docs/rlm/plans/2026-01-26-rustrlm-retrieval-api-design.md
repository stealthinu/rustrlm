# RustRLM Retrieval API 設計（HTTP JSON, inline documents）

## 目的（成功条件）
- RustRLM を **“Retriever 置き換え”** として提供できるようにする。
- LangChain / LlamaIndex / 他SDK から **HTTP JSON** 経由で接続できる。
- **LLM+REPL 方式の retrieve** を提供し、非公式RLM実装の挙動に寄せる。
- まずは **inline documents 型**（リクエストで文書群を一緒に渡す）で最小実装。

## 非目標
- 永続インデックス（vector DB の代替）や大規模DB管理はしない。
- 学習/重み更新はしない。
- REPL側にネットワーク機能を追加しない（RustRLM本体がHTTPを担当）。
- 決定論リトリーバの維持（必要なら将来別エンドポイント）。

## 位置づけ
- RustRLMは「関連情報を探して取ってくる」レイヤーを担う。
- LangChain/LlamaIndexでは **Retriever** を差し替える形で利用する。
- 返り値は **passages（本文＋メタデータ＋スコア＋根拠位置）** を返す。
- スコアは **LLM自己評価（0.0〜1.0）** を採用する。

---

## HTTP API

### `GET /v1/health`
- **レスポンス**: `{"status":"ok","name":"rustrlm","version":"<semver>"}`

### `GET /v1/version`
- **レスポンス**: `{"name":"rustrlm","version":"<semver>","build":"dev"}`

### `POST /v1/retrieve`
**リクエスト**（JSON）:
```json
{
  "query": "string",
  "documents": [
    {"id":"doc1","text":"...","metadata":{"source":"..."}},
    {"id":"doc2","text":"..."}
  ],
  "options": {
    "top_k": 5,
    "max_chunk_chars": 800,
    "min_score": 0.0,
    "include_spans": true
  }
}
```

**レスポンス**（JSON）:
```json
{
  "trace_id": "uuid",
  "results": [
    {
      "doc_id": "doc2",
      "score": 0.42,
      "text": "...relevant span...",
      "metadata": {"source": "..."},
      "spans": [{"start": 120, "end": 155}]
    }
  ],
  "warnings": []
}
```

## スキーマ詳細

### Request
- `query` (string, required): 検索対象クエリ
- `documents` (array, required): inline 文書群
  - `id` (string, required)
  - `text` (string, required)
  - `metadata` (object, optional)
- `options` (object, optional)
  - `top_k` (int, default: 5)
  - `max_chunk_chars` (int, default: 800)
  - `min_score` (float, default: 0.0)
  - `include_spans` (bool, default: true)

### Response
- `trace_id` (string, uuid): 追跡用ID（ログに対応）
- `results` (array): スコア降順。tieは `doc_id` で安定ソート
- `warnings` (array of string)

### Result
- `doc_id` (string)
- `score` (float)
- `text` (string): 抽出された関連スパン
- `metadata` (object | null): 入力文書のメタデータを透過
- `spans` (array): `text` 内での根拠位置（byte index）

---

## 決定性と安全性
- 同一入力でも **LLMにより揺らぎが発生** する（非決定的）
- REPLはネットワーク/IO禁止のまま。HTTPはRustRLM本体のみが担当
- 文書は「リクエスト内のみ」。永続保存はしない

## エラーモデル
- 400: JSON不正 / 必須フィールド欠落
- 422: スキーマ違反（空文字のquery等）
- 500: 内部エラー（trace_id を返す）

**エラー例**:
```json
{"error": {"code": "invalid_request", "message": "documents is empty"}}
```

---

## SDK統合方針

### 共通クライアント
- Python薄いクライアントを用意し、
  - `retrieve(query, documents, options) -> results`
  - `health()`

#### Pythonクライアント（標準ライブラリのみ）
- 依存を増やさずに導入できるよう、`urllib.request` を使う
- JSONのシリアライズ/デシリアライズは標準 `json` を使用
- 例外は `RustRLMError` に統一

API案:
```python
client = RustRLMClient(base_url="http://127.0.0.1:8080", timeout=30)
client.health()  # -> dict
client.retrieve(query, documents, options=None)  # -> dict
```

### LangChain
- `BaseRetriever` 実装
- `_get_relevant_documents` がHTTP呼び出しを行い、`Document` へ変換
- 依存が無い場合は ImportError を明示

### LlamaIndex
- Retriever 実装 (`retrieve`)
- `NodeWithScore` へ変換
- 依存が無い場合は ImportError を明示

---

## 実装フェーズ
1. **HTTPサーバ + LLM+REPL retrieve** を最小で実装（本設計）
2. **Python薄いクライアント** + LangChain/LlamaIndexアダプタ
3. 追加機能（別エンドポイント、キャッシュなど）は必要になってから検討
