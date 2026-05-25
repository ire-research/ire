{wake_prompt}

Experiment uuid: {uuid}
Exit code: {exit_code}

stdout tail:
{stdout_tail}

stderr tail:
{stderr_tail}

---
IMPORTANT: If the exit code is 126 (permission denied) or 127 (command not found), do NOT call `experiment.start` again. These errors mean the environment is misconfigured. Report the problem clearly to the user and stop — do not retry.
