use std::{error::Error as StdError, fmt::Display, io, str::FromStr};

use super::config_file as cf;
use super::util;
use lexopt;
use toml;

const HELP: &str = "
USAGE: scratch [OPTIONS] [COMMAND]

Easily transmit small bits of short-lived data.

OPTIONS:
    --api-key API_KEY       API Key found in your account settings page.
    --endpoint ENDPOINT     Endpoint for dataplane operations, found in
                            your account settings page.
    --out-format FORMAT     Control how responses are rendered.  Allowed
                            values [text/plain, text/javascript, txt, js]

COMMAND:
    push        Upload the contents of a file
    pull        Get the contents of a file
    ls          List all file metadata
    rm          Remove a file by id
    stats       Get usage stats for your account
    bootstrap   Create a valid config file
";

const PUSH_HELP: &str = r#"
USAGE: scratch push [OPTIONS] FILE

Upload a file.  The key of the created file is printed.  When pushing from
stdin, buffers the entire input into memory.
(see scratch --help for global options)

OPTIONS:
    --stdin                 (default) Push data from stdin
                            Note: buffers input to memory before writing
    --file FILE             Push the named file
    --lifetime LIFETIME     How long the file should live eg. 10m
                            Format: \d+(h|m|s)
    --private               Whether the file can be read by anyone.
    --pw PASSWORD           Optional password that will be required to
                            read the file.  Format: [a-zA-Z0-9._-]{1,20}
    --burn                  Whether the file should be deleted the first
                            time it's read
    --prefix PREFIX         Optional prefix for the random file key.
                            Useful for segmenting temporary files by use.
                            Format: [a-zA-Z0-9._-:|]{1,64}

EXAMPLES:
    scratch push --lifetime 2h < ~/.ssh/id_rsa.pub
    scratch push --burn --prefix creds.aws: --file ~/.aws/config
"#;

const PULL_HELP: &str = r#"
USAGE: scratch pull [OPTIONS] [ID]

Pull a file by id.  If the file was pushed with a password, it is
required to pull the file.  When ID is omitted, pulls the most recently
pushed file.
(see scratch --help for global options)

ARGUMENTS:
    ID          The id of the file to pull.  If you pushed the file with a
                prefix, you must include that prefix.  Defaults to the
                id of the most recently pushed file.

OPTIONS:
    --anon      pull without passing credentials.  only public files
                (pushed with private=false) can be pulled anonymously.
    --pw PW     password the file was pushed with, if any.

"#;

const LIST_HELP: &str = r#"
USAGE: scratch ls

List file ids and their metadata.
(see scratch --help for global options)
"#;

const DELETE_HELP: &str = r#"
USAGE: scratch rm ID

Delete a file by id.

ARGUMENTS:
    ID  The id of the file to delete.  If you pushed the file with a prefix,
        you must include that prefix.  Deletion does not require a password.

EXAMPLES:
    scratch delete c869d7cc
    scratch delete creds.aws:f0022e5a
"#;

const STATS_HELP: &str = r#"
USAGE: scratch stats

List usage and capacity stats for your account.
(see scratch --help for global options)
"#;

const BOOTSTRAP_HELP: &str = r#"
USAGE: scratch bootstrap

Creates a minimal valid config file to use the service.
By default this writes to ~/.kilobytetools/config.

OPTIONS:
    --stdout    Write to stdout instead of the default path.
"#;

#[derive(Debug)]
pub enum ErrorKind {
    Lexopt(lexopt::Error),
    BadSubcommand(String),
    MalformedConfigFile(&'static str, toml::de::Error),
    MissingArgument(&'static str, &'static str),
    MissingPositionalArgument(&'static str),
    IoError(io::Error),
    CustomError(String),
}

impl StdError for ErrorKind {}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKind::BadSubcommand(name) => {
                write!(f, "unknown subcommand {}", name)
            }
            ErrorKind::Lexopt(err) => {
                write!(f, "{}", err)
            }
            ErrorKind::MalformedConfigFile(filename, msg) => {
                write!(f, "malformed config file at {}: {}", filename, msg)
            }
            ErrorKind::MissingArgument(cli_name, config_name) => {
                write!(
                    f,
                    "missing required option '{}' or config setting '{}'",
                    cli_name, config_name
                )
            }
            ErrorKind::MissingPositionalArgument(name) => {
                write!(f, "missing positional argument {}", name)
            }
            ErrorKind::IoError(err) => {
                write!(f, "{}", err)
            }
            ErrorKind::CustomError(msg) => {
                write!(f, "{}", msg)
            }
        }
    }
}

impl From<lexopt::Error> for ErrorKind {
    fn from(err: lexopt::Error) -> Self {
        ErrorKind::Lexopt(err)
    }
}

impl From<io::Error> for ErrorKind {
    fn from(err: io::Error) -> Self {
        ErrorKind::IoError(err)
    }
}

pub struct Args {
    pub opts: CommonOptions,
    pub command: Option<Command>,
}

#[derive(Default)]
pub struct CommonOptions {
    pub api_key: Option<String>,
    pub endpoint: Option<String>,

    pub response_format: Option<util::ResponseFormat>,
}

pub enum Command {
    Help(&'static str),
    Push(PushArgs),
    Pull(PullArgs),
    List,
    Delete(DeleteArgs),
    Stats,
    Bootstrap(BootstrapArgs),
}

enum CommandName {
    Push,
    Pull,
    List,
    Delete,
    Stats,
    Bootstrap,
}

impl FromStr for CommandName {
    type Err = ErrorKind;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "push" => Ok(CommandName::Push),
            "pull" => Ok(CommandName::Pull),
            "ls" => Ok(CommandName::List),
            "rm" => Ok(CommandName::Delete),
            "stats" => Ok(CommandName::Stats),
            "bootstrap" => Ok(CommandName::Bootstrap),
            _ => Err(ErrorKind::BadSubcommand(s.into())),
        }
    }
}

#[derive(Default)]
pub struct PushArgs {
    pub lifetime: Option<util::Lifetime>,
    pub private: Option<bool>,
    pub pw: Option<String>,
    pub burn: Option<bool>,
    pub prefix: Option<util::Prefix>,
    pub input: Option<util::InputMode>,
}

#[derive(Default)]
pub struct PullArgs {
    pub id: Option<String>,
    pub anon: Option<bool>,
    pub pw: Option<String>,
}

#[derive(Default)]
pub struct DeleteArgs {
    pub id: Option<String>,
}

#[derive(Default)]
pub struct BootstrapArgs {
    pub stdout: bool,
}

pub fn try_get_args() -> Result<Args, ErrorKind> {
    let mut opts = CommonOptions::default();
    let mut help = false;

    let mut command = None;
    let mut subcommand_name: Option<CommandName> = None;

    let mut pw = None;
    let mut push_args = PushArgs::default();
    let mut pull_args = PullArgs::default();
    let mut delete_args = DeleteArgs::default();
    let mut bootstrap_args = BootstrapArgs::default();

    use lexopt::prelude::*;
    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match &arg {
            Short('h') | Long("help") => help = true,

            Long("api-key") => opts.api_key = Some(parser.value()?.parse()?),
            Long("endpoint") => opts.endpoint = Some(parser.value()?.parse()?),
            Long("out-format") => opts.response_format = Some(parser.value()?.parse()?),

            Long("lifetime") => push_args.lifetime = Some(parser.value()?.parse()?),
            Long("private") => push_args.private = Some(true),
            Long("no-private") => push_args.private = Some(false),
            Long("pw") => pw = Some(parser.value()?.parse()?),
            Long("burn") => push_args.burn = Some(true),
            Long("no-burn") => push_args.burn = Some(false),
            Long("prefix") => push_args.prefix = Some(parser.value()?.parse()?),

            // note: defer reading stdin to memory until all args are parsed
            Long("stdin") => push_args.input = None,

            Long("file") => {
                let name: String = parser.value()?.parse()?;
                push_args.input = Some(util::InputMode::from_filename(name)?)
            }

            Long("anon") => pull_args.anon = Some(true),
            Long("no-anon") => pull_args.anon = Some(false),

            Long("stdout") => bootstrap_args.stdout = true,
            Long("no-stdout") => bootstrap_args.stdout = false,

            Value(subcommand) if subcommand_name.is_none() => {
                if subcommand == "help" {
                    help = true;
                } else {
                    subcommand_name = Some(subcommand.parse()?)
                }
            }

            Value(next_arg) => match &subcommand_name {
                Some(x) => match x {
                    CommandName::Pull => pull_args.id = Some(next_arg.parse()?),
                    CommandName::Delete => delete_args.id = Some(next_arg.parse()?),
                    _ => return Err(arg.unexpected().into()),
                },
                None => return Err(arg.unexpected().into()),
            },
            _ => return Err(arg.unexpected().into()),
        }
    }

    fn mv<T>(src: Option<T>, dst: &mut Option<T>) {
        if let Some(value) = src {
            dst.get_or_insert(value);
        }
    }

    match cf::load(cf::DEFAULT_CONFIG_PATH) {
        Ok(config_file) => {
            mv(config_file.api_key, &mut opts.api_key);
            mv(config_file.endpoint, &mut opts.endpoint);
            mv(config_file.response.format, &mut opts.response_format);

            mv(config_file.push.lifetime, &mut push_args.lifetime);
            mv(config_file.push.private, &mut push_args.private);
            mv(config_file.push.burn, &mut push_args.burn);
            mv(config_file.push.prefix, &mut push_args.prefix);
        }
        Err(err) => match err {
            cf::ErrorKind::IoError(_) => {}
            cf::ErrorKind::DeError(err) => {
                return Err(ErrorKind::MalformedConfigFile(cf::DEFAULT_CONFIG_PATH, err))
            }
        },
    }

    // set defaults, move subcommand args
    match &subcommand_name {
        Some(name) => match name {
            CommandName::Push => {
                if push_args.input.is_none() {
                    push_args.input = Some(util::InputMode::from_stdin()?);
                }
                push_args.pw = pw;
                command = Some(Command::Push(push_args));
            }
            CommandName::Pull => {
                pull_args.pw = pw;
                if let Some(true) = pull_args.anon {
                    // unset api_key when --anon
                    opts.api_key = None;
                }
                command = Some(Command::Pull(pull_args))
            }
            CommandName::List => command = Some(Command::List),
            CommandName::Delete => command = Some(Command::Delete(delete_args)),
            CommandName::Stats => command = Some(Command::Stats),
            CommandName::Bootstrap => command = Some(Command::Bootstrap(bootstrap_args)),
        },
        _ => {
            help = true;
        }
    }

    if help {
        // replace command with Command::Help so caller can render it
        let msg = match command {
            Some(command) => match command {
                Command::Help(_) => HELP,
                Command::Push(_) => PUSH_HELP,
                Command::Pull(_) => PULL_HELP,
                Command::List => LIST_HELP,
                Command::Delete(_) => DELETE_HELP,
                Command::Stats => STATS_HELP,
                Command::Bootstrap(_) => BOOTSTRAP_HELP,
            },
            None => HELP,
        };
        command = Some(Command::Help(msg));
    }
    let args = Args { opts, command };
    if !help {
        // don't validate args during --help, they're probably mangled
        validate_args(&args)?;
    }
    Ok(args)
}

fn validate_args(args: &Args) -> Result<(), ErrorKind> {
    if args.opts.api_key.is_none() {
        match &args.command {
            Some(command) => match command {
                Command::Pull(args) => {
                    if let Some(true) = args.anon {
                        // anon pulls don't need api key
                    } else {
                        return Err(ErrorKind::MissingArgument("--api-key", "api_key"));
                    }
                }
                Command::Bootstrap(_) => {
                    // bootstrapping doesn't require api key
                }
                _ => return Err(ErrorKind::MissingArgument("--api-key", "api_key")),
            },
            None => return Err(ErrorKind::MissingArgument("--api-key", "api_key")),
        }
    }
    if args.opts.endpoint.is_none() {
        match &args.command {
            Some(command) => match command {
                Command::Bootstrap(_) => {
                    // bootstrapping doesn't require endpoint
                }
                _ => {
                    return Err(ErrorKind::MissingArgument("--endpoint", "endpoint"));
                }
            },
            None => {}
        }
    }
    match &args.command {
        Some(x) => match x {
            Command::Delete(args) => {
                if args.id.is_none() {
                    return Err(ErrorKind::MissingPositionalArgument("ID"));
                }
            }
            Command::Bootstrap(args) => {
                if !args.stdout && cf::exists(cf::DEFAULT_CONFIG_PATH) {
                    return Err(ErrorKind::CustomError(format!(
                        "error: existing config file found at {}",
                        cf::DEFAULT_CONFIG_PATH
                    )));
                }
            }
            _ => {}
        },
        _ => {}
    }
    Ok(())
}
