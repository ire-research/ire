{wake_prompt}

Experiment uuid: {uuid}
Exit code: {exit_code}
Experiment folder: {record_dir}/ — read `EXPERIMENT.md` there for the original goal, and write any result files, notes, or scripts worth keeping into that folder.

stdout tail:
{stdout_tail}

stderr tail:
{stderr_tail}

---
IMPORTANT: If the exit code is 126 (permission denied) or 127 (command not found), do NOT call `experiment.start` again. These errors mean the environment is misconfigured. Report the problem clearly to the user and stop — do not retry.

Once you've actually looked at the result and discussed it (or reported it), call `experiment.mark_reviewed` with this uuid. Opening this chat doesn't mark it reviewed by itself — that's on you to do once you're done.
