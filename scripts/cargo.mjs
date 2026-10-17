import { spawn } from "node:child_process";
import { root, environment } from "./environment.mjs";
const child = spawn("cargo", process.argv.slice(2), {
  cwd: root,
  env: environment(),
  stdio: "inherit",
});
child.on("error", (error) => {
  console.error(error.message);
  process.exitCode = 1;
});
child.on("exit", (code) => {
  process.exitCode = code ?? 1;
});
