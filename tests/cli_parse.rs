use clap::Parser;
use rmusix::Cli;

#[test]
fn no_args_gives_no_query() {
    let cli = Cli::try_parse_from(["rmusix"]).expect("parse");
    assert!(cli.query.is_none());
}

#[test]
fn positional_query_parsed() {
    let cli = Cli::try_parse_from(["rmusix", "never gonna give you up"]).expect("parse");
    assert_eq!(cli.query.as_deref(), Some("never gonna give you up"));
}

#[test]
fn help_flag_succeeds() {
    let res = Cli::try_parse_from(["rmusix", "--help"]);
    // clap returns DisplayHelp on --help, not a Cli value.
    assert!(res.is_err());
}
