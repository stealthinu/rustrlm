# RustRLM LLM+REPL Retrieve 設計

## 目的（成功条件）
- `/v1/retrieve` を **LLM+REPL 方式**で再定義し、Retriever の置き換えとして機能すること。
- 返却スキーマは既存 `RetrieveResponse` 互換（`doc_id/text/score/spans`）を維持すること。
- 非公式 Python 実装（`upstream/recursive-llm`）の **RLMループ挙動**に合わせること。
- Rustのベストプラクティスに従い、責務分離・テスト容易性・安全性を担保すること。

## 非目標
- ベクタDBや永続インデックスの実装。
- 高度なランキング学習やembedding検索。
- 既存の決定論リトリーバ実装の保持（必要なら将来別エンドポイントで検討）。

---

## 全体アーキテクチャ

### 主要コンポーネント
- `rlm_runner`（HTTPサーバ + RLMループ + LLMクライアント）
- `python_string_repl`（REPL実行。LLMが出力したコードを評価）
- `final_parser`（`FINAL(...)` / `FINAL_VAR(...)` の抽出）

### モジュール構成（Rust）
- `crates/rlm_runner/src/retrieve.rs`
  - `async fn retrieve(req: RetrieveRequest, ctx: &RetrieveContext) -> RetrieveResponse`
  - `RetrieveContext`（LLMクライアント・REPL・設定を保持）
- `crates/rlm_runner/src/rlm_loop.rs`（新規）
  - 非公式実装の `RLM.acompletion` と同等の制御ループ
  - max_depth / max_iterations / retries / timeout
- `crates/rlm_runner/src/llm_client.rs`（新規）
  - OpenAI APIクライアント（gpt-5.2 / gpt-5-mini）
  - テスト用 `MockLLM`
- `crates/rlm_runner/src/prompts.rs`（新規）
  - RLM用システムプロンプト + retrieve専用指示

---

## `/v1/retrieve` の挙動（LLM+REPL）

### 入力（変更なし）
```json
{
  "query": "string",
  "documents": [{"id":"doc1","text":"...","metadata":{}}],
  "options": {"top_k": 5, "max_chunk_chars": 800, "min_score": 0.0, "include_spans": true}
}
```

### 返却（変更なし）
```json
{
  "trace_id": "uuid",
  "results": [{"doc_id":"doc1","score":0.73,"text":"...","metadata":{},"spans":[{"start":10,"end":25}]}],
  "warnings": []
}
```

### LLM出力フォーマット
LLMは **`FINAL(\"\"\"{json}\"\"\")`** 形式で以下のJSONを返す：
```json
{
  "results": [
    {"doc_id": "doc1", "score": 0.73, "snippet": "..." },
    {"doc_id": "doc2", "score": 0.41, "snippet": "..." }
  ],
  "warnings": []
}
```

- `score` は **0.0〜1.0**（LLM自己評価）
- `snippet` は原文からの **短い抜粋**（スパン抽出に利用）

### Rust側の補正ルール
- `score` が欠落/範囲外 → **0.0–1.0にクランプ**
- `snippet` が見つからない → `text` は先頭 `max_chunk_chars` を返し `spans` は空
- JSONが壊れている → **1回だけ修正プロンプトで再試行**, 失敗なら空結果+警告

---

## REPL環境の注入

REPLに以下を注入する：
- `query: str`
- `documents: list[{"id":str,"text":str,"metadata":...}]`
- `top_k: int`
- `max_chunk_chars: int`
- `min_score: float`

LLMには「**documents を REPL から参照して検索/抽出する**」ように促す。REPL側は
`python_string_repl` の安全なサブセットで評価し、**ファイルIO/ネットワーク等は禁止**のまま維持する。

---

## プロンプト設計（LLM）

- システムプロンプトは非公式実装の `prompts.py` を基準に、`retrieve` 用の指示を追加。
- 追加指示の要点:
  - 「REPLを使ってdocumentsを分析し、上位k件を選ぶ」
  - 「必ず `FINAL("""{json}""")` で終了する」
  - 「scoreは0.0〜1.0の小数」
  - 「snippetは原文そのまま（span抽出用）」

---

## データフロー

1. HTTP `POST /v1/retrieve` を受信
2. `RetrieveRequest` を検証（documents空・query空など）
3. RLMループ開始:
   - LLMへ `system` + `user(query)` を送信
   - LLM出力を REPL で実行
   - `FINAL(...)` が出るまで繰り返し
4. `FINAL` から JSON を抽出 → `results` 変換
5. `snippet` から spans を算出し `RetrieveResponse` を構築

---

## エラーハンドリング方針

- LLMが `FINAL` を返さない → `max_iterations` で中断し警告
- JSONが壊れている → 1回だけ修正リクエスト（再整形）を送る
- doc_id が存在しない → 当該結果は捨てる（warnings に記録）
- score が不正 → クランプして警告

---

## テスト戦略

- `MockLLM` で RLMループを制御し、`retrieve` が正しい `results` を返すかをテスト
- `snippet` → `spans` 抽出の境界テスト
- `FINAL` 欠落 / JSON不正 / score不正 の異常系テスト
- 既存の `retrieve_api_tests.rs` は LLM版に合わせて書き換え
