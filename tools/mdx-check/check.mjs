// Builds every page of docs/ the way the Docs7 site does and fails on what breaks it there.
//
// - The page compiles as MDX (with GFM and front matter).
// - Every name an expression uses is defined: by the page's own `export const`, by an
//   `import`, as a parameter, or as a JavaScript global. `{`...`}` and `{" "}` are
//   fine; `{browser}` with no `browser` defined fails the page on Docs7.
// - No relative link to a `.md` file: Docs7 serves those raw. Use the site route
//   (`/tutorials/selectors`), in Markdown links and in component `href`s alike.
// - The front matter has a title.
// - Every page is in the navigation of docs.json, and every page there exists.
//
// Usage: just mdx  (or: bun tools/mdx-check/check.mjs docs)
import { compile } from "@mdx-js/mdx";
import remarkFrontmatter from "remark-frontmatter";
import remarkGfm from "remark-gfm";
import { visit } from "unist-util-visit";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const root = process.argv[2] ?? "docs";
const SKIP = new Set(["SUMMARY.md"]); // mdBook's table of contents, not a Docs7 page
const SKIP_DIRS = new Set(["node_modules", "book", "images", "logo"]);
const GLOBALS = new Set([
  "undefined", "NaN", "Infinity", "Math", "JSON", "Date", "Array", "Object", "String",
  "Number", "Boolean", "RegExp", "Map", "Set", "Promise", "Symbol", "console", "window",
  "document", "props", "React", "encodeURIComponent", "decodeURIComponent", "parseInt",
  "parseFloat", "isNaN",
]);
const RELATIVE_MD = /^(?!https?:|mailto:|\/|#)[^\s]*\.mdx?(#.*)?$/;

function pages(dir, out = []) {
  for (const name of readdirSync(dir).sort()) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) {
      if (!SKIP_DIRS.has(name)) pages(path, out);
    } else if ((name.endsWith(".md") || name.endsWith(".mdx")) && !SKIP.has(relative(root, path))) {
      out.push(path);
    }
  }
  return out;
}

// Names bound by a pattern: `x`, `{ a, b: c }`, `[d, ...e]`, `f = 1`.
function patternNames(pattern, out = new Set()) {
  if (!pattern) return out;
  switch (pattern.type) {
    case "Identifier": out.add(pattern.name); break;
    case "ObjectPattern":
      for (const p of pattern.properties) patternNames(p.type === "RestElement" ? p.argument : p.value, out);
      break;
    case "ArrayPattern": for (const e of pattern.elements) patternNames(e, out); break;
    case "RestElement": patternNames(pattern.argument, out); break;
    case "AssignmentPattern": patternNames(pattern.left, out); break;
  }
  return out;
}

// The free names of an estree node, given the names in scope.
function freeNames(node, scope, out) {
  if (!node || typeof node !== "object") return;
  if (Array.isArray(node)) { for (const n of node) freeNames(n, scope, out); return; }
  switch (node.type) {
    case "Identifier":
      if (!scope.has(node.name) && !GLOBALS.has(node.name)) out.add(node.name);
      return;
    case "MemberExpression":
      freeNames(node.object, scope, out);
      if (node.computed) freeNames(node.property, scope, out);
      return;
    case "Property":
      if (node.computed) freeNames(node.key, scope, out);
      freeNames(node.value, scope, out);
      return;
    case "ArrowFunctionExpression":
    case "FunctionExpression":
    case "FunctionDeclaration": {
      const inner = new Set(scope);
      if (node.id) inner.add(node.id.name);
      for (const p of node.params) patternNames(p, inner);
      collectDeclarations(node.body, inner);
      for (const p of node.params) if (p.type === "AssignmentPattern") freeNames(p.right, inner, out);
      freeNames(node.body, inner, out);
      return;
    }
    case "VariableDeclarator":
      freeNames(node.init, scope, out);
      return;
    case "JSXIdentifier": case "JSXMemberExpression": case "JSXNamespacedName":
      return; // component and attribute names: Docs7 provides the components
    case "JSXAttribute":
      freeNames(node.value, scope, out);
      return;
    case "TemplateElement": case "Literal":
      return;
  }
  for (const key of Object.keys(node)) {
    if (key === "type" || key === "start" || key === "end" || key === "loc" || key === "range" || key === "comments") continue;
    freeNames(node[key], scope, out);
  }
}

// `const`/`let`/`function` declared anywhere in a block, for the scope of that block.
function collectDeclarations(node, scope) {
  if (!node || typeof node !== "object") return;
  if (Array.isArray(node)) { for (const n of node) collectDeclarations(n, scope); return; }
  if (node.type === "VariableDeclarator") patternNames(node.id, scope);
  if (node.type === "FunctionDeclaration" && node.id) scope.add(node.id.name);
  if (node.type.endsWith("FunctionExpression") || node.type === "ArrowFunctionExpression") return;
  for (const key of Object.keys(node)) {
    if (key === "type" || key === "loc" || key === "range") continue;
    if (node[key] && typeof node[key] === "object") collectDeclarations(node[key], scope);
  }
}

function checkTree(problems) {
  return () => (tree) => {
    const defined = new Set();
    visit(tree, "mdxjsEsm", (node) => {
      for (const statement of node.data?.estree?.body ?? []) {
        if (statement.type === "ImportDeclaration") {
          for (const s of statement.specifiers) defined.add(s.local.name);
        }
        const declaration = statement.type === "ExportNamedDeclaration" ? statement.declaration : statement;
        if (declaration?.type === "VariableDeclaration") {
          for (const d of declaration.declarations) patternNames(d.id, defined);
        } else if (declaration?.type === "FunctionDeclaration" && declaration.id) {
          defined.add(declaration.id.name);
        }
      }
    });
    const at = (node) => `${node.position?.start?.line ?? "?"}:${node.position?.start?.column ?? "?"}`;
    const report = (node, estree, what) => {
      const free = new Set();
      freeNames(estree, defined, free);
      for (const name of free) problems.push(`${at(node)}: ${what} uses \`${name}\`, which is not defined (MDX evaluates braces outside code; write \\{ \\} or put it in backticks)`);
    };
    visit(tree, (node) => {
      if ((node.type === "mdxFlowExpression" || node.type === "mdxTextExpression") && node.data?.estree) {
        report(node, node.data.estree, "an expression");
      }
      if (node.type === "mdxJsxFlowElement" || node.type === "mdxJsxTextElement") {
        for (const attribute of node.attributes ?? []) {
          if (attribute.type === "mdxJsxAttribute" && typeof attribute.value === "string"
              && attribute.name === "href" && RELATIVE_MD.test(attribute.value)) {
            problems.push(`${at(node)}: href="${attribute.value}" opens the raw file on Docs7; use the site route (/path/page)`);
          }
          const estree = attribute.value?.data?.estree ?? attribute.data?.estree;
          if (estree) report(node, estree, `the attribute ${attribute.name ?? "{...}"}`);
        }
      }
      if ((node.type === "link" || node.type === "definition") && RELATIVE_MD.test(node.url ?? "")) {
        problems.push(`${at(node)}: link (${node.url}) opens the raw file on Docs7; use the site route (/path/page)`);
      }
    });
  };
}

function navPages(config) {
  const found = new Set();
  const walk = (value) => {
    if (Array.isArray(value)) value.forEach(walk);
    else if (value && typeof value === "object") {
      for (const [key, inner] of Object.entries(value)) {
        if (key === "pages" && Array.isArray(inner)) {
          for (const page of inner) typeof page === "string" ? found.add(page) : walk(page);
        } else walk(inner);
      }
    }
  };
  walk(config.navigation);
  return found;
}

let bad = 0;
const files = pages(root);
for (const file of files) {
  const raw = readFileSync(file, "utf8");
  const problems = [];
  const front = raw.match(/^---\n([\s\S]*?)\n---\n/);
  if (!front || !/^title:\s*\S/m.test(front[1])) problems.push("1:1: the page has no title in its front matter");
  try {
    await compile({ value: raw, path: file }, {
      format: file.endsWith(".mdx") ? "mdx" : "mdx",
      remarkPlugins: [remarkFrontmatter, remarkGfm, checkTree(problems)],
    });
  } catch (e) {
    const line = e.line ?? e.place?.line ?? e.position?.start?.line ?? "?";
    const column = e.column ?? e.place?.column ?? e.position?.start?.column ?? "?";
    problems.push(`${line}:${column}: does not compile: ${String(e.reason ?? e.message).split("\n")[0]}`);
  }
  if (problems.length) {
    bad++;
    for (const p of problems) console.log(`${file}:${p}`);
  }
}

const configPath = join(root, "docs.json");
let navBad = 0;
if (existsSync(configPath)) {
  const nav = navPages(JSON.parse(readFileSync(configPath, "utf8")));
  const onDisk = new Set(files.map((f) => relative(root, f).replace(/\.mdx?$/, "")));
  for (const page of [...onDisk].sort()) {
    if (!nav.has(page)) { navBad++; console.log(`${configPath}: ${page} is in no menu of the navigation`); }
  }
  for (const page of [...nav].sort()) {
    if (!onDisk.has(page)) { navBad++; console.log(`${configPath}: the navigation lists ${page}, which has no file`); }
  }
}

console.log(`${files.length} pages checked, ${bad} with errors${navBad ? `, ${navBad} navigation problems` : ""}`);
process.exit(bad || navBad ? 1 : 0);
