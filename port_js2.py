import sys
sys.path.append("/Users/notlaggy/Documents/GitFolder/copilot-worktrees/SeleniumBase/mustcodeal-fantastic-dollop")
from seleniumbase.js_code.active_css_js import get_active_css_js
from seleniumbase.js_code.live_js import live_js
from seleniumbase.js_code.recorder_js import get_recorder_js

def escape_raw(s):
    # Rust raw string: r#"..."#
    # If the string contains "#, we need more #
    hashes = 1
    while '#' * hashes + '"' in s or '"' + '#' * hashes in s:
        hashes += 1
    h = '#' * hashes
    return f'r{h}"{s}"{h}'

with open("/Users/notlaggy/Documents/GitFolder/seleniumbase-rs/src/js_code/active_css_js.rs", "w") as f:
    f.write('pub fn get_active_css_js() -> String {\n')
    f.write(f'    {escape_raw(get_active_css_js())}.to_string()\n')
    f.write('}\n')

with open("/Users/notlaggy/Documents/GitFolder/seleniumbase-rs/src/js_code/live_js.rs", "w") as f:
    f.write('pub fn get_live_js() -> String {\n')
    f.write(f'    {escape_raw(live_js)}.to_string()\n')
    f.write('}\n')

with open("/Users/notlaggy/Documents/GitFolder/seleniumbase-rs/src/js_code/recorder_js.rs", "w") as f:
    f.write('pub fn get_recorder_js() -> String {\n')
    f.write(f'    {escape_raw(get_recorder_js())}.to_string()\n')
    f.write('}\n')

print("Done JS")
