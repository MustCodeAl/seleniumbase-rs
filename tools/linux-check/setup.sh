#!/bin/sh
# Clones the commit under test into the volume and links the drivers the way the Dockerfile does.
set -e
commit="$1"
if [ ! -d /work/.git ]; then
  git clone --quiet https://github.com/MustCodeAl/seleniumbase-rs /work
fi
cd /work
git fetch --quiet origin '+refs/heads/*:refs/remotes/origin/*'
git checkout --quiet --force "$commit"
for dir in . crates/cli crates/playwright; do
  mkdir -p "$dir/downloaded_drivers"
  ln -sf /usr/bin/chromedriver "$dir/downloaded_drivers/chromedriver"
done
git log -1 --format='%H %s'
chromium --version
chromedriver --version
rustc --version
