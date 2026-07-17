import re
import sys

def escape_raw(s):
    hashes = 1
    while '#' * hashes + '"' in s or '"' + '#' * hashes in s:
        hashes += 1
    h = '#' * hashes
    return f'r{h}"{s}"{h}'

# live_js.py
with open("/Users/notlaggy/Documents/GitFolder/copilot-worktrees/SeleniumBase/mustcodeal-fantastic-dollop/seleniumbase/js_code/live_js.py", "r") as f:
    text = f.read()
    match = re.search(r'live_js\s*=\s*r"""(.*?)"""', text, re.DOTALL)
    if match:
        val = match.group(1)
        with open("/Users/notlaggy/Documents/GitFolder/seleniumbase-rs/src/js_code/live_js.rs", "w") as out:
            out.write('pub fn get_live_js() -> String {\n')
            out.write(f'    {escape_raw(val)}.to_string()\n')
            out.write('}\n')

# active_css_js.py
with open("/Users/notlaggy/Documents/GitFolder/copilot-worktrees/SeleniumBase/mustcodeal-fantastic-dollop/seleniumbase/js_code/active_css_js.py", "r") as f:
    text = f.read()
    match = re.search(r'def get_active_css_js\(\):\s*return r"""(.*?)"""', text, re.DOTALL)
    if match:
        val = match.group(1)
        with open("/Users/notlaggy/Documents/GitFolder/seleniumbase-rs/src/js_code/active_css_js.rs", "w") as out:
            out.write('pub fn get_active_css_js() -> String {\n')
            out.write(f'    {escape_raw(val)}.to_string()\n')
            out.write('}\n')

# recorder_js.py
with open("/Users/notlaggy/Documents/GitFolder/copilot-worktrees/SeleniumBase/mustcodeal-fantastic-dollop/seleniumbase/js_code/recorder_js.py", "r") as f:
    text = f.read()
    match = re.search(r'def get_recorder_js\(\):\s*return r"""(.*?)"""', text, re.DOTALL)
    if match:
        val = match.group(1)
        with open("/Users/notlaggy/Documents/GitFolder/seleniumbase-rs/src/js_code/recorder_js.rs", "w") as out:
            out.write('pub fn get_recorder_js() -> String {\n')
            out.write(f'    {escape_raw(val)}.to_string()\n')
            out.write('}\n')

print("Done JS parsing")
