use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchOptions {
    pub profile_name: Option<String>,
    pub launch_game: bool,
}

impl LaunchOptions {
    pub fn from_env_args() -> Self {
        parse_launch_options(std::env::args().skip(1))
    }
}

fn parse_launch_options<I>(args: I) -> LaunchOptions
where
    I: IntoIterator<Item = String>,
{
    let mut options = LaunchOptions::default();
    let mut iter = args.into_iter().peekable();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-profile" | "--profile" => {
                if let Some(value) = iter.next() {
                    let trimmed = value.trim();
                    if !trimmed.is_empty() {
                        options.profile_name = Some(trimmed.to_string());
                    }
                }
            }
            "-launchgame" | "--launchgame" => {
                options.launch_game = true;
            }
            _ => {}
        }
    }

    options
}

#[tauri::command]
pub fn get_startup_launch_options(options: tauri::State<'_, LaunchOptions>) -> LaunchOptions {
    options.inner().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_profile_and_launchgame_flags() {
        let options = parse_launch_options([
            "-profile".to_string(),
            "Survival".to_string(),
            "-launchgame".to_string(),
        ]);
        assert_eq!(options.profile_name.as_deref(), Some("Survival"));
        assert!(options.launch_game);
    }

    #[test]
    fn parses_long_form_flags() {
        let options = parse_launch_options([
            "--profile".to_string(),
            "My Pack".to_string(),
            "--launchgame".to_string(),
        ]);
        assert_eq!(options.profile_name.as_deref(), Some("My Pack"));
        assert!(options.launch_game);
    }

    #[test]
    fn parses_launchgame_only() {
        let options = parse_launch_options(["-launchgame".to_string()]);
        assert!(options.profile_name.is_none());
        assert!(options.launch_game);
    }

    #[test]
    fn ignores_unknown_flags() {
        let options = parse_launch_options([
            "--verbose".to_string(),
            "-profile".to_string(),
            "Vanilla".to_string(),
        ]);
        assert_eq!(options.profile_name.as_deref(), Some("Vanilla"));
        assert!(!options.launch_game);
    }

    #[test]
    fn trims_profile_name() {
        let options = parse_launch_options([
            "-profile".to_string(),
            "  Survival  ".to_string(),
        ]);
        assert_eq!(options.profile_name.as_deref(), Some("Survival"));
    }
}
