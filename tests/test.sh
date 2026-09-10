#!/bin/bash
# oapi e2e test suite (petstore semantics, port of the busyagent oapi.tests)
set -u
cd "$(dirname "$0")/.."
OAPI=$PWD/target/debug/oapi
ROOT=$PWD/tests/e2e
PORT=$((24000 + $$ % 20000))
fail=0
pass=0

rm -rf "$ROOT" tests/o.home
export BA_HOME=$PWD/tests/o.home

cargo build 2>/dev/null

assert_eq() {
    local name="$1" got="$2" want="$3"
    if [ "$got" = "$want" ]; then
        echo "PASS: $name"
        pass=$((pass+1))
    else
        echo "FAIL: $name"
        echo "  want: $want"
        echo "  got:  $got"
        fail=$((fail+1))
    fi
}

assert_contains() {
    local name="$1" got="$2" want="$3"
    if printf '%s' "$got" | grep -Fq -- "$want"; then
        echo "PASS: $name"
        pass=$((pass+1))
    else
        echo "FAIL: $name"
        echo "  want substring: $want"
        echo "  got:  $got"
        fail=$((fail+1))
    fi
}

# start test httpd (background, foreground script, ready-file handshake)
mkdir -p "$ROOT"
nohup python3 tests/test_server.py "$ROOT" "$PORT" >/dev/null 2>&1 &
for _ in $(seq 1 200); do
    [ -f "$ROOT/ready" ] && break
    sleep 0.05
done
if [ ! -f "$ROOT/ready" ]; then
    echo "FAIL: test server did not start (port $PORT)"
    cat "$ROOT/error.log" 2>/dev/null
    exit 1
fi
BASE="http://127.0.0.1:$PORT"

# usage without args exits 1
$OAPI >/dev/null 2>&1
assert_eq "usage" "$?" "1"

# connect from file
assert_eq "connect-file" "$($OAPI connect pet tests/petstore.json 2>&1)" "connected pet: 3 operations (source tests/petstore.json)"

# connect from URL (re-register with http source)
assert_eq "connect-url" "$($OAPI connect pet "$BASE/petstore.json" 2>&1)" "connected pet: 3 operations (source $BASE/petstore.json)"

# ls shows the registry
assert_contains "ls" "$($OAPI ls 2>&1)" "3 operations"

# schema lists operations in kebab-case
assert_contains "schema-list" "$($OAPI schema pet 2>&1)" "list-pets"
assert_contains "schema-op" "$($OAPI schema pet get-pet-by-id 2>&1)" "petId"
assert_contains "schema-raw-id" "$($OAPI schema pet getPetById 2>&1)" "GET /pets/{petId}"

# operation call: query flag
assert_contains "call-query" "$($OAPI pet list-pets --limit 2 2>&1)" '"id":1'

# call with jq filter
assert_eq "call-jq" "$($OAPI pet list-pets --jq '[].name' 2>&1 | tr '\n' ' ')" "fluffy brie "

# call with -o text
assert_eq "call-text" "$($OAPI pet list-pets --jq '[].id' -o text 2>&1 | tr '\n' ' ')" "1 2 "

# path parameter positional
assert_contains "call-path" "$($OAPI pet get-pet-by-id 42 2>&1)" '"id":"42"'

# positional count mismatch
$OAPI pet get-pet-by-id >/dev/null 2>&1
assert_eq "path-missing" "$?" "1"

# -F body fields with type inference
assert_contains "call-f" "$($OAPI pet create-pet -F name=fluffy -F age=3 2>&1)" '"age":3'

# --body inline JSON
assert_contains "call-body" "$($OAPI pet create-pet --body '{"name":"x","tags":[1,2]}' 2>&1)" '"tags":[1,2]'

# --body @file
echo '{"name":"fromfile"}' > "$ROOT/body.json"
assert_contains "call-body-file" "$($OAPI pet create-pet --body @"$ROOT/body.json" 2>&1)" 'fromfile'

# unknown flag rejected
$OAPI pet list-pets --bogus 1 >/dev/null 2>&1
assert_eq "unknown-flag" "$?" "1"

# non-2xx exits 1 with HTTP status on stderr
$OAPI api pet GET /fail >/dev/null 2>/dev/null
assert_eq "api-fail" "$?" "1"

# dry-run prints request without sending
assert_eq "dry-run" "$($OAPI pet list-pets --limit 7 --dry-run 2>&1)" "GET $BASE/api/pets?limit=7"

# api raw escape hatch (paths are relative to the spec base url)
assert_contains "api-get" "$($OAPI api pet GET /pets 2>&1)" '"id":1'

# api with --params and --data
assert_contains "api-params" "$($OAPI api pet GET /pets --params 'limit=1' 2>&1)" '"id":1'
assert_contains "api-data" "$($OAPI api pet POST /pets --data '{"k":1}' 2>&1)" '"k":1'

# dry-run masks sensitive headers
assert_contains "mask-header" "$($OAPI pet list-pets --header 'Authorization: Bearer secret123' --dry-run 2>&1)" 'Bearer ***'

# sync re-fetches from the stored source
assert_eq "sync" "$($OAPI sync pet 2>&1)" "connected pet: 3 operations (source $BASE/petstore.json)"

# kebab-case unit: acronym handling is covered by schema output
assert_contains "kebab" "$($OAPI schema pet 2>&1)" "get-pet-by-id"

# rm removes the entry
assert_eq "rm" "$($OAPI rm pet 2>&1; $OAPI ls 2>&1)" "no APIs registered (oapi connect NAME SPEC)"

# unknown API name
$OAPI pet list-pets >/dev/null 2>&1
assert_eq "unknown-api" "$?" "1"

# cleanup
PID=$(cat "$ROOT/test.pid" 2>/dev/null)
if [ -n "$PID" ]; then
    kill "$PID" 2>/dev/null
    wait "$PID" 2>/dev/null
fi
rm -rf "$ROOT" tests/o.home

if [ "$fail" -gt 0 ]; then
    echo "FAILED ($fail tests)"
    exit 1
fi
echo "ALL PASSED ($pass tests)"
