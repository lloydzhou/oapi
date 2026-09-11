# oapi

A standalone OpenAPI command-line client written in Rust.

`oapi` registers an OpenAPI (swagger) JSON document under a short name and then
calls its operations as if they were plain shell commands: operationIds become
kebab-case commands, required path parameters become positional arguments and
query/header parameters become `--kebab-case` flags.

## Install

```bash
cargo install --path .
# or via Homebrew
brew install lloydzhou/tap/oapi
```

```bash
# or via APT (Debian/Ubuntu; the repository is hosted alongside bash-agent)
curl -fsSL https://lloydzhou.github.io/bash-agent/install.sh | sudo bash
sudo apt-get install oapi
```

```bash
# or via AUR (Arch Linux; oapi-bin for the prebuilt binary)
yay -S oapi
```

## Usage

```bash
oapi connect petstore https://example.com/openapi.json   # cache the spec
oapi ls                                                  # list registered APIs
oapi schema petstore                                     # list operations
oapi schema petstore get-pet-by-id                       # show one operation

oapi petstore list-pets --limit 10                       # query flag
oapi petstore get-pet-by-id 42                           # positional path param
oapi petstore create-pet -F name=fluffy -F age=3         # typed body fields
oapi petstore list-pets --jq '[].name'                   # filter output
oapi petstore list-pets --jq '[].id' -o text             # plain text output

oapi api petstore GET /pets --params 'limit=1'           # raw escape hatch
oapi sync petstore                                       # re-fetch from source
oapi rm petstore                                         # drop the entry
```

## CLI overview

```
oapi connect NAME SPEC-URL-OR-FILE | sync NAME | ls | rm NAME
oapi schema NAME [OPERATION]
oapi api NAME METHOD PATH [--params J] [--data J]
oapi NAME OPERATION [PARAM]... [--FLAG V]... [--body X] [-F K=V]

call options: --body X (inline / @file / -)   -F K=V (repeatable, auto-typed)
              --jq PATH   -o text   --header 'N: V'   --dry-run   --timeout N
```

stdout carries data only; diagnostics go to stderr and a non-2xx response
exits 1. `--dry-run` prints the request (with credential headers masked)
instead of sending it.

## Registry

Specs are cached under `$OAPI_HOME/apis/<name>.json` (falls back to
`$BA_HOME/oapi`, then `~/.local/share/oapi`) as
`{"source": ..., "fetched_at": ..., "spec": {...}}`.

## Development

```bash
cargo build
cargo test
bash tests/test.sh          # e2e tests against a local petstore server
```

## Relationship to bash-agent

This tool implements the OpenAPI half of the functionality discussed in
[bash-agent#88](https://github.com/lloydzhou/bash-agent/issues/88). It is a
standalone Rust port of the `oapi` applet from
[lloydzhou/busyagent](https://github.com/lloydzhou/busyagent) PR #4, sharing the
same CLI contract so it can be composed with `bash-agent` or used on its own.

## License

MIT
