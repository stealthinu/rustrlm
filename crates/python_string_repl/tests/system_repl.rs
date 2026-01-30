use python_string_repl::repl::{ExecRequest, ReplConfig, ReplEngine};

fn run(code: &str, context: &str, query: &str) -> (bool, String, Option<String>) {
    let engine = ReplEngine::new(ReplConfig::default());
    let resp = engine.exec(ExecRequest {
        context: context.to_string(),
        query: query.to_string(),
        code: code.to_string(),
        max_output_chars: None,
        state: None,
    });
    (resp.ok, resp.output, resp.error)
}

#[test]
fn sys_no_code_to_execute() {
    let (ok, out, err) = run("   \n", "", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "No code to execute");
}

#[test]
fn sys_echo_last_expr_name() {
    let (ok, out, err) = run("query", "", "  hello  ");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "hello");
}

#[test]
fn sys_import_is_ignored_and_preprovided_modules_work() {
    let code = r#"
import re
m = re.search(r"abc", context)
print(m.group(0) if m else "no")
"#;
    let (ok, out, err) = run(code, "xxabczz", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "abc");
}

#[test]
fn sys_import_as_alias_binds_name() {
    let code = r#"
import re as r
m = r.search(r"abc", context)
print(m.group(0) if m else "no")
"#;
    let (ok, out, err) = run(code, "xxabczz", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "abc");
}

#[test]
fn sys_from_import_binds_symbols() {
    let code = r#"
from re import search, IGNORECASE, DOTALL
m = search(r"key-1.*?([0-9]+)", context, flags=IGNORECASE|DOTALL)
print(m.group(1) if m else "")
"#;
    let (ok, out, err) = run(code, "KEY-1 blah 42 end", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "42");
}

#[test]
fn sys_import_multiple_modules() {
    let code = r#"
import base64, zlib, binascii
raw = base64.b64decode(query.strip())
out = zlib.decompress(raw)
print(binascii.hexlify(out).decode("ascii"))
"#;
    // base64(zlib.compress(b"abc"))
    let (ok, out, err) = run(code, "", "eJxLTEoGAAJNASc=");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "616263");
}

#[test]
fn sys_from_import_zlib_decompress_and_constant() {
    let code = r#"
from zlib import decompress, MAX_WBITS
raw = base64.b64decode(query.strip())
outb = decompress(raw, MAX_WBITS)
print(outb.decode("utf-8"))
"#;
    // base64(zlib.compress(b"hello"))
    let (ok, out, err) = run(code, "", "eJzLSM3JyQcABiwCFQ==");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "hello");
}

#[test]
fn sys_regex_search_group_with_flags() {
    let code = r#"
m = re.search(r'key-31.*?(\d+)', context, flags=re.IGNORECASE|re.DOTALL)
ans = m.group(1) if m else ""
print(ans)
"#;
    let (ok, out, err) = run(code, "blah KEY-31 ... 345938494 end", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "345938494");
}

#[test]
fn sys_regex_findall_and_len() {
    let code = r#"
errors = re.findall(r'ERROR', context)
print(len(errors))
"#;
    let (ok, out, err) = run(code, "ok ERROR a ERROR b", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "2");
}

#[test]
fn sys_json_loads_dict_and_list_indexing() {
    let code = r#"
obj = json.loads('{"a": 1, "b": [2, 3]}')
print(obj["a"], obj["b"][0])
"#;
    let (ok, out, err) = run(code, "", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "1 2");
}

#[test]
fn sys_import_json_and_from_import_loads() {
    let code = r#"
import json
from json import loads
obj = loads('{"a": 1}')
print(json.loads('{"a": 2}')["a"], obj["a"])
"#;
    let (ok, out, err) = run(code, "", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "2 1");
}

#[test]
fn sys_string_strip_lower_find() {
    let code = r#"
s = query.strip()
idx = context.lower().find(s.lower())
print(idx)
"#;
    let (ok, out, err) = run(code, "Hello WORLD", "  world  ");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "6");
}

#[test]
fn sys_slice_context_prefix() {
    let code = r#"
print(context[:5])
"#;
    let (ok, out, err) = run(code, "Hello WORLD", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "Hello");
}

#[test]
fn sys_list_slicing() {
    let code = r#"
xs = [1, 2, 3, 4]
print(xs[:2][1])
print(xs[1:3][0])
"#;
    let (ok, out, err) = run(code, "", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "2\n2");
}

#[test]
fn sys_base64_b64decode_then_decode_utf8() {
    let code = r#"
raw = base64.b64decode(query.strip())
print(raw.decode("utf-8"))
"#;
    let (ok, out, err) = run(code, "", "aGVsbG8=");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "hello");
}

#[test]
fn sys_zlib_decompress_then_decode() {
    // "hello" compressed with zlib and base64-encoded.
    let code = r#"
raw = base64.b64decode(query.strip())
outb = zlib.decompress(raw)
print(outb.decode("utf-8"))
"#;
    // Precomputed: base64(zlib.compress(b"hello"))
    let (ok, out, err) = run(code, "", "eJzLSM3JyQcABiwCFQ==");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "hello");
}

#[test]
fn sys_binascii_hexlify() {
    let code = r#"
print(binascii.hexlify(b"abc").decode("utf-8"))
"#;
    let (ok, out, err) = run(code, "", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "616263");
}

#[test]
fn sys_try_except_decode_fallback() {
    let code = r#"
raw = b"\xff"
try:
    s = raw.decode("utf-8")
except Exception:
    s = raw.decode("latin-1")
print(s)
"#;
    let (ok, out, err) = run(code, "", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "\u{00ff}");
}

#[test]
fn sys_for_loop_build_string() {
    let code = r#"
def f(x):
    return x.strip()
out = ""
for ch in f(query):
    if ch != "_":
        out = out + ch
print(out)
"#;
    let (ok, out, err) = run(code, "", "__a_b__");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "ab");
}

#[test]
fn sys_list_comprehension_basic_and_if_filter() {
    let code = r#"
xs = [1, 2, 3, 4]
ys = [x for x in xs if x != 2]
print(len(ys), ys[0], ys[1], ys[2])
"#;
    let (ok, out, err) = run(code, "", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "3 1 3 4");
}

#[test]
fn sys_type_is_not_available_and_error_mentions_subset() {
    let code = r#"
print(type(query))
"#;
    let (ok, _out, err) = run(code, "", "hello");
    assert!(!ok);
    let msg = err.unwrap_or_default();
    assert!(msg.contains("name error: type"), "unexpected err: {msg}");
    assert!(
        msg.to_lowercase().contains("restricted python subset"),
        "missing subset hint: {msg}"
    );
}

#[test]
fn sys_dict_int_indexing_is_supported_for_llm_robustness() {
    // Dict literals are not supported in this subset, so use json.loads.
    let code = r#"
d = json.loads('{"a": 1, "b": 2}')
print(d[0], d[1])
"#;
    let (ok, out, err) = run(code, "", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "1 2");
}

#[test]
fn sys_in_compare_and_boolop() {
    let code = r#"
t = context.lower()
print(("alien" in t) and ("truth" in t))
print(("missing" in t) or ("alien" in t))
"#;
    let (ok, out, err) = run(code, "Alien Truth", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "True\nTrue");
}

#[test]
fn sys_range_basic() {
    let code = r#"
xs = range(3)
print(len(xs), xs[0], xs[1], xs[2])
"#;
    let (ok, out, err) = run(code, "", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "3 0 1 2");
}

#[test]
fn sys_dict_literal_with_str_keys() {
    let code = r#"
d = {"a": 1, "b": 2}
print(d["a"], d[0])
"#;
    let (ok, out, err) = run(code, "", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "1 1");
}

#[test]
fn sys_list_append_and_dict_get_and_str_replace_split_startswith_and_json_dumps() {
    let code = r#"
xs = []
xs.append("a")
xs.append("b")
print(len(xs), xs[0], xs[1])

d = {"x": 1}
print(d.get("x"), d.get("y", 9))

s = "hello world"
print(s.replace("world", "you"))
print(s.split()[0])
print(s.startswith("he"))

print(json.dumps({"a": 1}))
"#;
    let (ok, out, err) = run(code, "", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "2 a b\n1 9\nhello you\nhello\nTrue\n{\"a\":1}");
}

#[test]
fn sys_rank_documents_helper() {
    let code = r#"
docs = json.loads('[{"id":"d1","text":"alpha beta","metadata":null},{"id":"d2","text":"the quick brown fox","metadata":null}]')
hits = rank_documents(docs, query, 2)
print(len(hits), hits[0]["doc_id"])
"#;
    let (ok, out, err) = run(code, "", "brown fox");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "1 d2");
}

#[test]
fn sys_zlib_output_limit_is_enforced() {
    // Decompressing this should exceed the default cap (1_000_000) if not enforced.
    // We'll use a smaller cap via request max_output_chars only affects stdout, so we rely on engine default cap.
    // Constructed payload: zlib-compressed 1_100_000 'a' bytes, base64-encoded (precomputed).
    let code = r#"
raw = base64.b64decode(query.strip())
outb = zlib.decompress(raw)
print(len(outb))
"#;

    // This is a short "bomb" for testing: 1_100_000 'a' bytes compressed with zlib, then base64.
    // Generated offline for determinism; if zlib cap works, this should error.
    let payload = include_str!("zlib_bomb_1100000_a.b64");
    let (ok, _out, err) = run(code, "", payload.trim());
    assert!(!ok);
    let msg = err.unwrap_or_default();
    assert!(
        msg.contains("resource limit") || msg.contains("ValueError") || msg.contains("exceeds"),
        "unexpected err: {msg}"
    );
}

#[test]
fn sys_bytes_decode_errors_replace_is_allowed() {
    let code = r#"
raw = b"\xff"
print(raw.decode("utf-8", errors="replace"))
"#;
    let (ok, out, err) = run(code, "", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "\u{FFFD}");
}

#[test]
fn sys_zlib_max_wbits_constant() {
    let code = r#"
print(zlib.MAX_WBITS)
"#;
    let (ok, out, err) = run(code, "", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "15");
}

#[test]
fn sys_break_is_supported() {
    let code = r#"
out = ""
for ch in query.strip():
    out = out + ch
    if len(out) == 2:
        break
print(out)
"#;
    let (ok, out, err) = run(code, "", "abcd");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "ab");
}

#[test]
fn sys_percent_formatting() {
    let code = r#"
raw = b"abc"
print("len(raw)=%d" % len(raw))
print("head=%s" % binascii.hexlify(raw).decode("ascii"))
"#;
    let (ok, out, err) = run(code, "", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "len(raw)=3\nhead=616263");
}

#[test]
fn sys_re_z_anchor_is_supported() {
    let code = r#"
m = re.search(r"abc\Z", context)
print(m.group(0) if m else "no")
"#;
    let (ok, out, err) = run(code, "abc", "");
    assert!(ok, "err={err:?}");
    assert_eq!(out, "abc");
}
