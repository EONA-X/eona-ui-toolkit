#!/usr/bin/env bash
# Keeps the toolkit's chart reader and the vocabulary's chart profile in step.
#
#   tests/differential.sh <eona-vocabularies-reference checkout>
#
# 1. Byte-for-byte: `src/cube/context.jsonld` is the profile's `v0.1.0/context.jsonld`,
#    and `tests/fixtures/cube/profile/` is the profile's `tests/`.
# 2. Verdicts: for every fixture, pySHACL (the shapes) and `examples/cube-verdict.rs`
#    (the reader) both accept or both reject; conforming ones must be accepted,
#    violating ones rejected.
# 3. Samples: every `assets/samples/*.jsonld`, with the built-in context inlined
#    (pySHACL does not fetch), conforms to the shapes and is accepted by the reader.
# 4. The same verdict rule on `tests/fixtures/cube/{conforming,violating}`.
#
# Needs pyshacl (0.40.1 in CI), python3 and a Rust toolchain.
set -uo pipefail
cd "$(dirname "$0")/.."
vocab=${1:?usage: tests/differential.sh <eona-vocabularies-reference checkout>}
profile="$vocab/eonax-chart-profile"
shapes="$(ls -d "$profile"/v*/ | sort -V | tail -1)ontology.ttl"
context="$(dirname "$shapes")/context.jsonld"
status=0
fail() { echo "FAIL $*"; status=1; }

# 1. copies
cmp -s src/cube/context.jsonld "$context" || fail "src/cube/context.jsonld differs from $context"
for d in conforming violating context; do
  diff -r "tests/fixtures/cube/profile/$d" "$profile/tests/$d" >/dev/null \
    || fail "tests/fixtures/cube/profile/$d differs from $profile/tests/$d"
done

# 2. the reader's verdicts, all files in one run
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
cargo build -q --example cube-verdict --features charts || { echo "FAIL cube-verdict does not build"; exit 1; }
verdict=target/debug/examples/cube-verdict

shacl_verdict() { # file -> accept|reject
  if pyshacl -w -s "$shapes" -df json-ld "$1" >/dev/null 2>&1; then echo accept; else echo reject; fi
}
reader_verdicts=$("$verdict" "$profile"/tests/conforming/*.jsonld "$profile"/tests/violating/*.jsonld)
for f in "$profile"/tests/conforming/*.jsonld "$profile"/tests/violating/*.jsonld; do
  want=accept; [[ $f == */violating/* ]] && want=reject
  shacl=$(shacl_verdict "$f")
  reader=$(awk -F'\t' -v f="$f" '$2 == f { print $1 }' <<<"$reader_verdicts")
  if [[ $shacl != "$reader" ]]; then fail "verdicts differ on $f: pyshacl $shacl, reader $reader"
  elif [[ $reader != "$want" ]]; then fail "$f should be ${want}ed, both say $reader"
  else echo "ok   $reader   $f"; fi
done

# 3. samples
python3 - "$context" "$tmp" assets/samples/*.jsonld <<'PY'
import json, os, sys
ctx = json.load(open(sys.argv[1]))["@context"]
out = sys.argv[2]
for path in sys.argv[3:]:
    doc = json.load(open(path))
    doc["@context"] = ctx
    json.dump(doc, open(os.path.join(out, os.path.basename(path)), "w"))
PY
for f in assets/samples/*.jsonld; do
  shacl=$(shacl_verdict "$tmp/$(basename "$f")")
  reader=$("$verdict" "$f" | cut -f1)
  if [[ $shacl != accept || $reader != accept ]]; then fail "sample $f: pyshacl $shacl, reader $reader"
  else echo "ok   sample   $f"; fi
done

# 4. the toolkit's own fixtures, with the same verdict rule. `remote-context` is the one
#    reader-only rule: inlining the built-in context would hide it from pySHACL.
mkdir "$tmp/own"
for kind in conforming violating; do
  mkdir "$tmp/own/$kind"
  python3 - "$context" "$tmp/own/$kind" tests/fixtures/cube/$kind/*.jsonld <<'PY'
import json, os, sys
ctx = json.load(open(sys.argv[1]))["@context"]
out = sys.argv[2]
for path in sys.argv[3:]:
    doc = json.load(open(path))
    if doc.get("@context") == "https://eona-x.eu/shape/chart-profile/context.jsonld":
        doc["@context"] = ctx
    json.dump(doc, open(os.path.join(out, os.path.basename(path)), "w"))
PY
done
own_verdicts=$("$verdict" tests/fixtures/cube/conforming/*.jsonld tests/fixtures/cube/violating/*.jsonld)
for f in tests/fixtures/cube/conforming/*.jsonld tests/fixtures/cube/violating/*.jsonld; do
  [[ $f == */remote-context.jsonld ]] && continue
  want=accept; [[ $f == */violating/* ]] && want=reject
  shacl=$(shacl_verdict "$tmp/own/${f#tests/fixtures/cube/}")
  reader=$(awk -F'\t' -v f="$f" '$2 == f { print $1 }' <<<"$own_verdicts")
  if [[ $shacl != "$reader" ]]; then fail "verdicts differ on $f: pyshacl $shacl, reader $reader"
  elif [[ $reader != "$want" ]]; then fail "$f should be ${want}ed, both say $reader"
  else echo "ok   $reader   $f"; fi
done
exit $status
