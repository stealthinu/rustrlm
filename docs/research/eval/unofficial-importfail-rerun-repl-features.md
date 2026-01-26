# 非公式ベースライン実測: REPL使用機能サマリ（30タスク）
- transcript: `extracted/runs/unofficial_importfail_rerun_transcript.jsonl`
- unique REPL snippets: 30
- ok_by_dataset: {'browsecomp_plus': 14, 'longbench_v2_codeqa': 1}
- fail_by_dataset: {'browsecomp_plus': 10, 'longbench_v2_codeqa': 4, 's_niah': 1}

## AST特徴（成功したREPLスニペットのみ）

### node_types (top)
- Load: 236
- Name: 207
- Call: 188
- Constant: 87
- Attribute: 50
- Expr: 39
- Store: 37
- Assign: 34
- BinOp: 16
- Module: 15
- Subscript: 12
- Slice: 11
- Try: 10
- ExceptHandler: 10
- Compare: 8
- keyword: 8
- If: 6
- Is: 6
- Sub: 5
- Mod: 5

### call_names (top)
- print: 39
- len: 6
- try_decompress: 1
- func: 1

### attr_calls (top)
- re.findall: 7
- query.strip: 6
- base64.b64decode: 6
- raw.decode: 5
- decode: 4
- outb.decode: 4
- zlib.decompress: 3
- binascii.hexlify: 3
- decomp.decode: 2
- b.decode: 2
- re.search: 2
- context.find: 2
- m.group: 1

## 上位REPLスニペット（頻出; 生コード）

### 1c1948e6e8e8889f (count=1 ok=0 datasets=['browsecomp_plus'])
```python
data_b64 = query.strip()

# Try common decode/decompress patterns: base64 -> zlib (various wbits), else raw bytes as text
raw = base64.b64decode(data_b64)

def try_decompress(b):
    for wbits in (zlib.MAX_WBITS, -zlib.MAX_WBITS, zlib.MAX_WBITS | 16):
        try:
            return zlib.decompress(b, wbits)
        except Exception:
            pass
    return None

decomp = try_decompress(raw)
if decomp is None:
    # Fallback: treat as utf-8/latin1
    try:
        out = raw.decode("utf-8")
    except Exception:
        out = raw.decode("latin1", errors="replace")
else:
    try:
        out = decomp.decode("utf-8")
    except Exception:
        out = decomp.decode("latin1", errors="replace")

FINAL(out)
```

### e130f1b6b14720e1 (count=1 ok=0 datasets=['browsecomp_plus'])
```python
data_b64 = query.strip()
raw = base64.b64decode(data_b64)

def try_decompress(b):
    candidates = []
    mw = getattr(zlib, "MAX_WBITS", 15)
    if mw is None:
        mw = 15
    candidates = [mw, -mw, mw | 16]
    for wbits in candidates:
        try:
            return zlib.decompress(b, wbits)
        except Exception:
            pass
    return None

decomp = try_decompress(raw)
if decomp is None:
    try:
        out = raw.decode("utf-8")
    except Exception:
        out = raw.decode("latin1", errors="replace")
else:
    try:
        out = decomp.decode("utf-8")
    except Exception:
        out = decomp.decode("latin1", errors="replace")

FINAL(out)
```

### 66c7c282e53a6e73 (count=1 ok=0 datasets=['browsecomp_plus'])
```python
data_b64 = query.strip()
raw = base64.b64decode(data_b64)

def try_decompress(b):
    mw = zlib.MAX_WBITS
    if mw is None:
        mw = 15
    for wbits in (mw, 0 - mw, mw | 16):
        try:
            return zlib.decompress(b, wbits)
        except Exception:
            pass
    return None

decomp = try_decompress(raw)
if decomp is None:
    try:
        out = raw.decode("utf-8")
    except Exception:
        out = raw.decode("latin1", errors="replace")
else:
    try:
        out = decomp.decode("utf-8")
    except Exception:
        out = decomp.decode("latin1", errors="replace")

FINAL(out)
```

### a33216fc74178ae7 (count=1 ok=1 datasets=['browsecomp_plus'])
```python
data_b64 = query.strip()
raw = base64.b64decode(data_b64)

def try_decompress(b):
    mw = zlib.MAX_WBITS
    if mw is None:
        mw = 15
    for wbits in (mw, 0 - mw, mw | 16):
        try:
            return zlib.decompress(b, wbits)
        except Exception:
            pass
    return None

decomp = try_decompress(raw)
if decomp is None:
    try:
        out = raw.decode("utf-8")
    except Exception:
        out = raw.decode("latin1", errors="replace")
else:
    try:
        out = decomp.decode("utf-8")
    except Exception:
        out = decomp.decode("latin1", errors="replace")

print(out)
```

### 7237134cf8626d60 (count=1 ok=1 datasets=['browsecomp_plus'])
```python
print(base64.b64decode(query.strip()).decode("latin1", errors="replace"))
```

### 701f7e255f849d4f (count=1 ok=1 datasets=['browsecomp_plus'])
```python
print(query)
```

### 526539c3d4ceb400 (count=1 ok=1 datasets=['browsecomp_plus'])
```python
raw = base64.b64decode(query.strip())
print(raw.decode("latin1", errors="replace"))
```

### 37658cc1210b74db (count=1 ok=1 datasets=['browsecomp_plus'])
```python
raw = base64.b64decode(query.strip())

# Try zlib/gzip/raw-deflate
outb = None
mw = zlib.MAX_WBITS
if mw is None:
    mw = 15
for wbits in (mw, 0 - mw, mw | 16):
    try:
        outb = zlib.decompress(raw, wbits)
        break
    except Exception:
        pass

if outb is None:
    # If not compressed, just show bytes
    try:
        print(raw.decode("utf-8"))
    except Exception:
        print(raw.decode("latin1", errors="replace"))
else:
    try:
        print(outb.decode("utf-8"))
    except Exception:
        print(outb.decode("latin1", errors="replace"))
```

### 294f0add71cd2a35 (count=1 ok=1 datasets=['browsecomp_plus'])
```python
raw = base64.b64decode(query.strip())
print("len(raw)=%d" % len(raw))
print("head(hex)=%s" % binascii.hexlify(raw[:32]).decode("ascii"))
print("tail(hex)=%s" % binascii.hexlify(raw[-32:]).decode("ascii"))
```

### 5fa11b7f0a89189b (count=1 ok=1 datasets=['browsecomp_plus'])
```python
raw = base64.b64decode(query.strip())

# Try common container formats by magic bytes
def show_try(name, func):
    try:
        b = func()
        if b is None:
            return
        try:
            s = b.decode("utf-8")
        except Exception:
            s = b.decode("latin1", errors="replace")
        print(name + " ok, len=%d" % len(b))
        print(s)
    except Exception as e:
        pass

# zlib/gzip/raw deflate
mw = zlib.MAX_WBITS
if mw is None:
    mw = 15
for wbits in (mw, 0 - mw, mw | 16):
    try:
        outb = zlib.decompress(raw, wbits)
        print("zlib wbits=%d ok, len=%d" % (wbits, len(outb)))
        try:
            print(outb.decode("utf-8"))
        except Exception:
            print(outb.decode("latin1", errors="replace"))
        raise SystemExit
    except Exception:
        pass

# If not compressed, maybe XOR/other; print as hex for full inspection
print(binascii.hexlify(raw).decode("ascii"))
```

### 6902afcfcdbd8723 (count=1 ok=1 datasets=['browsecomp_plus'])
```python
print("Not zlib/gzip/deflate. Raw bytes (hex) already shown; no further decoding possible without additional scheme/key.")
```

### 16a04655fabc7e19 (count=1 ok=1 datasets=['browsecomp_plus'])
```python
print("Please provide the encoding/encryption method (e.g., AES key/IV, XOR key, or what produced this base64). As-is it is just 283 bytes of non-text binary data and does not match zlib/gzip/deflate.")
```

### 2eac98d3ba8fd15b (count=1 ok=1 datasets=['browsecomp_plus'])
```python
print("I cannot decode it further without the method/key. Share how it was generated (algorithm, key/IV/salt, mode, padding, and whether it was compressed) and I can produce the plaintext.")
```

### 88fd97e51d8a2a7b (count=1 ok=1 datasets=['browsecomp_plus'])
```python
print("Understood. Provide: (1) algorithm (AES/ChaCha20/etc), (2) key (and format), (3) IV/nonce, (4) mode (CBC/GCM/etc), (5) any salt/KDF params, (6) whether data was compressed before encrypting. Then I will decode.")
```

### 3113910435791bb7 (count=1 ok=1 datasets=['browsecomp_plus'])
```python
print("Waiting for the details listed. Without them, the base64 decodes to opaque binary and cannot be reliably interpreted.")
```

### ff5c6ed78c5f96f4 (count=1 ok=1 datasets=['browsecomp_plus'])
```python
print("No further action possible until you provide the encryption/encoding details (algorithm, key, IV/nonce, mode, KDF/salt, compression).")
```

### 70631c07ac85e338 (count=1 ok=1 datasets=['browsecomp_plus'])
```python
print("OK.")
```

### 284f05220963c660 (count=1 ok=0 datasets=['browsecomp_plus'])
```python
data_b64 = query.strip()

# Try base64 decode then zlib decompress (common for such blobs)
raw = None
try:
    raw = base64.b64decode(data_b64, validate=False)
except Exception:
    raw = base64.b64decode(data_b64 + "==", validate=False)

out = None
errs = []

# Try zlib with common wbits
for wbits in (zlib.MAX_WBITS, -zlib.MAX_WBITS, zlib.MAX_WBITS | 16):
    try:
        out = zlib.decompress(raw, wbits)
        break
    except Exception as e:
        errs.append(str(e))

if out is None:
    # If not zlib, just show decoded bytes as best-effort text
    try:
        txt = raw.decode("utf-8", errors="replace")
    except Exception:
        txt = str(raw)
    FINAL(txt)
else:
    try:
        txt = out.decode("utf-8", errors="replace")
    except Exception:
        txt = str(out)
    FINAL(txt)
```

### 4ae12b014efda42b (count=1 ok=0 datasets=['browsecomp_plus'])
```python
data_b64 = query.strip()

# base64 decode
try:
    raw = base64.b64decode(data_b64, validate=False)
except Exception:
    raw = base64.b64decode(data_b64 + "==", validate=False)

# zlib wbits values without relying on zlib.MAX_WBITS (may be None in this REPL)
wbits_list = (15, -15, 31)

out = None
for wbits in wbits_list:
    try:
        out = zlib.decompress(raw, wbits)
        break
    except Exception:
        pass

if out is None:
    FINAL(raw.decode("utf-8", errors="replace"))
else:
    FINAL(out.decode("utf-8", errors="replace"))
```

### 5627efed4c7d3372 (count=1 ok=0 datasets=['browsecomp_plus'])
```python
s = query.strip()

# Try base64 decode then zlib decompress (common pattern)
data = None
try:
    data = base64.b64decode(s + "===")
except Exception:
    data = None

out = None
if data is not None:
    for wbits in (zlib.MAX_WBITS, -zlib.MAX_WBITS, zlib.MAX_WBITS | 16):
        try:
            out = zlib.decompress(data, wbits)
            break
        except Exception:
            continue

if out is None:
    # If not compressed, treat decoded bytes as output
    out = data if data is not None else b""

# Try to render as UTF-8 text; fallback to latin-1
try:
    text = out.decode("utf-8")
except Exception:
    text = out.decode("latin-1", errors="replace")

FINAL(text)
```
