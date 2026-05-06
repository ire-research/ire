//! Centralised prompt registry. All CC-facing prompt text lives in
//! `assets/prompts/` and is embedded at build time. Edit the `.md` files
//! to change CC's behaviour — never hardcode prompts elsewhere.

const MODE_BRAINSTORM: &str = include_str!("../../assets/prompts/mode_brainstorm.md");
const MODE_EXPERIMENT: &str = include_str!("../../assets/prompts/mode_experiment.md");
const RESOURCE_SUMMARIZER: &str = include_str!("../../assets/prompts/resource_summarizer.md");
const RESOURCE_CONFIRM: &str = include_str!("../../assets/prompts/resource_confirm.md");
const EXPERIMENT_WAKEUP: &str = include_str!("../../assets/prompts/experiment_wakeup.md");

pub fn mode_preamble(mode: &str) -> &'static str {
    match mode {
        "experiment" => MODE_EXPERIMENT.trim_end(),
        _ => MODE_BRAINSTORM.trim_end(),
    }
}

pub fn resource_summarizer() -> &'static str {
    RESOURCE_SUMMARIZER.trim_end()
}

pub fn resource_confirm() -> &'static str {
    RESOURCE_CONFIRM.trim_end()
}

pub struct WakeupArgs<'a> {
    pub wake_prompt: &'a str,
    pub uuid: &'a str,
    pub exit_code: i32,
    pub plan_path: &'a str,
    pub stdout_tail: &'a str,
    pub stderr_tail: &'a str,
}

pub fn experiment_wakeup(args: WakeupArgs<'_>) -> String {
    EXPERIMENT_WAKEUP
        .replace("{wake_prompt}", args.wake_prompt)
        .replace("{uuid}", args.uuid)
        .replace("{exit_code}", &args.exit_code.to_string())
        .replace("{plan_path}", args.plan_path)
        .replace("{stdout_tail}", args.stdout_tail)
        .replace("{stderr_tail}", args.stderr_tail)
}
