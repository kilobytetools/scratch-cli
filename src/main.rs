mod api;
mod args;
mod config_file;
mod util;

use api::{BootstrapArgs, ClientOpts, DeleteArgs, ListArgs, PullArgs, PushArgs, StatsArgs};
use args::try_get_args;
use config_file as cf;
use rpassword;
use std::{
    fmt::Display,
    io::{self, Write},
    process,
};

fn blind<T>(o: Option<T>) -> T {
    o.expect("programming error, please open an issue")
}

macro_rules! unwrap_or_exit {
    ($expr:expr) => {
        match $expr {
            Ok(t) => t,
            Err(err) => render_err(err),
        }
    };
}

fn main() {
    let args = get_args();
    let opts = ClientOpts::new(&args.opts.response_format);
    let command = blind(args.command);

    use args::Command::*;
    match command {
        Help(msg) => print_help(msg),
        Push(push) => {
            let endpoint = blind(args.opts.endpoint);
            let render_prefix = match push.render_url {
                true => format!("{}/scratch/file/", endpoint),
                false => "".into(),
            };
            let report_id = |id: &String| {
                println!("{}{}", render_prefix, id.trim());
                let _ = io::stdout().flush();
            };
            let args = PushArgs::new(
                blind(args.opts.api_key),
                endpoint,
                blind(push.input),
                push.burn,
                push.private,
                push.pw,
                push.prefix,
                push.lifetime,
            );
            render_response(api::push(args, opts, report_id));
        }
        Pull(pull) => {
            let args = PullArgs::new(
                blind(args.opts.endpoint),
                pull.id,
                args.opts.api_key,
                pull.pw,
                io::stdout(),
            );
            render_response(api::pull(args, opts));
        }
        List => {
            let args = ListArgs::new(blind(args.opts.api_key), blind(args.opts.endpoint));
            render_response(api::list(args, opts));
        }
        Delete(delete) => {
            let args = DeleteArgs::new(
                blind(args.opts.api_key),
                blind(args.opts.endpoint),
                blind(delete.id),
            );
            render_response(api::delete(args, opts));
        }
        Stats => {
            let args = StatsArgs::new(blind(args.opts.api_key), blind(args.opts.endpoint));
            render_response(api::stats(args, opts));
        }
        Bootstrap(bootstrap) => {
            let args = BootstrapArgs::new(get_handle(), get_password());
            let resp = unwrap_or_exit!(api::bootstrap(args));
            let (api_key, endpoint) = (resp.api_key, resp.dataplane_endpoint);
            let cfg = format!(
                "\
                api_key = \"{api_key}\"\n\
                endpoint = \"{endpoint}\"\n\
                \n\
                [response]\n\
                format = \"text/plain\"  # or \"text/javascript\"\n\
                \n\
                [scratch-push]\n\
                lifetime = \"5m\"  # or \"120s\", \"2m\", \"1h\", ...\n\
                # burn = false\n\
                # private = true\n\
                "
            );
            match bootstrap.stdout {
                true => print!("{}", cfg),
                false => unwrap_or_exit!(cf::write(cf::DEFAULT_CONFIG_PATH, cfg)),
            }
        }
    }
}

fn get_args() -> args::Args {
    unwrap_or_exit!(try_get_args())
}

fn get_handle() -> String {
    print!("Enter your handle: ");
    let _ = io::stdout().flush();
    let mut resp = String::new();
    unwrap_or_exit!(io::stdin().read_line(&mut resp));
    resp.trim().to_string()
}

fn get_password() -> String {
    unwrap_or_exit!(rpassword::prompt_password("Enter your password: "))
}

fn print_help(msg: &str) -> ! {
    println!("{}", msg);
    process::exit(0);
}

fn render_response(res: Result<String, api::ErrorKind>) {
    let data = unwrap_or_exit!(res);
    if !data.trim().is_empty() {
        println!("{}", data.trim());
    }
}

fn render_err<T: Display>(err: T) -> ! {
    eprintln!("{}", err);
    process::exit(1);
}
