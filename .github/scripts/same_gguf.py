"""Check that a set of GGUF split files holds exactly the model of one GGUF file.

    python same_gguf.py ORIGINAL.gguf SPLIT-00001-of-0000N.gguf SPLIT-00002-... ...

Compares every metadata key and value (except the split.* keys the split
format adds) and every tensor's name, type, shape and data bytes. Exits 1 on
any difference. A byte-for-byte comparison of a re-merged file does not work:
the merge tool may lay out the header and padding differently.

Needs the `gguf` package (llama.cpp's own reader).
"""

import hashlib
import sys

from gguf import GGUFReader

CHUNK = 64 << 20


def metadata(reader):
    out = {}
    for name, field in reader.fields.items():
        if name.startswith("split.") or name.startswith("GGUF."):
            continue
        value = field.contents()
        out[name] = value.tolist() if hasattr(value, "tolist") else value
    return out


def tensors(path, reader):
    out = {}
    with open(path, "rb") as f:
        for t in reader.tensors:
            h = hashlib.sha256()
            f.seek(t.data_offset)
            left = t.n_bytes
            while left:
                block = f.read(min(CHUNK, left))
                if not block:
                    raise SystemExit(f"{path}: tensor {t.name} is cut short")
                h.update(block)
                left -= len(block)
            out[t.name] = (t.tensor_type.name, tuple(int(d) for d in t.shape), t.n_bytes, h.hexdigest())
    return out


def main(original, splits):
    a_reader = GGUFReader(original)
    a_meta, a_tensors = metadata(a_reader), tensors(original, a_reader)
    b_meta, b_tensors = {}, {}
    for path in splits:
        reader = GGUFReader(path)
        b_meta.update(metadata(reader))
        for name, info in tensors(path, reader).items():
            if name in b_tensors:
                raise SystemExit(f"tensor {name} appears in more than one split")
            b_tensors[name] = info
    problems = []
    for key in sorted(set(a_meta) | set(b_meta)):
        if a_meta.get(key) != b_meta.get(key):
            problems.append(f"metadata {key}: {str(a_meta.get(key))[:80]!r} vs {str(b_meta.get(key))[:80]!r}")
    for name in sorted(set(a_tensors) | set(b_tensors)):
        if a_tensors.get(name) != b_tensors.get(name):
            problems.append(f"tensor {name}: {a_tensors.get(name)} vs {b_tensors.get(name)}")
    if problems:
        print("\n".join(problems[:50]))
        raise SystemExit(f"{len(problems)} difference(s)")
    total = sum(info[2] for info in a_tensors.values())
    print(f"same model: {len(a_tensors)} tensors ({total} bytes of tensor data) and {len(a_meta)} metadata keys identical")


if __name__ == "__main__":
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    main(sys.argv[1], sys.argv[2:])
