# RLM論文の評価スイート（2512.24601v1）

このメモは、RLM論文（arXiv:2512.24601v1）本文から読み取れる「評価に使ったタスク/設定」を整理し、
評価データ取得とREPL仕様抽出の入口にするためのものです。

## 論文で言及される主タスク（§2.1）
- S-NIAH
  - RULERの single needle-in-a-haystack を元にした「50タスク」のセット
- BrowseComp-Plus（1K documents）
  - BrowseComp-Plus の「100K documents のオフラインコーパス」から、各タスクに 1000 docs を与える設定
  - 評価は 150 タスク（ランダムサンプル）と記述
- OOLONG
  - OOLONG の `trec_coarse` split を使用（50 tasks と記述）
- OOLONG-Pairs
  - OOLONG `trec_coarse` を元に「20個の追加クエリ」を作った、と記述
  - さらに Appendix E.1 に “全クエリを明示” と記述
- LongBench-v2 CodeQA
  - LongBench-v2 の CodeQA split（多肢選択のコード理解）を使用

## 論文内に埋め込みで提供される評価アーティファクト
### OOLONG-Pairs のクエリ（Appendix E.1）
RLM論文の Appendix E.1（A5.SS1）には、Task 1〜Task 20 の “クエリ文” が本文中に埋め込まれています。

- 抽出スクリプト: `tools/extract_rlm_paper_eval_artifacts.py`
- 抽出結果: `extracted/paper/eval_artifacts.json`
  - `oolong_pairs_tasks[*].prompt` に Task 文が入る

注意: これらの Task 文は「In the above data...」という形式で、入力データ（= OOLONG側のデータセット本体）
が別途必要です。よって、Oolong のデータ入手は必須です。

