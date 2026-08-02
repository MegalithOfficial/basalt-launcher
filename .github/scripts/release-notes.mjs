import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const SECTIONS = [
  { title: "New", types: ["feat"] },
  { title: "Fixed", types: ["fix"] },
  { title: "Changed", types: ["refactor", "style", "perf"] },
  { title: "Documentation", types: ["docs"] },
];

const IGNORED = new Set(["chore", "ci", "build", "test", "release"]);

const CONVENTIONAL = /^([a-z]+)(?:\(([^)]*)\))?(!)?:\s*(.+)$/i;

export function parseCommit(line) {
  const [hash, ...rest] = line.split(" ");
  const subject = rest.join(" ").trim();
  if (!hash || !subject) return null;
  const match = CONVENTIONAL.exec(subject);
  if (!match) return { hash, type: null, scope: null, breaking: false, summary: subject };
  const [, type, scope, bang, summary] = match;
  return {
    hash,
    type: type.toLowerCase(),
    scope: scope?.trim() || null,
    breaking: bang === "!",
    summary: summary.trim(),
  };
}

function entry(commit, repository) {
  const scope = commit.scope ? `**${commit.scope}**: ` : "";
  const link = `[\`${commit.hash.slice(0, 7)}\`](https://github.com/${repository}/commit/${commit.hash})`;
  return `- ${scope}${commit.summary} ${link}`;
}

export function buildNotes({ lines, repository, tag, previousTag }) {
  const commits = lines
    .map((line) => parseCommit(line))
    .filter((commit) => commit !== null)
    .filter((commit) => !(commit.type && IGNORED.has(commit.type)));

  const blocks = [];
  const breaking = commits.filter((commit) => commit.breaking);
  if (breaking.length > 0) {
    blocks.push(`### Breaking\n${breaking.map((c) => entry(c, repository)).join("\n")}`);
  }

  for (const section of SECTIONS) {
    const matched = commits.filter(
      (commit) => !commit.breaking && commit.type && section.types.includes(commit.type),
    );
    if (matched.length === 0) continue;
    blocks.push(`### ${section.title}\n${matched.map((c) => entry(c, repository)).join("\n")}`);
  }

  const known = new Set(SECTIONS.flatMap((section) => section.types));
  const other = commits.filter(
    (commit) => !commit.breaking && (!commit.type || !known.has(commit.type)),
  );
  if (other.length > 0) {
    blocks.push(`### Other\n${other.map((c) => entry(c, repository)).join("\n")}`);
  }

  if (blocks.length === 0) {
    blocks.push("No user facing changes in this build.");
  }

  if (previousTag) {
    blocks.push(
      `**Full changelog**: [\`${previousTag}...${tag}\`](https://github.com/${repository}/compare/${previousTag}...${tag})`,
    );
  }

  return blocks.join("\n\n");
}

function git(args) {
  return execFileSync("git", args, { encoding: "utf8" }).trim();
}

export function previousReleaseTag(tag) {
  const tags = git(["tag", "--list", "v*", "--sort=-creatordate"])
    .split("\n")
    .map((value) => value.trim())
    .filter((value) => value.length > 0 && value !== tag);
  return tags[0] ?? null;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [repository, tag] = process.argv.slice(2);
  if (!repository || !tag) {
    console.error("usage: release-notes.mjs <repository> <tag>");
    process.exit(1);
  }
  const previousTag = previousReleaseTag(tag);
  const range = previousTag ? `${previousTag}..HEAD` : "HEAD";
  const lines = git(["log", range, "--no-merges", "--pretty=format:%H %s"])
    .split("\n")
    .filter((line) => line.trim().length > 0);
  process.stdout.write(buildNotes({ lines, repository, tag, previousTag }));
}
