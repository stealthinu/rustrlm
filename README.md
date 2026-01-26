# RustRLM

Rustで実装した **Recursive Language Models (RLM)** ランナーと、RLMが安全に「コンテキストを変数として保持して処理」するための
**軽量・決定的なPython REPLサブセット**（内包ライブラリ）を同一リポジトリで開発するプロジェクトです。

特徴:
- RLM本体（Rust） + Python REPLサブセット（Rust, allowlist方式）を同梱
- REPLは文字列処理中心・安全第一（import/eval/exec/IO/ネットワーク等は原則不可）
- 実験ログ/トランスクリプトを保存して、挙動を“観測→仕様化→TDD”で固める

## 重要: 秘密情報
- APIキーは **`.env` に保存** し、**絶対にgitにコミットしない**（`.gitignore` 済み）。
- チャットやIssueにキーを貼らないでください（漏洩扱い）。

`.env` の例は `.env.example` を参照してください。

## 構成（Cargo workspace）
- `crates/python_string_repl`: Python REPLサブセット（ライブラリ）
- `crates/python_string_repl_cli`: REPL CLI（bin名 `python_string_repl`。stdin JSON -> stdout JSON）
- `crates/rlm_runner`: Rust RLMランナー（OpenAI API固定。開発中）

## Rust: ビルド/テスト
この環境では `~/.cargo` が書き込み不可のため、ローカルの `CARGO_HOME` を使います:
```bash
mkdir -p .cargo-home
CARGO_HOME=$PWD/.cargo-home cargo test
```

REPL CLI をビルド:
```bash
CARGO_HOME=$PWD/.cargo-home cargo build -p python_string_repl_cli
```

## REPL CLI 実行（JSON in/out）
stdin: JSON 1個（`context/query/code`）
```bash
./target/debug/python_string_repl <<'JSON'
{"context":"Hello WORLD","query":"  world  ","code":"s = query.strip()\nidx = context.lower().find(s.lower())\nprint(idx)\n"}
JSON
```
stdout: JSON 1個（`ok/output/error/state`）

## Python依存（調査/評価用ツール）
この環境では `venv` が使えない前提のため、依存は `vendor/python` に入れて `PYTHONPATH` で参照します。

依存を入れる:
```bash
python3 -m pip install -r requirements-vendor.txt --target vendor/python
```

実行例:
```bash
PYTHONPATH=vendor/python python3 tools/repl_probe_runner.py --out-jsonl extracted/runs/repl_probes.jsonl
```

## 大きいデータ/クローンの扱い
`upstream/`（外部repoやデータセット）と `extracted/`（抽出物/ログ）はローカルキャッシュとして clarifying に使います。
これらは容量が大きくなりやすいため、デフォルトで `.gitignore` 対象です。

進捗や根拠は `docs/` と `CONTINUITY.md` に集約します。
