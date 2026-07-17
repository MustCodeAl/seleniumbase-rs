import re

with open("/Users/notlaggy/Documents/GitFolder/copilot-worktrees/SeleniumBase/mustcodeal-fantastic-dollop/seleniumbase/config/settings.py", "r") as f:
    text = f.read()

rust_code = ""
for line in text.splitlines():
    match = re.match(r'^([A-Z0-9_]+)\s*=\s*(.*?)\s*(#.*)?$', line.strip())
    if match:
        var_name = match.group(1)
        val = match.group(2)
        # determine type
        if val in ["True", "False"]:
            rust_code += f"pub const {var_name}: bool = {val.lower()};\n"
        elif val.isdigit():
            rust_code += f"pub const {var_name}: u64 = {val};\n"
        elif val.replace('.', '', 1).isdigit():
            rust_code += f"pub const {var_name}: f64 = {val};\n"
        elif val.startswith('"') or val.startswith("'"):
            # String const
            val = val.replace("'", '"')
            rust_code += f"pub const {var_name}: &'static str = {val};\n"

if rust_code:
    with open("/Users/notlaggy/Documents/GitFolder/seleniumbase-rs/src/config/settings.rs", "w") as out:
        out.write(rust_code)

print("Settings done")
