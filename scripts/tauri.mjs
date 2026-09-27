import { spawn } from "node:child_process";
import { loadEnvFile } from "node:process";
import { resolve } from "node:path";

try {
  loadEnvFile(resolve(process.cwd(), ".env"));
} catch (error) {
  if (error.code !== "ENOENT") throw error;
}

const tauriCli = resolve(process.cwd(), "node_modules/@tauri-apps/cli/tauri.js");
const child = spawn(process.execPath, [tauriCli, ...process.argv.slice(2)], {
  stdio: "inherit",
  env: process.env,
});

child.on("error", (error) => {
  console.error(`Could not start Tauri CLI: ${error.message}`);
  process.exitCode = 1;
});
child.on("exit", (code, signal) => {
  process.exitCode = code ?? (signal ? 1 : 0);
});
