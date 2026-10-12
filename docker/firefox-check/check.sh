#!/bin/sh
# Drives a real geckodriver with curl and prints every request and response.
#
# It installs a temporary add-on as an unpacked directory and then as an .xpi
# file, sees the add-on's content script run on a page, uninstalls the add-on and
# sees the script stop. The bodies sent here are the ones
# BaseCase::install_addon and uninstall_addon send. It also records what
# geckodriver answers when it refuses a request. Exits non-zero if a request that
# should work does not answer 200 or the add-on does not behave.
set -eu

DRIVER=http://127.0.0.1:4444
PAGE=http://127.0.0.1:8000/index.html
OUT=/tmp/response.json
STATUS=0

say() { printf '\n== %s\n' "$*"; }

fail() {
    printf '\nFAILED: %s\n' "$*" >&2
    exit 1
}

# call METHOD PATH [BODY]: sends the request, prints it and what came back, and
# leaves the HTTP status in STATUS and the body in OUT.
call() {
    method=$1
    path=$2
    body=${3-}
    printf '>>> %s %s %s\n' "$method" "$path" "$body"
    if [ -n "$body" ]; then
        STATUS=$(curl -sS -o "$OUT" -w '%{http_code}' -X "$method" \
            -H 'content-type: application/json' -d "$body" "$DRIVER$path")
    else
        STATUS=$(curl -sS -o "$OUT" -w '%{http_code}' -X "$method" "$DRIVER$path")
    fi
    printf '<<< HTTP %s %s\n' "$STATUS" "$(cat "$OUT")"
}

ok() {
    [ "$STATUS" = 200 ] || fail "expected HTTP 200 for: $*"
}

# mark SESSION: prints the add-on's mark on the page the browser is on, as a JSON
# value. The request and response go to stderr so that `$(mark ...)` is the value.
mark() {
    call POST "/session/$1/execute/sync" \
        '{"script":"return document.documentElement.getAttribute(\"data-sbrs-addon\");","args":[]}' >&2
    ok reading the page
    jq -c .value "$OUT"
}

say "versions"
geckodriver --version | head -n 1
firefox-esr --version

say "start a page server and geckodriver"
mkdir -p /tmp/site
echo '<!doctype html><title>page</title><p>page</p>' >/tmp/site/index.html
python3 -m http.server 8000 --bind 127.0.0.1 --directory /tmp/site >/tmp/http.log 2>&1 &
geckodriver --host 127.0.0.1 --port 4444 --log warn >/tmp/geckodriver.log 2>&1 &
for _ in $(seq 1 50); do
    if curl -fsS "$DRIVER/status" >/dev/null 2>&1; then
        break
    fi
    sleep 0.2
done
curl -fsS "$DRIVER/status" >/dev/null || fail "geckodriver did not start: $(cat /tmp/geckodriver.log)"

say "new headless Firefox session"
call POST /session \
    '{"capabilities":{"alwaysMatch":{"browserName":"firefox","moz:firefoxOptions":{"args":["-headless"]}}}}'
ok creating the session
SESSION=$(jq -r .value.sessionId "$OUT")
echo "session id: $SESSION"

say "before the install the page carries no mark"
call POST "/session/$SESSION/url" "{\"url\":\"$PAGE\"}"
ok opening the page
BEFORE=$(mark "$SESSION")
echo "mark: $BEFORE"
[ "$BEFORE" = null ] || fail "the page had a mark before the add-on was installed"

say "install the unpacked add-on directory, temporary"
call POST "/session/$SESSION/moz/addon/install" '{"path":"/opt/addon","temporary":true}'
ok installing the directory
DIR_ID=$(jq -r .value "$OUT")
echo "add-on id: $DIR_ID"
[ "$DIR_ID" = "sbrs-check@example.org" ] || fail "unexpected add-on id: $DIR_ID"

say "with the add-on installed the page carries its mark"
call POST "/session/$SESSION/url" "{\"url\":\"$PAGE\"}"
ok opening the page
INSTALLED=$(mark "$SESSION")
echo "mark: $INSTALLED"
[ "$INSTALLED" = '"installed"' ] || fail "the add-on's content script did not run"

say "uninstall it"
call POST "/session/$SESSION/moz/addon/uninstall" "{\"id\":\"$DIR_ID\"}"
ok uninstalling
call POST "/session/$SESSION/url" "{\"url\":\"$PAGE\"}"
ok opening the page
AFTER=$(mark "$SESSION")
echo "mark: $AFTER"
[ "$AFTER" = null ] || fail "the add-on still ran after it was uninstalled"

say "install the same add-on packed as an .xpi file, temporary"
(cd /opt/addon && zip -q -r /tmp/sbrs-check.xpi .)
call POST "/session/$SESSION/moz/addon/install" '{"path":"/tmp/sbrs-check.xpi","temporary":true}'
ok installing the .xpi
XPI_ID=$(jq -r .value "$OUT")
echo "add-on id: $XPI_ID"
[ "$XPI_ID" = "sbrs-check@example.org" ] || fail "unexpected add-on id: $XPI_ID"
call POST "/session/$SESSION/url" "{\"url\":\"$PAGE\"}"
ok opening the page
[ "$(mark "$SESSION")" = '"installed"' ] || fail "the .xpi's content script did not run"
call POST "/session/$SESSION/moz/addon/uninstall" "{\"id\":\"$XPI_ID\"}"
ok uninstalling the .xpi

say "what geckodriver answers when it refuses (for the error cases)"
call POST "/session/$SESSION/moz/addon/install" '{"path":"/opt/no-such-addon.xpi","temporary":true}'
echo "(refused: status $STATUS)"
call POST "/session/$SESSION/moz/addon/install" '{"path":"/tmp/sbrs-check.xpi","temporary":false}'
echo "(refused: status $STATUS)"
call POST "/session/$SESSION/moz/addon/uninstall" '{"id":"never-installed@example.org"}'
echo "(status $STATUS)"
call POST "/session/$SESSION/moz/addon/install" '{"temporary":true}'
echo "(refused: status $STATUS)"

say "end the session"
call DELETE "/session/$SESSION"
ok deleting the session

printf '\nALL CHECKS PASSED\n'
