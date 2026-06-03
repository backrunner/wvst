import { createReadStream, existsSync, statSync } from "node:fs";
import http from "node:http";
import { extname, join, normalize, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const example = process.argv[2] ?? "effect";
const port = Number(process.env.PORT ?? 5179);
const host = process.env.HOST ?? "127.0.0.1";
const packageRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const repoRoot = resolve(packageRoot, "../..");
const startPath = `/packages/wvst-web-examples/public/${example}/index.html`;

const server = http.createServer((request, response) => {
  const url = new URL(request.url ?? "/", `http://${host}:${port}`);
  const pathname = url.pathname === "/" ? startPath : url.pathname;
  const file = resolvePath(pathname);
  if (!file) {
    response.writeHead(403);
    response.end("Forbidden");
    return;
  }
  if (!existsSync(file) || !statSync(file).isFile()) {
    response.writeHead(404);
    response.end("Not found");
    return;
  }

  response.writeHead(200, {
    "Content-Type": contentType(file),
    "Cross-Origin-Embedder-Policy": "require-corp",
    "Cross-Origin-Opener-Policy": "same-origin",
  });
  createReadStream(file).pipe(response);
});

server.listen(port, host, () => {
  console.log(`WVST ${example} example: http://${host}:${port}${startPath}`);
});

function resolvePath(pathname) {
  const cleanPath = normalize(decodeURIComponent(pathname)).replace(/^[/\\]+/, "");
  const file = resolve(repoRoot, cleanPath);
  return file === repoRoot || file.startsWith(`${repoRoot}${sep}`) ? file : null;
}

function contentType(file) {
  switch (extname(file)) {
    case ".css":
      return "text/css";
    case ".html":
      return "text/html";
    case ".js":
      return "text/javascript";
    case ".json":
      return "application/json";
    default:
      return "application/octet-stream";
  }
}
