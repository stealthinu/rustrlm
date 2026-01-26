# REPLプローブ結果（非公式ベースライン実測）

目的: RLM論文の「評価データ」由来の巨大コンテキストを入力にして、非公式実装 `ysz/recursive-llm` の
REPLExecutor が **どのPython要素を実際に通す/落とすか** を観測する（ベンチマークを解くのではなく互換性確認）。

## 実行方法（再現用）
S-NIAH（簡易版）を生成:
- `PYTHONPATH=vendor/python python3 tools/generate_s_niah.py --pg-json upstream/RULER/scripts/data/synthetic/json/PaulGrahamEssays.json --out-jsonl extracted/eval/s_niah.jsonl --num 50 --seed 42`

プローブを実行:
- `PYTHONPATH=vendor/python python3 tools/repl_probe_runner.py --out-jsonl extracted/runs/repl_probes.jsonl`

ログ:
- `extracted/runs/repl_probes.jsonl`

## 対象データ（代表サンプル）
各データセットから「先頭1件」を取り、サイズを抑えて（最大200k chars）プローブを実行:
- BrowseComp-Plus: `Tevatron/browsecomp-plus`（test parquet先頭行から docs を連結）
- LongBench-v2: `zai-org/LongBench-v2`（sub_domain == "Code repo QA" の先頭例）
- OOLONG（小サイズ派生）: `tonychenxyz/oolong-synth-1k-16k`（test.jsonl先頭行の prompt）
- S-NIAH（簡易生成）: `extracted/eval/s_niah.jsonl`（先頭行）

## プローブ（典型操作）
以下のような「文字列処理で頻出の操作」を少数のスニペットとして実行:
- `slice_head_len`: `context[:10000]` と `len`
- `splitlines_count`: `splitlines`
- `regex_findall_digits_no_import`: `re.findall(r"\d+", ...)`（import無し）
- `regex_split_whitespace_no_import`: `re.split(r"\s+", ...)`（import無し）
- `import_re_should_fail`: `import re`（失敗する前提の確認）
- `listcomp_context_in_expr_slice`: list comprehension の式側で `context[i:j]`
- `listcomp_context_in_iter_clause`: list comprehension の `for` 側で `str(context)` を走査
- `for_loop_append`: `for` + `if` + `list.append`
- `fstring_len`: f-string（JoinedStr）
- `any_generatorexp_digit`: generator expression + `any`
- `json_loads_no_import`: `json.loads(...)`（import無し; `json` は注入されている想定）

## 結果サマリ（2026-01-24 実測）
結果は 4データセット × 11プローブ = 44件。

- 成功: 36 / 44
- 失敗: 8 / 44（内訳は下記）

失敗したもの:
- `import_re_should_fail`: 4/4 失敗
  - 代表エラー: `Execution error: __import__ not found`
- `listcomp_context_in_expr_slice`: 4/4 失敗
  - 代表エラー: `Execution error: name 'context' is not defined`

成功したもの（全データセットで成功）:
- `slice_head_len`, `splitlines_count`, `regex_findall_digits_no_import`, `regex_split_whitespace_no_import`,
  `listcomp_context_in_iter_clause`, `for_loop_append`, `fstring_len`, `any_generatorexp_digit`, `json_loads_no_import`

## 観測から言えること（暫定）
- 非公式実装は `import` を遮断している一方で、`re` はグローバルに注入されているため import無しで利用できる。
- list comprehension の「式側」から外側スコープ（`context`）が参照できない挙動がある（RestrictedPython由来の可能性）。
