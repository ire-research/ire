{wake_prompt}

Experiment `{uuid}` finished with exit code {exit_code}.

Read `{record_dir}/EXPERIMENT.md` for the original goal and run details, and look through that folder for result files. For the run's output, call `experiment.tail_logs` with this uuid (raw stdout/stderr live in `.ire/cache/experiments/{uuid}/`). Write anything worth keeping back into the experiment folder.

If the exit code is 126 (permission denied) or 127 (command not found), do NOT call `experiment.start` again — the environment is misconfigured. Report the problem to the user and stop.

Once you've actually looked at the result and discussed it (or reported it), call `experiment.mark_reviewed` with this uuid. Receiving this message doesn't mark it reviewed by itself.
