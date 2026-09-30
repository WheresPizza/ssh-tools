import type { GitProfile } from "./tauri";

export function cloneUrl(profile: GitProfile, repository: string): string | null {
  const path = repository.trim();
  if (!path || path.startsWith("/") || path.startsWith("-") || !/^[A-Za-z0-9_./-]+$/.test(path) || path.split("/").some(part => !part || part === "." || part === "..")) return null;
  return `${profile.user}@${profile.alias}:${path.endsWith(".git") ? path : `${path}.git`}`;
}
