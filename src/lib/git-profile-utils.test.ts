import { describe, it, expect } from "vitest";
import { cloneUrl } from "./git-profile-utils";
import type { GitProfile } from "./tauri";
const profile: GitProfile = { name: "Work", account: "work", alias: "git-work", hostname: "github.com", user: "git", port: 2222, key_path: "/fixture/key" };
describe("profile clone URL", () => {
  it("uses the alias so SSH selects the profile key and port", () => expect(cloneUrl(profile, "team/repo")).toBe("git@git-work:team/repo.git"));
  it("preserves GitLab subgroups and existing suffixes", () => expect(cloneUrl(profile, "group/sub/repo.git")).toBe("git@git-work:group/sub/repo.git"));
  it("rejects command fragments and traversal", () => { for (const value of ["", "-x", "../repo", "a/../b", "a//b", "a;echo bad", "https://github.com/repo", "/a"]) expect(cloneUrl(profile, value)).toBeNull(); });
});
