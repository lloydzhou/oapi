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
nohup python3 tests/test_server.py "$ROOT" "$PORT" >"$ROOT/server.out" 2>&1 &
SRVPID=$!
for _ in $(seq 1 1200); do
    [ -f "$ROOT/ready" ] && break
    sleep 0.05
done
if [ ! -f "$ROOT/ready" ]; then
    echo "FAIL: test server did not start (port $PORT, pid $SRVPID)"
    kill -0 "$SRVPID" 2>/dev/null && echo "server process alive" || echo "server process dead"
    echo "--- boot file ---"
    ls -la "$ROOT" 2>/dev/null
    echo "--- proxy env ---"
    env | grep -i proxy || echo "(none)"
    echo "--- server.out ---"
    cat "$ROOT/server.out" 2>/dev/null
    echo "--- error.log ---"
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

# --- multipart upload (-F name=@path) and binary output (-o FILE) ---

echo 'hello upload' > "$ROOT/upload.txt"
$OAPI connect up "$BASE/upload.json" >/dev/null 2>&1
assert_contains "multipart-upload" "$($OAPI up upload -F purpose=text_recognition -F file=@"$ROOT/upload.txt" 2>&1)" '"filename":"upload.txt"'
assert_contains "multipart-text" "$($OAPI up upload -F purpose=text_recognition -F file=@"$ROOT/upload.txt" 2>&1)" 'text_recognition'

# dry-run masks the multipart body instead of dumping file bytes
assert_contains "multipart-dryrun" "$($OAPI up upload -F file=@"$ROOT/upload.txt" --dry-run 2>&1)" '<multipart'

# -o FILE saves the raw response bytes (non-UTF-8 safe)
$OAPI up download -o "$ROOT/out.bin" >/dev/null 2>/dev/null
assert_eq "binary-exit" "$?" "0"
python3 -c "import sys; sys.exit(0 if open('$ROOT/out.bin','rb').read() == bytes([0,1,255,254]) + '中文二进制'.encode() else 1)"
assert_eq "binary-bytes" "$?" "0"
$OAPI rm up >/dev/null 2>&1

# --- end multipart/binary ---

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

# ${VAR} in header values expands from the environment
export OAPI_TEST_TOKEN='tok-secret-123'
assert_contains "header-env" "$($OAPI api pet GET /headers --header 'Authorization: Bearer ${OAPI_TEST_TOKEN}' 2>&1)" '"authorization":"Bearer tok-secret-123"'
unset OAPI_TEST_TOKEN

# unset variables fail closed instead of sending the placeholder
$OAPI api pet GET /headers --header 'Authorization: Bearer ${OAPI_UNSET_TEST_VAR}' >/dev/null 2>&1
assert_eq "header-env-missing" "$?" "1"

# --- connect-time credentials (mcpc-style) ---

# connect with -H stores the credential
assert_contains "cred-connect" "$($OAPI connect authed "$BASE/petstore.json" -H 'Authorization: Bearer tok-conn-123' 2>&1)" "+ credentials stored (1 header)"

# later calls send it automatically, no --header needed
assert_contains "cred-call-stored" "$($OAPI api authed GET /headers 2>&1)" '"authorization":"Bearer tok-conn-123"'

# credential file is owner-only, dir too
assert_eq "cred-mode" "$(python3 -c "import os; print(oct(os.stat('tests/o.home/oapi/credentials/authed.json').st_mode & 0o777))")" "0o600"
assert_eq "cred-dir-mode" "$(python3 -c "import os; print(oct(os.stat('tests/o.home/oapi/credentials').st_mode & 0o777))")" "0o700"

# the stored value never lands in the registry file
if grep -q 'tok-conn-123' tests/o.home/oapi/apis/authed.json 2>/dev/null; then
    echo "FAIL: cred-leaked-into-registry"
    fail=$((fail+1))
else
    echo "PASS: cred-not-in-registry"
    pass=$((pass+1))
fi

# dry-run masks the stored header too
assert_contains "cred-masked" "$($OAPI api authed GET /headers --dry-run 2>&1)" "Bearer ***"

# ls marks APIs that have credentials
assert_contains "cred-ls" "$($OAPI ls 2>&1)" "+cred"

# call-time --header overrides the stored one for the same header name
assert_contains "cred-override" "$($OAPI api authed GET /headers --header 'Authorization: Bearer tok-tmp' 2>&1)" '"authorization":"Bearer tok-tmp"'

# ${VAR} at connect resolves once; later calls need no environment
export OAPI_TEST_TOKEN='tok-conn-env'
$OAPI connect envapi "$BASE/petstore.json" -H 'Authorization: Bearer ${OAPI_TEST_TOKEN}' >/dev/null 2>&1
unset OAPI_TEST_TOKEN
assert_contains "cred-env-resolved" "$($OAPI api envapi GET /headers 2>&1)" '"authorization":"Bearer tok-conn-env"'
$OAPI rm envapi >/dev/null 2>&1

# connect without --header keeps stored credentials
assert_contains "cred-kept" "$($OAPI connect authed "$BASE/petstore.json" 2>&1)" "(credentials kept)"
assert_contains "cred-kept-sent" "$($OAPI api authed GET /headers 2>&1)" '"authorization":"Bearer tok-conn-123"'

# re-connect with a new header rotates the credential
assert_contains "cred-rotate" "$($OAPI connect authed "$BASE/petstore.json" -H 'Authorization: Bearer tok-new' 2>&1)" "+ credentials stored"
assert_contains "cred-rotated" "$($OAPI api authed GET /headers 2>&1)" '"authorization":"Bearer tok-new"'

# sync refreshes the spec but keeps credentials
$OAPI sync authed >/dev/null 2>&1
assert_contains "cred-sync-kept" "$($OAPI api authed GET /headers 2>&1)" '"authorization":"Bearer tok-new"'

# world-readable credential file is refused (fail closed, mcpc semantics)
chmod 644 tests/o.home/oapi/credentials/authed.json
$OAPI api authed GET /headers >/dev/null 2>/dev/null
assert_eq "cred-refused" "$?" "1"
assert_contains "cred-refused-msg" "$($OAPI api authed GET /headers 2>&1)" "group/other-readable"
chmod 600 tests/o.home/oapi/credentials/authed.json

# rm removes the registry entry and its credential
$OAPI rm authed >/dev/null 2>&1
assert_contains "cred-rm-unknown" "$($OAPI api authed GET /headers 2>&1)" "unknown API 'authed'"
assert_eq "cred-rm-file-gone" "$(ls tests/o.home/credentials/authed.json 2>/dev/null)" ""

# unknown connect option rejected
$OAPI connect authed "$BASE/petstore.json" --bogus >/dev/null 2>&1
assert_eq "cred-connect-badopt" "$?" "1"

# --- end connect-time credentials ---

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
