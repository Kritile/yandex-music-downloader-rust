use clap::{CommandFactory, Parser};
use yandex_music_downloader::cli::Cli;

#[test]
fn token_can_come_from_environment_and_cli_overrides_it() {
    const NAME: &str = "YANDEX_MUSIC_TOKEN";
    let previous = std::env::var_os(NAME);
    std::env::remove_var(NAME);
    assert!(Cli::try_parse_from(["ymd", "--track-id", "42"]).is_err());

    std::env::set_var(NAME, "fixture-env-token");
    let args = Cli::try_parse_from(["ymd", "--track-id", "42"]).unwrap();
    assert_eq!(args.token, "fixture-env-token");
    let args =
        Cli::try_parse_from(["ymd", "--track-id", "42", "--token", "fixture-cli-token"]).unwrap();
    assert_eq!(args.token, "fixture-cli-token");

    let mut help = Vec::new();
    Cli::command().write_long_help(&mut help).unwrap();
    let help = String::from_utf8(help).unwrap();
    assert!(help.contains("YANDEX_MUSIC_TOKEN"));
    assert!(!help.contains("fixture-env-token"));

    match previous {
        Some(value) => std::env::set_var(NAME, value),
        None => std::env::remove_var(NAME),
    }
}
