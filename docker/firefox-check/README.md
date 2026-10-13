# geckodriver add-on check

A container with `firefox-esr` and geckodriver, and none of this crate. `check.sh`
drives geckodriver with `curl` and prints every request and response, so
[`BaseCase::install_addon`](../../src/api/base_case_impls/base_case_impl_addon.rs)
and `uninstall_addon` can be compared with what a real geckodriver accepts. Chrome
cannot answer this: only geckodriver has the `moz/addon` commands.

```sh
docker build -t sbrs-firefox-check docker/firefox-check
docker run --rm --shm-size 512m sbrs-firefox-check
docker rmi sbrs-firefox-check        # when done
```

The image is `linux/arm64` (geckodriver's `linux-aarch64` release, whose
SHA-256 the build checks); on an `amd64` host change the release name and the
checksum in the `Dockerfile` together. Nothing is exposed: geckodriver and a tiny
page server listen on loopback inside the container.

## What it does

1. Starts a headless Firefox session and opens a page the check serves.
2. Installs `addon/`, an unpacked Manifest V2 add-on (`temporary: true`), and
   checks that its content script marks the page. Then uninstalls it and checks
   that the mark is gone.
3. Does the same with the add-on packed as an `.xpi`.
4. Sends four requests geckodriver refuses, and prints what it answers.

## The bodies geckodriver accepted

`last-run.txt` is the full output (geckodriver 0.37.1, Firefox 153.4.0 ESR, on
2026-10-11). The requests, below `/session/{id}`:

| Request | Body | Answer |
| --- | --- | --- |
| `POST /moz/addon/install` | `{"path":"/opt/addon","temporary":true}` (a directory) | 200 `{"value":"sbrs-check@example.org"}` |
| `POST /moz/addon/install` | `{"path":"/tmp/sbrs-check.xpi","temporary":true}` | 200 `{"value":"sbrs-check@example.org"}` |
| `POST /moz/addon/uninstall` | `{"id":"sbrs-check@example.org"}` | 200 `{"value":null}` |

The value of an install is the add-on's id, the `browser_specific_settings.gecko.id`
of its manifest. What geckodriver refuses, all as `{"value":{"error":...}}`:

| Request body | Status | Message |
| --- | --- | --- |
| `{"path":"/opt/no-such-addon.xpi","temporary":true}` | 500 `unknown error` | `No such file or directory: /opt/no-such-addon.xpi` |
| `{"path":"/tmp/sbrs-check.xpi","temporary":false}` | 500 `unknown error` | `ERROR_SIGNEDSTATE_REQUIRED: The addon must be signed and isn't.` |
| uninstall `{"id":"never-installed@example.org"}` | 500 `unknown error` | `Add-on with ID "never-installed@example.org" is not installed.` |
| `{"temporary":true}` (no `path`) | 400 `invalid argument` | `data did not match any variant of untagged enum AddonInstallParameters` |

`tests/install_addon_firefox.rs` (ignored) runs `BaseCase::install_addon` and
`uninstall_addon` from the host against the geckodriver in this image, with a host
folder mounted at the same path inside the container; the commands are in its
header. It passed on 2026-10-11 against geckodriver 0.37.1 and Firefox 153.4.0 ESR.

The `path` is read by geckodriver, so it has to be a path on geckodriver's own
machine. geckodriver also takes the add-on as base64 in an `addon` field, which
is what Python's Selenium sends so that a remote driver works; this crate does not
send it, and this check did not try it.
