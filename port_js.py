import os
import glob

py_dir = "/Users/notlaggy/Documents/GitFolder/copilot-worktrees/SeleniumBase/mustcodeal-fantastic-dollop/seleniumbase/js_code"
rs_dir = "/Users/notlaggy/Documents/GitFolder/seleniumbase-rs/src/js_code"

for py_file in glob.glob(f"{py_dir}/*.py"):
    if "__init__" in py_file: continue
    
    with open(py_file, 'r', encoding='utf-8') as f:
        content = f.read()
        
    # Extract the variable containing the JS string
    # Usually it's something like `live_js = r"""..."""`
    
    lines = content.splitlines()
    js_content = ""
    var_name = ""
    in_string = False
    for line in lines:
        if not in_string and ('= r"""' in line or '= """' in line or "= r'''" in line or "= '''" in line):
            in_string = True
            var_name = line.split('=')[0].strip()
            continue
        elif not in_string and ('= r"' in line):
            pass # might be single line
        if in_string:
            if '"""' in line or "'''" in line:
                in_string = False
                # capture before the closing quotes
                js_content += line.replace('"""', '').replace("'''", '') + "\n"
            else:
                js_content += line + "\n"

    # Some of the py files have functions returning string, or just pure string definitions. Let's just do a simpler replacement or write a raw rust file.
    
    # Better approach: parse the variable assignments
