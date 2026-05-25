//! Centralised prompt registry. All CC-facing prompt text lives in
//! `assets/prompts/` and is embedded at build time. Edit the `.md` files
//! to change CC's behaviour — never hardcode prompts elsewhere.

const RESOURCE_SUMMARIZER: &str = include_str!("../../assets/prompts/resource_summarizer.md");
const RESOURCE_CONFIRM: &str = include_str!("../../assets/prompts/resource_confirm.md");
const EXPERIMENT_WAKEUP: &str = include_str!("../../assets/prompts/experiment_wakeup.md");

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
    pub stdout_tail: &'a str,
    pub stderr_tail: &'a str,
}

pub fn experiment_wakeup(args: WakeupArgs<'_>) -> String {
    EXPERIMENT_WAKEUP
        .replace("{wake_prompt}", args.wake_prompt)
        .replace("{uuid}", args.uuid)
        .replace("{exit_code}", &args.exit_code.to_string())
        .replace("{stdout_tail}", args.stdout_tail)
        .replace("{stderr_tail}", args.stderr_tail)
}
