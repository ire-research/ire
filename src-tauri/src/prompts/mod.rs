//! Centralised prompt registry. All CC-facing prompt text lives in
//! `assets/prompts/` and is embedded at build time. Edit the `.md` files
//! to change CC's behaviour — never hardcode prompts elsewhere.

const RESOURCE_SUMMARIZER: &str = include_str!("../../assets/prompts/resource_summarizer.md");
const EXPERIMENT_WAKEUP: &str = include_str!("../../assets/prompts/experiment_wakeup.md");
const CHAT_TITLE: &str = include_str!("../../assets/prompts/chat_title.md");

pub fn resource_summarizer() -> &'static str {
    RESOURCE_SUMMARIZER.trim_end()
}

/// Prompt for the lightweight model that names a new chat from its first message.
pub fn chat_title(user_message: &str) -> String {
    CHAT_TITLE
        .trim_end()
        .replace("{user_message}", user_message)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_title_substitutes_user_message() {
        let out = chat_title("how do transformers work?");
        assert!(out.contains("how do transformers work?"));
        assert!(!out.contains("{user_message}"));
    }
}
