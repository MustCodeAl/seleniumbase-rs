# A throwaway check of geckodriver's add-on commands (moz/addon/install and
# moz/addon/uninstall), which BaseCase::install_addon and uninstall_addon send.
# It holds firefox-esr and geckodriver and nothing of this crate: check.sh drives
# geckodriver with curl, so the Rust request is compared with what a real
# geckodriver accepts. See README.md for how to build and run it.
#
# Build with the local wrapper's --no-slim: Slim would strip the browser.
# No EXPOSE: the wrapper would take this for a web app and probe the port.
FROM debian:bookworm-slim

# geckodriver's linux-aarch64 release and the SHA-256 GitHub lists for that
# asset (`gh release view v0.37.1 --repo mozilla/geckodriver --json assets`).
# Move both together.
ARG GECKODRIVER_VERSION=0.37.1
ARG GECKODRIVER_SHA256=8fd90b951422fbad5b56539fb344dff66eb3f986d615d8d6fde9f1f62dad610c

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
        ca-certificates curl firefox-esr jq python3 zip \
 && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL -o /tmp/geckodriver.tar.gz \
        "https://github.com/mozilla/geckodriver/releases/download/v${GECKODRIVER_VERSION}/geckodriver-v${GECKODRIVER_VERSION}-linux-aarch64.tar.gz" \
 && echo "${GECKODRIVER_SHA256}  /tmp/geckodriver.tar.gz" | sha256sum -c - \
 && tar -xzf /tmp/geckodriver.tar.gz -C /usr/local/bin geckodriver \
 && rm /tmp/geckodriver.tar.gz \
 && geckodriver --version \
 && firefox-esr --version

COPY addon/ /opt/addon/
COPY check.sh /usr/local/bin/check.sh

RUN useradd --create-home checker && chmod 0555 /usr/local/bin/check.sh
USER checker
WORKDIR /home/checker
CMD ["/usr/local/bin/check.sh"]
