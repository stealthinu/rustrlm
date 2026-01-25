# python-string-repl

安全・決定的・文字列操作に特化した「Python-REPL互換サブセット」を Rust で実装するための調査/作業用リポジトリです。

## 重要: 秘密情報
- APIキーは **`.env` に保存** し、**絶対にgitにコミットしない**（`.gitignore` 済み）。
- チャットやIssueにキーを貼らないでください（漏洩扱い）。

`.env` の例は `.env.example` を参照してください。

## Python依存（このリポジトリのツール用）
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

## Rust REPLサブセット（実装）
このリポジトリは Rust で「安全・決定的・文字列操作中心」の Python-REPL互換サブセットを実装しています。

### ビルド/テスト
この環境では `~/.cargo` が書き込み不可のため、ローカルの `CARGO_HOME` を使います:
```bash
mkdir -p .cargo-home
CARGO_HOME=$PWD/.cargo-home cargo test
```

### CLI実行（JSON in/out）
stdin: JSON 1個（`context/query/code`）
```bash
CARGO_HOME=$PWD/.cargo-home cargo run -q <<'JSON'
{"context":"Hello WORLD","query":"  world  ","code":"s = query.strip()\nidx = context.lower().find(s.lower())\nprint(idx)\n"}
JSON
```
stdout: JSON 1個（`ok/output/error`）
