# 参照ソース（RLM）

## 論文
- Recursive Language Models (arXiv:2512.24601)
  - 抄録ページ: https://arxiv.org/abs/2512.24601
  - HTML: https://arxiv.org/html/2512.24601v1

## ブログ
- Recursive Language Models | Alex L. Zhang
  - https://alexzhang13.github.io/blog/2025/rlm/

## 実装
### 公式/参照実装
- alexzhang13/rlm
  - https://github.com/alexzhang13/rlm
- alexzhang13/rlm-minimal
  - https://github.com/alexzhang13/rlm-minimal

### 非公式実装
- ysz/recursive-llm
  - https://github.com/ysz/recursive-llm

## データセット/評価（入手先）
- BrowseComp-Plus
  - https://texttron.github.io/BrowseComp-Plus/
  - HF: `Tevatron/browsecomp-plus`, `Tevatron/browsecomp-plus-corpus`
- OOLONG
  - HF: `oolongbench/oolong-synth`, `oolongbench/oolong-real`
  - 派生（小サイズ）: `tonychenxyz/oolong-synth-1k-16k`, `tonychenxyz/oolong-synth-32k-128k`
- LongBench-v2
  - https://github.com/THUDM/LongBench
  - https://longbench2.github.io
  - HF: `zai-org/LongBench-v2`
- RULER (S-NIAH 元)
  - https://github.com/hsiehjackson/RULER

## ローカルで確認済みの固定リビジョン
- alexzhang13/rlm: 6eb5f6be87eec214bd6b75b23f8dff60d9242f6c
- ysz/recursive-llm: 2fb46cc59e64cddc0768ce0bf428138dab3016eb
- hsiehjackson/RULER: ab17b7853df4e0a30b78cd5d2b463ac7dff6ee13
