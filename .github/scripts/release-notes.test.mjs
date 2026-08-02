import assert from "node:assert/strict";
import test from "node:test";

import { buildNotes, parseCommit } from "./release-notes.mjs";

const REPO = "MegalithOfficial/basalt-launcher";

test("reads the type, scope and summary out of a conventional subject", () => {
  assert.deepEqual(parseCommit("abc1234567 feat(instances): organize instances into groups"), {
    hash: "abc1234567",
    type: "feat",
    scope: "instances",
    breaking: false,
    summary: "organize instances into groups",
  });
  assert.deepEqual(parseCommit("def7654321 fix!: drop the legacy launch path"), {
    hash: "def7654321",
    type: "fix",
    scope: null,
    breaking: true,
    summary: "drop the legacy launch path",
  });
  assert.deepEqual(parseCommit("aaa1111111 tidy up the sidebar"), {
    hash: "aaa1111111",
    type: null,
    scope: null,
    breaking: false,
    summary: "tidy up the sidebar",
  });
});

test("groups commits and links each one back to its hash", () => {
  const notes = buildNotes({
    lines: [
      "1111111aaaa feat(instances): organize instances into groups",
      "2222222bbbb fix(window): resize from the edges",
      "3333333cccc style(settings): drop the install card",
      "4444444dddd chore(release): bump version to 1.0.0",
      "5555555eeee docs(readme): show the current experience",
    ],
    repository: REPO,
    tag: "v1.0.1",
    previousTag: "v1.0.0",
  });

  assert.match(notes, /### New\n- \*\*instances\*\*: organize instances into groups/);
  assert.match(notes, /### Fixed\n- \*\*window\*\*: resize from the edges/);
  assert.match(notes, /### Changed\n- \*\*settings\*\*: drop the install card/);
  assert.match(notes, /### Documentation\n- \*\*readme\*\*: show the current experience/);
  assert.doesNotMatch(notes, /bump version/);
  assert.match(notes, /\[`1111111`\]\(https:\/\/github\.com\/.+\/commit\/1111111aaaa\)/);
  assert.match(notes, /compare\/v1\.0\.0\.\.\.v1\.0\.1/);
});

test("puts breaking changes first and keeps unlabelled commits", () => {
  const notes = buildNotes({
    lines: [
      "1111111aaaa feat(launch)!: require Java 21",
      "2222222bbbb rewrote the loader picker",
    ],
    repository: REPO,
    tag: "v2.0.0",
    previousTag: "v1.9.0",
  });

  assert.ok(notes.indexOf("### Breaking") < notes.indexOf("### Other"));
  assert.match(notes, /### Breaking\n- \*\*launch\*\*: require Java 21/);
  assert.match(notes, /### Other\n- rewrote the loader picker/);
});

test("says so when a build carries nothing user facing", () => {
  const notes = buildNotes({
    lines: ["1111111aaaa chore(ci): retry flaky uploads"],
    repository: REPO,
    tag: "v1.0.1",
    previousTag: "v1.0.0",
  });

  assert.match(notes, /No user facing changes in this build\./);
});
