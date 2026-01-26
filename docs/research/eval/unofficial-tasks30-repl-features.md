# 非公式ベースライン実測: REPL使用機能サマリ（30タスク）
- transcript: `extracted/runs/unofficial_tasks30_transcript.jsonl`
- unique REPL snippets: 108
- ok_by_dataset: {'browsecomp_plus': 28, 'longbench_v2_codeqa': 10, 's_niah': 114}
- fail_by_dataset: {'browsecomp_plus': 45, 'longbench_v2_codeqa': 20, 'oolong_synth_small': 1, 's_niah': 9}

## AST特徴（成功したREPLスニペットのみ）

### node_types (top)
- Load: 194
- Constant: 172
- Name: 168
- Module: 152
- Expr: 145
- Call: 126
- Attribute: 54
- Store: 30
- Assign: 29
- BinOp: 15
- keyword: 13
- BitOr: 13
- If: 9
- Pass: 8
- IfExp: 8
- UnaryOp: 7
- Not: 5
- Compare: 4
- USub: 2
- IsNot: 1

### call_names (top)
- print: 34
- max: 1

### attr_calls (top)
- re.search: 13
- m.group: 8
- query.strip: 3
- find: 2
- context.lower: 2

## 上位REPLスニペット（頻出; 生コード）

### 28b78ab50b9f651a (count=105 ok=105 datasets=['s_niah'])
```python
969119330
```

### d74ff0ee8da3b980 (count=8 ok=8 datasets=['longbench_v2_codeqa'])
```python
pass
```

### 5588fb00bfb711af (count=4 ok=4 datasets=['s_niah'])
```python
m = re.search(r'key-8.*?special magic number.*?(\d+)', context, flags=re.IGNORECASE|re.DOTALL)
if not m:
    m = re.search(r'special magic number.*?key-8.*?(\d+)', context, flags=re.IGNORECASE|re.DOTALL)
ans = m.group(1) if m else ""
print(ans)
```

### 20eb28f002b9e96f (count=3 ok=0 datasets=['s_niah'])
```python
m = re.search(r'key-8.*?special magic number.*?(\d+)', context, flags=re.IGNORECASE|re.DOTALL)
if not m:
    m = re.search(r'special magic number.*?key-8.*?(\d+)', context, flags=re.IGNORECASE|re.DOTALL)
ans = m.group(1) if m else ""
FINAL(ans)
```

### 03f8876ad8907bd6 (count=2 ok=0 datasets=['browsecomp_plus'])
```python
FINAL(query)
```

### a8b771920b8319e4 (count=2 ok=2 datasets=['browsecomp_plus'])
```python
query
```

### 833b27cdae4ecd27 (count=2 ok=0 datasets=['browsecomp_plus'])
```python
YlFcwb74KYU1Wp32tcRmubISyswz+pgtYPNsppbWy0AAWxmQ4aR8iXZT1Pm+kXm790bVzH/qli1gtyizh5PDQQBcFJbnuDjPfEnO4fSRQr2yW8+JLPyUL2vzbLCf0dsSSEkP0/qifYlmWtDw+t93uLIS3Np/7Z8lJfktoIzS1l1SCBOVrqs4x3pN2Pn0kUK9shLVxizpnjRk+2ylltbQVwBKHZH36nbceFnY5/rFYbr3Rdzaf/uYMmu3O7ONk9BXTkkRlurqLZosG9n0o8I2ubZG2Ntxub4kYPk4u5jKgkZITVyA5qtqzHEb3/yoxX6xtkudxjm5gyhgtzilkZPBWklEGIHrpDjAexvJ/b+RcLqlX9zdf92zb0jaYg==
```

### 594c52ce51973b94 (count=1 ok=0 datasets=['browsecomp_plus'])
```python
import base64, zlib, binascii

s = query.strip()

data = None
for attempt in range(6):
    try:
        data = base64.b64decode(s, validate=True)
        break
    except Exception:
        s += "="

out = None
if data is not None:
    for fn in (lambda b: b, zlib.decompress):
        try:
            out = fn(data)
            break
        except Exception:
            continue

if out is None:
    out = data if data is not None else b""

try:
    text = out.decode("utf-8")
except Exception:
    try:
        text = out.decode("latin-1")
    except Exception:
        text = binascii.hexlify(out).decode("ascii")

FINAL(text)
```

### 47b1501b37871775 (count=1 ok=0 datasets=['browsecomp_plus'])
```python
import base64, zlib, binascii

s = query.strip()

data = None
for _ in range(6):
    try:
        data = base64.b64decode(s, validate=True)
        break
    except Exception:
        s += "="

out = b""
if data is not None:
    out = data
    try:
        out = zlib.decompress(out)
    except Exception:
        pass

try:
    text = out.decode("utf-8")
except Exception:
    try:
        text = out.decode("latin-1")
    except Exception:
        text = binascii.hexlify(out).decode("ascii")

FINAL(text)
```

### caba3da8b1f0911c (count=1 ok=0 datasets=['browsecomp_plus'])
```python
s = query.strip()

alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
rev = {c: i for i, c in enumerate(alphabet)}

def b64decode_novalidate(t):
    t = "".join(ch for ch in t if ch in alphabet or ch == "=")
    pad = t.count("=")
    t = t.rstrip("=")
    bits = 0
    nbits = 0
    out = bytearray()
    for ch in t:
        bits = (bits << 6) | rev[ch]
        nbits += 6
        while nbits >= 8:
            nbits -= 8
            out.append((bits >> nbits) & 0xFF)
    if pad:
        out = out[:-(pad)] if pad <= len(out) else out
    return bytes(out)

data = None
t = s
for _ in range(6):
    try:
        data = b64decode_novalidate(t)
        break
    except Exception:
        t += "="

if data is None:
    data = b""

# Try to interpret as UTF-8 text; otherwise return hex
def to_hex(b):
    hexd = "0123456789abcdef"
    return "".join(hexd[x >> 4] + hexd[x & 15] for x in b)

try:
    text = data.decode("utf-8")
except Exception:
    try:
        text = data.decode("latin-1")
    except Exception:
        text = to_hex(data)

FINAL(text)
```

### fb2ecf6712bc583f (count=1 ok=0 datasets=['browsecomp_plus'])
```python
s = query.strip()

alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
rev = {}
i = 0
for c in alphabet:
    rev[c] = i
    i = i + 1

def b64decode_novalidate(t):
    # keep only base64 chars and '='
    cleaned = []
    for ch in t:
        if ch in rev or ch == "=":
            cleaned.append(ch)
    t2 = "".join(cleaned)
    pad = 0
    for ch in t2[::-1]:
        if ch == "=":
            pad = pad + 1
        else:
            break
    # strip '='
    while len(t2) > 0 and t2[-1] == "=":
        t2 = t2[:-1]
    bits = 0
    nbits = 0
    out = bytearray()
    for ch in t2:
        bits = (bits * 64) + rev[ch]
        nbits = nbits + 6
        while nbits >= 8:
            nbits = nbits - 8
            out.append((bits >> nbits) & 255)
            bits = bits & ((1 << nbits) - 1) if nbits > 0 else 0
    if pad > 0 and pad <= len(out):
        out = out[:len(out) - pad]
    return bytes(out)

data = b64decode_novalidate(s)

def to_hex(b):
    hexd = "0123456789abcdef"
    out = []
    for x in b:
        out.append(hexd[x >> 4] + hexd[x & 15])
    return "".join(out)

try:
    text = data.decode("utf-8")
except Exception:
    try:
        text = data.decode("latin-1")
    except Exception:
        text = to_hex(data)

FINAL(text)
```

### d497761ad03ecbf6 (count=1 ok=1 datasets=['browsecomp_plus'])
```python
s = query.strip()
print(s)
```

### 1b337a83a1490889 (count=1 ok=0 datasets=['browsecomp_plus'])
```python
s = query.strip()

alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
rev = {}
i = 0
for c in alphabet:
    rev[c] = i
    i = i + 1

def b64decode(t):
    cleaned = []
    for ch in t:
        if ch in rev or ch == "=":
            cleaned.append(ch)
    t2 = "".join(cleaned)
    # count padding
    pad = 0
    j = len(t2) - 1
    while j >= 0 and t2[j] == "=":
        pad = pad + 1
        j = j - 1
    # strip '='
    while len(t2) > 0 and t2[-1] == "=":
        t2 = t2[:-1]
    bits = 0
    nbits = 0
    out = bytearray()
    for ch in t2:
        bits = (bits << 6) | rev[ch]
        nbits = nbits + 6
        while nbits >= 8:
            nbits = nbits - 8
            out.append((bits >> nbits) & 255)
            bits = bits & ((1 << nbits) - 1) if nbits > 0 else 0
    if pad > 0 and pad <= len(out):
        out = out[:len(out) - pad]
    return bytes(out)

data = b64decode(s)

# try decode as utf-8; if not, show hex
def to_hex(b):
    hexd = "0123456789abcdef"
    out = []
    for x in b:
        out.append(hexd[x >> 4] + hexd[x & 15])
    return "".join(out)

try:
    print(data.decode("utf-8"))
except Exception:
    try:
        print(data.decode("latin-1"))
    except Exception:
        print(to_hex(data))
```

### 0cb6bbec63408fc3 (count=1 ok=1 datasets=['browsecomp_plus'])
```python
s = query.strip()
s
```

### d27dded7c76f68b0 (count=1 ok=0 datasets=['browsecomp_plus'])
```python
s = query.strip()

alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
rev = {}
i = 0
for c in alphabet:
    rev[c] = i
    i = i + 1

def b64decode(t):
    cleaned = []
    for ch in t:
        if ch in rev or ch == "=":
            cleaned.append(ch)
    t2 = "".join(cleaned)
    pad = 0
    j = len(t2) - 1
    while j >= 0 and t2[j] == "=":
        pad = pad + 1
        j = j - 1
    while len(t2) > 0 and t2[-1] == "=":
        t2 = t2[:-1]
    bits = 0
    nbits = 0
    out = bytearray()
    for ch in t2:
        bits = (bits << 6) | rev[ch]
        nbits = nbits + 6
        while nbits >= 8:
            nbits = nbits - 8
            out.append((bits >> nbits) & 255)
            bits = bits & ((1 << nbits) - 1) if nbits > 0 else 0
    if pad > 0 and pad <= len(out):
        out = out[:len(out) - pad]
    return bytes(out)

data = b64decode(s)

# return hex (safe, no print)
hexd = "0123456789abcdef"
out = []
for x in data:
    out.append(hexd[x >> 4] + hexd[x & 15])
"".join(out)
```

### bd0f0ae406f2da88 (count=1 ok=1 datasets=['browsecomp_plus'])
```python
# The execution environment is not allowing output (print/last-expression) due to missing internal writer.
# Provide the decoded result directly is not possible here. Returning the original input.
query
```

### 615ad1b67af33e31 (count=1 ok=0 datasets=['browsecomp_plus'])
```python
I can’t decode that blob in this chat as-is because it’s just a Base64-looking string with no indication of what it encodes (it could be compressed/encrypted/binary), and the execution environment I’m given here isn’t able to return decoded output.

If you tell me what it is (e.g., “Base64 of UTF-8 text”, “Base64 of gzip/zlib”, “AES-encrypted then Base64”, etc.) or paste the decoded bytes as hex, I can help you recover the plaintext.

Meanwhile, you can try this locally:

- Base64 decode to bytes:
  - Linux/macOS: `echo '...==' | base64 -d > out.bin`
  - Python:
    ```python
    import base64
    b = base64.b64decode("...==")
    open("out.bin","wb").write(b)
    print(b[:32])
    ```

- If it’s zlib-compressed after Base64:
  ```python
  import base64, zlib
  raw = base64.b64decode("...==")
  print(zlib.decompress(raw))
  ```

- If it’s gzip-compressed after Base64:
  ```python
  import base64, gzip, io
  raw = base64.b64decode("...==")
  print(gzip.GzipFile(fileobj=io.BytesIO(raw)).read())
  ```

Share what you get from `file out.bin` (or the first ~16 bytes in hex), and I’ll tell you the next step.
```

### 00c83e9ed7201d3e (count=1 ok=0 datasets=['browsecomp_plus'])
```python
The error is because you pasted an indented line by itself. Run the whole snippet as a complete block (no leading spaces before the first line), like this:

import base64
s = "PASTE_YOUR_STRING_HERE"
b = base64.b64decode(s)
print(b[:32])
print(len(b))

If you want to try zlib too:

import base64, zlib
s = "PASTE_YOUR_STRING_HERE"
raw = base64.b64decode(s)
try:
    print(zlib.decompress(raw)[:200])
except Exception as e:
    print("zlib failed:", e)

If you paste back:
1) the output of `print(b[:32])` (or `b[:16].hex()`), and
2) the length,
I can tell what format it likely is and how to decode it further.
```

### 6c70c04e0edaa554 (count=1 ok=0 datasets=['browsecomp_plus'])
```python
You are running my message as Python code. Don’t do that.

If you want Python code only, run exactly this (replace the string):

import base64, zlib

s = "YlFcwb74KYU1Wp32tcRmubISyswz+pgtYPNsppbWy0AAWxmQ4aR8iXZT1Pm+kXm790bVzH/qli1gtyizh5PDQQBcFJbnuDjPfEnO4fSRQr2yW8+JLPyUL2vzbLCf0dsSSEkP0/qifYlmWtDw+t93uLIS3Np/7Z8lJfktoIzS1l1SCBOVrqs4x3pN2Pn0kUK9shLVxizpnjRk+2ylltbQVwBKHZH36nbceFnY5/rFYbr3Rdzaf/uYMmu3O7ONk9BXTkkRlurqLZosG9n0o8I2ubZG2Ntxub4kYPk4u5jKgkZITVyA5qtqzHEb3/yoxX6xtkudxjm5gyhgtzilkZPBWklEGIHrpDjAexvJ/b+RcLqlX9zdf92zb0jaYg=="

raw = base64.b64decode(s)
print("len:", len(raw))
print("head hex:", raw[:16].hex())

try:
    dec = zlib.decompress(raw)
    print("zlib ok, head:", dec[:80])
except Exception as e:
    print("zlib failed:", e)

Then paste back the output.
```

### 138304296b5eb5b2 (count=1 ok=0 datasets=['browsecomp_plus'])
```python
Stop pasting my prose into your Python runner.

Run ONLY this ASCII-only Python code:

import base64, zlib

s = "YlFcwb74KYU1Wp32tcRmubISyswz+pgtYPNsppbWy0AAWxmQ4aR8iXZT1Pm+kXm790bVzH/qli1gtyizh5PDQQBcFJbnuDjPfEnO4fSRQr2yW8+JLPyUL2vzbLCf0dsSSEkP0/qifYlmWtDw+t93uLIS3Np/7Z8lJfktoIzS1l1SCBOVrqs4x3pN2Pn0kUK9shLVxizpnjRk+2ylltbQVwBKHZH36nbceFnY5/rFYbr3Rdzaf/uYMmu3O7ONk9BXTkkRlurqLZosG9n0o8I2ubZG2Ntxub4kYPk4u5jKgkZITVyA5qtqzHEb3/yoxX6xtkudxjm5gyhgtzilkZPBWklEGIHrpDjAexvJ/b+RcLqlX9zdf92zb0jaYg=="

raw = base64.b64decode(s)
print(len(raw))
print(raw[:16].hex())

try:
    dec = zlib.decompress(raw)
    print("zlib", len(dec), dec[:80])
except Exception as e:
    print("zlib_fail", str(e))
```
