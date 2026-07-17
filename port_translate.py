import glob
import re

py_dir = "/Users/notlaggy/Documents/GitFolder/copilot-worktrees/SeleniumBase/mustcodeal-fantastic-dollop/seleniumbase/translate"
rs_dir = "/Users/notlaggy/Documents/GitFolder/seleniumbase-rs/src/translate"

def camel_to_snake(name):
    s1 = re.sub('(.)([A-Z][a-z]+)', r'\1_\2', name)
    return re.sub('([a-z0-9])([A-Z])', r'\1_\2', s1).lower()

for py_file in glob.glob(f"{py_dir}/*.py"):
    if "__init__" in py_file or "master_dict" in py_file or "translator" in py_file: continue
    
    with open(py_file, 'r', encoding='utf-8') as f:
        text = f.read()

    # The python classes look like:
    # class English(object):
    #     step_1 = "..."
    #     step_2 = "..."
    
    lines = text.splitlines()
    rust_code = ""
    for line in lines:
        match = re.match(r'^\s+([a-zA-Z0-9_]+)\s*=\s*(["\'])(.*?)\2', line)
        if match:
            var_name = match.group(1)
            var_val = match.group(3).replace('"', '\\"')
            rust_code += f"pub fn {var_name}() -> &'static str {{ \"{var_val}\" }}\n"
            
    lang_name = py_file.split('/')[-1].replace('.py', '.rs')
    if rust_code:
        with open(f"{rs_dir}/{lang_name}", "w") as out:
            out.write(rust_code)

print("Done translating")
