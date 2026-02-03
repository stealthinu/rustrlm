import re
from difflib import SequenceMatcher
from typing import Iterable, List, Sequence, Set, Tuple


DocTuple = Tuple[str, str]  # (doc_id, text)


def normalize_ws(text: str) -> str:
    return " ".join(text.lower().split())


def normalize_alnum(text: str) -> str:
    # Collapse to [a-z0-9 ] to be robust to markdown, quotes, punctuation, and escapes.
    return " ".join(re.sub(r"[^a-z0-9]+", " ", text.lower()).split())


def ground_truth_to_doc_ids(
    docs: Sequence[DocTuple],
    ground_truth_contexts: Sequence[str],
) -> Set[str]:
    """
    Map ground-truth context strings to corpus doc_ids by substring search.

    Strategy:
    1) whitespace-normalized substring match (closest to original script)
    2) alnum-normalized substring match (robust to markdown/escaping)
    """
    gts_ws = [normalize_ws(gt) for gt in ground_truth_contexts if normalize_ws(gt)]
    gts_alnum = [normalize_alnum(gt) for gt in ground_truth_contexts if normalize_alnum(gt)]

    out: Set[str] = set()

    docs_ws: List[Tuple[str, str]] = [(doc_id, normalize_ws(text)) for doc_id, text in docs]
    docs_alnum: List[Tuple[str, str]] = [(doc_id, normalize_alnum(text)) for doc_id, text in docs]

    for gt in gts_ws:
        for doc_id, d in docs_ws:
            if gt in d:
                out.add(doc_id)

    if out:
        return out

    for gt in gts_alnum:
        for doc_id, d in docs_alnum:
            if gt in d:
                out.add(doc_id)

    return out


def hit_doc_id(retrieved_doc_ids: Iterable[str], ground_truth_doc_ids: Set[str]) -> bool:
    for doc_id in retrieved_doc_ids:
        if doc_id in ground_truth_doc_ids:
            return True
    return False


def hit_text_ws_substring(retrieved_texts: Iterable[str], ground_truth_contexts: Sequence[str]) -> bool:
    retrieved_norm = [normalize_ws(t) for t in retrieved_texts]
    for gt in ground_truth_contexts:
        gt_norm = normalize_ws(gt)
        if not gt_norm:
            continue
        for r in retrieved_norm:
            if gt_norm in r:
                return True
    return False


def hit_text_alnum_substring(retrieved_texts: Iterable[str], ground_truth_contexts: Sequence[str]) -> bool:
    retrieved_norm = [normalize_alnum(t) for t in retrieved_texts]
    for gt in ground_truth_contexts:
        gt_norm = normalize_alnum(gt)
        if not gt_norm:
            continue
        for r in retrieved_norm:
            if gt_norm in r:
                return True
    return False


def hit_text_relaxed(
    retrieved_texts: Iterable[str],
    ground_truth_contexts: Sequence[str],
    *,
    min_ratio: float = 0.72,
) -> bool:
    """
    "Relaxed" textual hit:
    - Alnum-substring (robust to markdown/punctuation)
    - Otherwise, alnum-normalized SequenceMatcher ratio >= min_ratio

    This is intended for quick sanity checks and debugging, not for paper-quality evals.
    """
    if hit_text_alnum_substring(retrieved_texts, ground_truth_contexts):
        return True

    retrieved_norm = [normalize_alnum(t) for t in retrieved_texts]
    gt_norms = [normalize_alnum(gt) for gt in ground_truth_contexts]
    gt_norms = [g for g in gt_norms if g]

    for g in gt_norms:
        for r in retrieved_norm:
            if not r:
                continue
            if SequenceMatcher(None, g, r).ratio() >= min_ratio:
                return True

    return False
