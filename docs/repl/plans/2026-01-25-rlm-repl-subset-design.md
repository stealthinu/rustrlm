# RLM向け Python-REPL互換サブセット（Rust）設計（ドラフト）

本設計は、RLM論文/非公式実装の実測ログから「実際に使われたREPL表面」を抽出し、
Rustで実装する **安全・決定的・文字列処理中心** のPython-REPL互換サブセット仕様を定義する。

根拠ログ:
- 最終観測（union）: `docs/repl/final-observed-repl-surface.md`
- ベースライン実測: `extracted/runs/unofficial_tasks30_transcript.jsonl`
- import失敗タスク再実行（base64/zlib注入）: `extracted/runs/unofficial_importfail_rerun_transcript.jsonl`

## 0) 目標（成功条件）
- 文字列操作と正規表現を中心に、RLMワークフローで必要な最小サブセットを提供する
- 入力（ユーザー/LLM出力）は不正/悪意あり前提で、安全に失敗できる（panicしない、構造化エラー）
- 同一入力は同一出力（決定的）
- import / I/O / ネットワーク / サブプロセス / 動的ロード / 反射フックは禁止
- 表面積は allowlist で固定し、拡張は明示的に追加する

## 1) 非目標（スコープ外）
- Python互換を網羅しない（CPython互換が目的ではない）
- パッケージ/モジュールシステム（import）
- ファイル/OS/ネットワーク/時間依存（乱数・現在時刻等）

## 2) REPLプロトコル（外部I/F）
### 2.1 入力
- 1ステップの入力は「UTF-8テキスト（ASCII推奨）」のPythonコード断片
- 形式は「複数行のプログラム」を許容（`if`/`try` 等のブロックを含む）

### 2.2 実行モデル
- 各ステップでコードを実行し、stdout相当を収集して返す
- 実行環境（変数/関数）はステップ間で保持される（同一タスク内）
  - 少なくとも `context`, `query`, `re` は常時存在

### 2.3 終了（FINALプロトコル）
- REPL内に `FINAL` 関数は存在しない
- 上位システムが「応答テキスト」から以下を構文的に抽出して終了する:
  - `FINAL("...")`（文字列リテラルのみ）
  - `FINAL_VAR(name)`（nameは識別子）
- 実装注意:
  - `FINAL(var)` のような“式”はサポートしない（誤用はエラーとして扱う）

## 3) サブセット言語仕様（allowlist）
ここからがRust実装での“最終仕様”。
（観測されたものの union を基本にするが、危険/不要なものは落とす）

### 3.1 型（値のドメイン）
最低限:
- `str`, `bytes`
- `int`, `bool`, `None`
- `list[str]` / `list[bytes]` は必要なら許容（観測では添字/スライスが出た）

### 3.2 変数（事前注入）
必須:
- `context: str`
- `query: str`
- `re`: 正規表現（下記）

任意（採用する場合は仕様化して注入）:
- `base64`: `b64decode` を含む（import不要）
- `binascii`: `hexlify`（import不要）
- `zlib`: `decompress` のみ（出力上限つき）

### 3.3 許可する文（Statements）
allowlist:
- 代入: `name = expr`
- `if expr: ...` / `if not expr: ...` / `else`
- `try: ... except Exception: ...`（例外クラスは固定/限定）
- `for name in iterable: ...`（必要最小。`range/iter` のみ想定）
- `pass`
- 式文: `expr`（戻り値はstdoutに出ないが、必要なら最後の式reprを返す設計も可）

denylist（必須）:
- `import` / `from ... import ...`
- `with`, `while`, `class`, `lambda`, `yield`, `async/await`

### 3.4 許可する式（Expressions）
allowlist（観測/必要性ベース）:
- 呼び出し: `print(...)`, `len(x)`, `max(a,b)`
- 属性/メソッド:
  - `re.search`, `re.findall`
  - `m.group(n)`
  - `str.strip`, `str.lower`, `str.find`
  - `bytes.decode`
- 条件式: `a if cond else b`
- 演算:
  - `|`（正規表現フラグの結合）
  - `+`（文字列結合・bytes結合は要検討）
  - 比較: `==`, `!=`, `is`, `is not`
  - 単項: `not`, `-`（ただし `None` など型不一致は型エラー）
- 添字/スライス:
  - `x[i]`
  - `x[a:b]`（`str`/`bytes` のみに限定推奨）

denylist（必須）:
- `globals()`, `locals()`, `vars()`, `getattr()`（反射の入口になりやすい）
- `__*__` 名称へのアクセス（dunder）

### 3.5 `re`（正規表現）仕様
提供:
- `re.search(pattern: str, string: str, flags: int=0) -> Match|None`
- `re.findall(pattern: str, string: str, flags: int=0) -> list[str]`
- flags: `re.IGNORECASE`, `re.DOTALL` を最低限

Match:
- `group(n: int) -> str`

制限:
- パターン長/実行時間に上限（ReDoS対策）
  - 実装でタイムアウト/ステップ上限を持てないなら、パターン長/入力長を厳しめに制限する

### 3.6 base64/binascii/zlib（採用する場合）
**方針**: importを許可せず、globals注入で提供する。

base64:
- `base64.b64decode(s: str|bytes) -> bytes`

binascii:
- `binascii.hexlify(b: bytes) -> bytes`（必要なら `.decode('ascii')` で文字列化）

zlib（安全版）:
- `zlib.decompress(data: bytes, wbits: int=15) -> bytes`
- 出力サイズ上限: `MAX_ZLIB_OUTPUT_BYTES`（例: 1,000,000）
- 上限超過は `ValueError` 相当の決定的エラー

## 4) リソース制限（必須）
最低限:
- 最大コード長（例: 20_000 chars）
- 最大stdout長（例: 2_000 chars、超過は切り詰め＋注記）
- 最大実行ステップ/ASTノード数（例: 50_000）もしくは最大時間（例: 50ms-200ms）
- 最大データサイズ（bytes/string/listの合計）

## 5) エラー仕様（決定的）
エラーは構造化して返す（文字列だけにしない）:
- `SyntaxError`
- `NameError`（未定義名）
- `TypeError`
- `ValueError`
- `ResourceLimitExceeded`（出力/時間/サイズ/ステップ）
- `ForbiddenSyntax` / `ForbiddenName`（allowlist違反）

## 6) テスト計画（TDD）
この設計をRustで実装する前に、テストを“仕様”として固定する。

### 6.1 ゴールデン（実測から）
- `docs/repl/final-observed-repl-surface.md` の代表スニペットを最小ケースに落として、受理/拒否を決める
- base64/zlib採用時は `docs/research/eval/unofficial-importfail-rerun-summary.md` の代表パターンを追加

### 6.2 拒否テスト（安全）
- import, open, __import__, getattr/globals/locals, dunderアクセスは必ず拒否
- 巨大データ/無限ループ相当（forの過大range等）は制限で落とす

### 6.3 決定性
- 同入力を複数回実行して同一出力/同一エラーコードになること

## 7) 未決事項（ここで確定が必要）
- base64/binascii/zlib を最終サブセットに含めるか（含めるなら上限/提供APIを確定）
- `for`/`try`/`FunctionDef` を許可するか（観測では出たが、最小化の観点で落とす選択肢もある）
