# RustRLM インプロセス Retrieval 設計（関数型）

## 概要
RustRLM の Retrieval 層を **サーバ不要で使えるインプロセス関数**として提供する。HTTP API は同一のコア関数を呼ぶ薄いラッパにし、LangChain/LlamaIndex 互換の薄いアダプタを維持する。Python 側は関数型 API のみを提供（クラス型クライアントは持たない）。

## 目的
- LangChain/LlamaIndex の Retriever を置き換え可能な Retrieval 層を提供する
- HTTP/インプロセスで **同一ロジック**を共有する
- 低依存・決定的・安全な Retrieval を実現する

## 非目標
- ベクトル検索や外部 DB 連携
- 学習やインデックス更新
- 高度なランキングや学習済み埋め込み

## API 方針（関数型）
### Python
```python
retrieve(
  query: str,
  documents: list[{"text": str, "metadata": dict, "id"?: str}],
  *,
  top_k: int = 5,
  min_score: float = 0.0,
  max_chunk_chars: int | None = None,
  return_spans: bool = True,
) -> list[{
  "id": str,
  "text": str,
  "metadata": dict,
  "score": float,
  "spans"?: list[{"start": int, "end": int}]
}]
```
- `id` は任意。未指定なら安定的に自動採番（入力順連番）。
- `max_chunk_chars` は **デフォルト無制限**（None）。

### Rust
```rust
fn retrieve(query: &str, docs: &[DocumentInput], opts: RetrieveOptions) -> Result<Vec<Hit>, RetrieveError>
```
- `DocumentInput { id: Option<String>, text: String, metadata: serde_json::Value }`
- `Hit { id: String, text: String, metadata: Value, score: f64, spans: Option<Vec<Span>> }`

## スコアリング
- 既存の簡易スコア（query の token との重なり）を継続
- `min_score` を満たすもののみ返却
- `top_k` で切り詰め

## エラーと決定性
- 入力型不一致・必須フィールド欠落は明示エラー
- Retrieval 自体は純粋関数的に動作し、外部 I/O を使わない

## LangChain / LlamaIndex 互換
- **LangChain**: `Document(page_content=text, metadata={... , "score": score})`
- **LlamaIndex**: `TextNode(text, metadata, id_)` + `NodeWithScore(node, score)`
- `QueryBundle` は `query_str` を抽出して利用

## HTTP API との関係
- HTTP API は **コア関数に委譲**するだけの薄い層
- ロジック二重化はしない

## テスト方針（TDD）
- Rust コアのユニット/統合テストで決定性、フィルタ、spans を固定
- PyO3 バインディングは入出力一致のみを検証
- Adapter は最小限の変換テストのみ

## 移行
- 既存 HTTP クライアントは維持
- インプロセスは追加機能として提供（Python は関数のみ）
