#!/bin/sh
# run.sh HTTP_PORT CDP_PORT WIDTH HEIGHT SCALE MOBILE STEPS  (serves dist/viewer, fresh headless Chrome per flow)
set -e
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../../.." && pwd)
prof=$(mktemp -d)
python3 "$here/serve.py" "$1" "$root/dist/viewer" >/dev/null 2>&1 & srv=$!
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --use-angle=metal --remote-debugging-port="$2" --user-data-dir="$prof" about:blank >/dev/null 2>&1 & chr=$!
trap 'kill $srv $chr 2>/dev/null; rm -rf "$prof"' EXIT
node "$here/drive.mjs" "$2" "http://127.0.0.1:$1/" "$here" "$3" "$4" "$5" "$6" "$7"
