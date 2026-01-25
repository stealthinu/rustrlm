# 非公式ベースライン実行結果（一次抽出）

目的: 非公式実装 `ysz/recursive-llm` の REPL 実行器を、論文HTMLから抽出した ```repl 例示コードに対して実際に動かし、
受理/拒否される構文や、出力・エラーの形を観測する。

## 実行対象
- ベースライン: `ysz/recursive-llm` @ `2fb46cc59e64cddc0768ce0bf428138dab3016eb`
- コーパス: `extracted/paper/repl_blocks/*.py`（論文HTML埋め込み listing から抽出した repl ブロック）

## 実行方法（ローカル）
- 依存（RestrictedPython など）を `vendor/python` にインストール
- 実行:
  - `PYTHONPATH=vendor/python python3 tools/run_unofficial_repl_on_corpus.py --corpus-manifest extracted/paper/repl_blocks/manifest.json --out-jsonl extracted/runs/unofficial_repl_on_paper_corpus.jsonl`

## 観測（重要）
- `import re` を含むスニペットは失敗した:
  - エラー: `Execution error: __import__ not found`
  - つまり、このベースラインでは import 文は基本的に禁止されている（RestrictedPython側）
  - ただし `re` 自体は「globalsに注入」されており、`re.findall(...)` のように import なしで使うのは成功した
- 論文コーパス内に「Pythonとして不正な例示コード」が混入していた:
  - 例: `A4.SS1.p2.2__repl_002.py` は引用符のエスケープ不足で SyntaxError になった
- 論文コーパス中の f-string 例は `{{...}}` が多く、実行するとプレースホルダが展開されない
  - これは「実際にモデルへ渡すプロンプト」ではなく「説明用の例」になっている可能性が高い
  - 仕様化はこの点を分離して考える（= 例示の書き方 vs 実運用のコード）

## 出力ファイル
- 実行ログ（JSONL）: `extracted/runs/unofficial_repl_on_paper_corpus.jsonl`

