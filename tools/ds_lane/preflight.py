"""The brief's two contracts: prompt phrasing and the write set.

Reasonix parses the prompt for constraints, so the wording of a brief decides
whether the worker may write at all; and the write set is what the harness
holds the lane to afterwards. Both are pure text handling - no I/O, no git.
"""
import fnmatch
import re
import sys

# Phrasing law: Reasonix parses the prompt for constraints
# (runtimepolicy.ParseConstraints). Negated-mutation wording outside code
# fences ("do not edit/modify/change", "no changes", "read-only") bans EVERY
# write for the whole session. The preamble therefore states lane rules
# positively; --read-only opts into the ban deliberately, and the preflight
# below refuses a brief whose wording would trigger it by accident.
PREAMBLE = """\
# ds-lane worker rules

You are a worker in an isolated git worktree lane. An orchestrator wrote the
brief below and will review your diff and the raw trajectory of this run.

- Keep all file writes inside the current directory (the lane worktree). The
  sandbox confines writes to it; if something is blocked, report it rather
  than seeking an escalation.
- Git: use only read-side commands (status, diff, log, show). The lane tooling
  records and commits your work after you finish.
- Stop processes only by the PID you recorded or the job id your tools
  returned. A command-line pattern match can hit the harness that started you
  and end the run (lane sw-S1-motavia SIGTERM'd its own worker that way).
- Follow the repository's AGENTS.md. Stay within the brief's scope and touch
  only the files it assigns to you.
- Keep every file you touch under 1,000 lines. When your change would take a
  file over that, reorganize it into cohesive modules as part of the work; the
  harness reports each changed file over the limit in the run summary.
- If you are blocked or the brief is ambiguous, stop and say so in the receipt
  rather than guessing.
- Claim a check passed only if you ran it in this session and saw it pass.
  Report failures verbatim.
- Save any log or scratch output you cite under `build/lane-evidence/` in this
  worktree (git ignores `build/`). Your `/tmp` is private to the sandbox and
  vanishes when the run ends, so evidence cited there cannot be reviewed.

End your final message with exactly this receipt:

## Receipt
- Status: done | blocked | partial
- Changed files: <list>
- Commands run: <command -> exit status, one per line>
- Acceptance: <each acceptance item -> met/not met, with evidence>
- Open issues: <anything unresolved, or none>

---

# Brief

"""

READ_ONLY_CLAUSE = """\
# Lane mode: investigation

This is a read-only investigation lane: do not modify any files. Deliver your
findings in the final message.

"""

CONSTRAINT_BLOCK = "blocked: the current constraints forbid"

# Negated-mutation wording Reasonix's constraint parser reacts to, matched
# case-insensitively against the brief once fenced blocks and inline code are
# removed (Reasonix ignores fenced text, so quoting there is safe).
PHRASING_PATTERNS = [
    r"\b(do not|don't|dont|never|must not|mustn't|shall not)\s+"
    r"(edit|modify|change|touch|write|alter|create|delete|remove|update|rewrite)\b",
    r"\bno (changes|edits|modifications|writes|file changes)\b",
    r"\bread[- ]only\b",
    r"\b(review|analysis|investigation) only\b",
    r"\bwithout (editing|modifying|changing|writing)\b",
    r"\bunchanged\b",
]
PHRASING_RE = re.compile("|".join(f"(?:{p})" for p in PHRASING_PATTERNS), re.IGNORECASE)
FENCE_RE = re.compile(r"^\s*(?:```|~~~)")
INLINE_CODE_RE = re.compile(r"`+[^`]*`+")
WRITE_SET_RE = re.compile(r"^[ \t]*```write-set[ \t]*\n(.*?)^[ \t]*```[ \t]*$", re.S | re.M)


# ------------------------------------------------------- preflight: phrasing

def strip_code(text):
    """The brief's lines with fenced blocks and inline code blanked out.

    Line numbering is preserved so violations can name the brief's own lines.
    """
    out, in_fence = [], False
    for line in text.splitlines():
        if FENCE_RE.match(line):
            in_fence = not in_fence
            out.append("")
        elif in_fence:
            out.append("")
        else:
            out.append(INLINE_CODE_RE.sub(" ", line))
    return out


def phrasing_violations(text):
    """[(line number, line)] of negated-mutation wording outside code."""
    return [(i, line) for i, line in enumerate(strip_code(text), 1)
            if PHRASING_RE.search(line)]


def preflight_phrasing(text, what, allow=False):
    if allow:
        return
    bad = phrasing_violations(text)
    if not bad:
        return
    lines = [f"ds-lane: {what} failed the phrasing preflight: negated-mutation wording "
             f"bans all writes for the session (Reasonix prompt constraints)."]
    lines += [f"  line {n}: {line.strip()}" for n, line in bad]
    lines.append('hint: state file ownership positively ("touch only X"), or keep quoted '
                 "wording inside a fenced block or inline code. --read-only and --allow-phrasing "
                 "skip this check.")
    sys.exit("\n".join(lines))


# --------------------------------------------------------- write set (globs)

def parse_write_set(brief):
    """Paths/globs from the brief's ```write-set block; None when absent."""
    m = WRITE_SET_RE.search(brief)
    if not m:
        return None
    pats = [l.strip() for l in m.group(1).splitlines() if l.strip()]
    return pats or None


def _glob_re(pat):
    """fnmatch, with `**` also matching across directory separators."""
    out, i = "", 0
    while i < len(pat):
        c = pat[i]
        if c == "*":
            if pat.startswith("**", i):
                i += 2
                if pat.startswith("/", i):  # `**/` also matches zero directories
                    i += 1
                    out += "(?:.*/)?"
                else:
                    out += ".*"
                continue
            out += "[^/]*"
        elif c == "?":
            out += "[^/]"
        else:
            out += re.escape(c)
        i += 1
    return re.compile(out + r"\Z")


def write_set_match(path, patterns):
    return any(fnmatch.fnmatch(path, p) or _glob_re(p).match(path) for p in patterns)


def write_set_violations(paths, patterns):
    return [p for p in paths if not write_set_match(p, patterns)]
