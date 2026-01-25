# 論文アーティファクト抽出（arXiv HTML）

目的: 論文本文/付録に埋め込まれている「プロンプト/軌跡/例示コード」を機械的に抽出し、
そこに登場する REPL（```repl）コードから「どんなPython機能が使われているか」を把握する。

## 入力（ローカル保存）
- `upstream/paper/rlm-2512.24601v1.html`
  - `https://arxiv.org/html/2512.24601v1` をダウンロードしたもの

## 抽出1: arXiv HTML 内の listing（data:text/plain;base64）
arXiv HTML には、付録のプロンプト等が `data:text/plain;base64,...` として埋め込まれている。
これをデコードしてテキスト化した。

- 抽出スクリプト: `tools/extract_arxiv_html_listings.py`
- 出力:
  - `extracted/paper/listings/*.txt`
  - `extracted/paper/listings/manifest.json`
- 現状の抽出数: 9 listing

## 抽出2: listing から ```repl ブロックを抽出
公式実装（`alexzhang13/rlm`）の `find_code_blocks` と同一の正規表現で、
` ```repl ... ``` ` を抽出してコード断片集（コーパス）にした。

- 抽出スクリプト: `tools/extract_repl_code_blocks.py`
- 出力:
  - `extracted/paper/repl_blocks/*.py`
  - `extracted/paper/repl_blocks/manifest.json`
- 現状の抽出数: 6 repl blocks

## 静的解析: ASTベースの機能カウント（参考）
抽出した repl blocks を `ast.parse` で解析し、使われている構文/呼び出しを数えた。
（論文中の「例示コード」には、引用符のエスケープ不足などで Python としては不正なものが混じるため、
その場合は `parse_error` として記録し、集計からは除外している。）

- 解析スクリプト: `tools/analyze_repl_blocks_ast.py`
- 出力: `extracted/paper/repl_ast_features.json`
- 現状:
  - parse成功: 5/6
  - parse失敗: 1/6 (`A4.SS1.p2.2__repl_002.py`)

### ざっくり出現傾向（現時点）
解析できた範囲では、以下が目立つ:
- ループ/制御: `for`, `if/else`
- 文字列: f-string, 連結, `join`
- コレクション: list作成/append, `enumerate`, `range`, `len`
- インデックス/スライス: `context[:N]`, `context[i:j]` 系
- モジュール: `import re` / `re.split`（= 正規表現）
- RLM固有: `llm_query(...)`

この結果を踏まえ、Rust実装で「どこまでをサブセットとして支えるか」を仕様化していく。

