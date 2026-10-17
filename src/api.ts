import { invoke, isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { Home } from "./types";
export const desktop = isTauri();
export async function call<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  if (desktop) return invoke<T>(command, args);
  // Browser development is an explicit UI preview, with no filesystem bridge.
  if (command === "home")
    return { game: null, installations: [], platform: "preview" } as Home as T;
  throw new Error(
    "Open the desktop application to use game files. This browser is an interface preview.",
  );
}
export async function folder(title: string): Promise<string | null> {
  if (!desktop) return null;
  return (await open({ directory: true, multiple: false, title })) as
    | string
    | null;
}
