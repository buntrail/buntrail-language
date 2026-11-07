#!/usr/bin/env bash
# hello
hello() {
 for name in "hello" world; do
  if [ -n "$name" ]; then echo "$name"; fi
 done
 cat <<'TEXT'
hello { literal }
TEXT
}
