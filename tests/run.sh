#!/usr/bin/env bash
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
bless=0; [ "${1:-}" = "--bless" ] && { bless=1; shift; }
bin="${BR_SERVER_BIN:-}"
if [ -z "$bin" ]; then
  cargo build --locked -q -p br-server --manifest-path "$root/Cargo.toml" || exit 2
  bin="$root/target/debug/br-server"; [ -x "$bin" ] || bin="$bin.exe"
fi
work="$(mktemp -d)"; trap 'kill $(jobs -p) 2>/dev/null; rm -rf "$work"' EXIT
names=("$@"); [ ${#names[@]} -eq 0 ] && names=($(ls "$root/tests/cases"))
failed=()

run_case() {
  local dir="$root/tests/cases/$1" data="$work/$1" envs=()
  mkdir -p "$data"
  [ -f "$dir/env" ] && while IFS= read -r l; do [ -n "$l" ] && envs+=("$l"); done < "$dir/env"
  if [ ! -f "$dir/request" ]; then
    local err; err="$(env PORT=0 BR_DATA_DIR="$data" "${envs[@]}" "$bin" 2>&1 >/dev/null </dev/null)"
    printf 'exit %s\n%s\n' "$?" "$err"; return
  fi
  local out="$work/$1.out"
  env PORT=0 BR_DATA_DIR="$data" LOG_LEVEL=error "${envs[@]}" "$bin" >"$out" 2>/dev/null </dev/null &
  local pid=$! port=""
  for _ in $(seq 100); do
    port="$(sed -n 's/^BR_SERVER_LISTENING //p' "$out" | tr -d '\r')"; [ -n "$port" ] && break; sleep 0.1
  done
  [ -z "$port" ] && { echo "server did not start"; kill $pid 2>/dev/null; return; }
  local method path hdrs=()
  read -r method path < "$dir/request"; path="${path%$'\r'}"
  while IFS= read -r l; do l="${l%$'\r'}"; [ -n "$l" ] && hdrs+=(-H "$l"); done < <(tail -n +2 "$dir/request")
  curl -s -X "$method" "${hdrs[@]}" -w '%{http_code}\n' "http://127.0.0.1:$port$path" | tr -d '\r'
  kill $pid 2>/dev/null; wait $pid 2>/dev/null
}

for n in "${names[@]}"; do
  actual="$(run_case "$n")"
  if [ $bless -eq 1 ]; then printf '%s\n' "$actual" > "$root/tests/cases/$n/expected"; continue; fi
  if ! diff -u <(tr -d '\r' < "$root/tests/cases/$n/expected") <(printf '%s\n' "$actual") >"$work/diff" 2>&1; then
    failed+=("$n"); echo "FAIL $n"; cat "$work/diff"
  fi
done
echo "${#names[@]} cases, ${#failed[@]} failed"
[ ${#failed[@]} -eq 0 ] || { printf 'failing: %s\n' "${failed[*]}"; exit 1; }
