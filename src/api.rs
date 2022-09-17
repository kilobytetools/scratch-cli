use super::util::{InputMode, Lifetime, Prefix, ResponseFormat};
use base64;
use lazy_static::lazy_static;
use regex::Regex;
use std::{fmt::Display, io, str::FromStr};
use ureq::{self, Request, Response};

pub enum ErrorKind {
    UReqError(String),
    ServerError(&'static str),
    LocalIoError(io::Error),
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKind::UReqError(msg) => write!(f, "{}", msg),
            ErrorKind::ServerError(msg) => write!(f, "{}", msg),
            ErrorKind::LocalIoError(err) => write!(f, "local io error: {}", err),
        }
    }
}

impl From<ureq::Error> for ErrorKind {
    fn from(err: ureq::Error) -> Self {
        ErrorKind::UReqError(match err {
            ureq::Error::Status(_, resp) => resp
                .into_string()
                .unwrap_or("malformed response body".into()),
            ureq::Error::Transport(_) => {
                format!("unexpected request error {}", err)
            }
        })
    }
}

pub struct ClientOpts<'a> {
    response_format: &'a Option<ResponseFormat>,
}
impl<'a> ClientOpts<'a> {
    pub fn new(response_format: &'a Option<ResponseFormat>) -> ClientOpts<'a> {
        Self { response_format }
    }
}

pub struct PushArgs {
    api_key: String,
    endpoint: String,
    input: InputMode,

    burn: Option<bool>,
    private: Option<bool>,
    pw: Option<String>,
    prefix: Option<Prefix>,
    lifetime: Option<Lifetime>,
}

pub struct PullArgs<W>
where
    W: io::Write,
{
    endpoint: String,
    id: Option<String>,

    api_key: Option<String>,
    pw: Option<String>,

    output: W,
}

pub struct ListArgs {
    api_key: String,
    endpoint: String,
}

pub struct DeleteArgs {
    api_key: String,
    endpoint: String,
    id: String,
}

pub struct StatsArgs {
    api_key: String,
    endpoint: String,
}

pub struct BootstrapArgs {
    handle: String,
    password: String,
}

pub struct BootstrapResponse {
    pub api_key: String,
    pub dataplane_endpoint: String,
}

impl PushArgs {
    pub fn new(
        api_key: String,
        endpoint: String,
        input: InputMode,
        burn: Option<bool>,
        private: Option<bool>,
        pw: Option<String>,
        prefix: Option<Prefix>,
        lifetime: Option<Lifetime>,
    ) -> PushArgs {
        Self {
            api_key,
            endpoint,
            input,
            burn,
            private,
            pw,
            prefix,
            lifetime,
        }
    }
}

impl<W> PullArgs<W>
where
    W: io::Write,
{
    pub fn new(
        endpoint: String,
        id: Option<String>,
        api_key: Option<String>,
        pw: Option<String>,
        output: W,
    ) -> Self {
        Self {
            endpoint,
            id,
            api_key,
            pw,
            output,
        }
    }
}

impl ListArgs {
    pub fn new(api_key: String, endpoint: String) -> Self {
        Self { api_key, endpoint }
    }
}

impl DeleteArgs {
    pub fn new(api_key: String, endpoint: String, id: String) -> Self {
        Self {
            api_key,
            endpoint,
            id,
        }
    }
}

impl StatsArgs {
    pub fn new(api_key: String, endpoint: String) -> Self {
        Self { api_key, endpoint }
    }
}

impl BootstrapArgs {
    pub fn new(handle: String, password: String) -> Self {
        Self { handle, password }
    }
}

fn request(method: &'static str, endpoint: &str, opts: &ClientOpts, action: &str) -> Request {
    const PRODUCT: &str = "scratch";
    let path = if endpoint.ends_with("/") {
        format!("{}{}/{}", endpoint, PRODUCT, action)
    } else {
        format!("{}/{}/{}", endpoint, PRODUCT, action)
    };
    let mut req = ureq::request(method, &path);
    if let Some(fmt) = &opts.response_format {
        req = req.set("Accept", fmt.to_api_name());
    }
    req
}

fn get_content_type(resp: &Response) -> Option<ResponseFormat> {
    let hval = resp.header("content-type");
    match hval {
        Some(header) => match ResponseFormat::from_str(header) {
            Ok(fmt) => Some(fmt),
            Err(_) => None,
        },
        None => None,
    }
}

fn extract_id(text: &String, content_type: ResponseFormat) -> Option<String> {
    match content_type {
        ResponseFormat::TextJavascript => {
            const ID_PATTERN: &str = r#"^\{\s*"id"\s*:\s*"(.*)"\s*\}$"#;
            lazy_static! {
                static ref ID_RE: Regex = Regex::new(ID_PATTERN).unwrap();
            }
            match ID_RE.captures(&text) {
                Some(captures) => Some(captures[1].into()),
                None => None,
            }
        }
        ResponseFormat::TextPlain => Some(text.trim().into()),
    }
}

trait ResponseBodyHelpers {
    fn text_or_err(self) -> Result<String, ErrorKind>;
}

impl ResponseBodyHelpers for Response {
    fn text_or_err(self) -> Result<String, ErrorKind> {
        let text = self
            .into_string()
            .map_err(|_| ErrorKind::ServerError("malformed resp from server: bad encoding"))?;
        Ok(text)
    }
}

pub fn push<R>(args: PushArgs, opts: ClientOpts, report_id: R) -> Result<String, ErrorKind>
where
    R: Fn(&String) -> (),
{
    let created_id: String;
    let resp_text: String;

    {
        let mut create = request("POST", &args.endpoint, &opts, "file")
            .set("Authorization", &format!("Bearer {}", args.api_key))
            .set("Content-Length", "0");
        if let Some(lifetime) = args.lifetime {
            create = create.query("lifetime", &lifetime.0);
        }
        if let Some(private) = args.private {
            create = create.query("private", &private.to_string());
        }
        if let Some(pw) = args.pw {
            create = create.query("pw", &pw);
        }
        if let Some(burn) = args.burn {
            create = create.query("burn", &burn.to_string());
        }
        if let Some(prefix) = args.prefix {
            create = create.query("prefix", &prefix.0);
        }

        let resp = create.call()?;
        let maybe_content_type = get_content_type(&resp);
        resp_text = resp.text_or_err()?;
        let content_type = match maybe_content_type {
            Some(x) => x,
            None => {
                return Err(ErrorKind::ServerError(
                    "malformed resp from server: no content_type",
                ))
            }
        };
        created_id = match extract_id(&resp_text, content_type) {
            Some(x) => x,
            None => return Err(ErrorKind::ServerError("malformed resp from server: no id")),
        };
    }

    report_id(&resp_text);

    {
        let push = request(
            "POST",
            &args.endpoint,
            &opts,
            &format!("file/{}", created_id),
        )
        .set("Authorization", &format!("Bearer {}", args.api_key))
        .set("Content-Length", &args.input.size().to_string());
        let resp = match args.input {
            InputMode::Buffer(buf) => push.send_bytes(&buf),
            InputMode::File(file) => push.send(file),
        };
        match resp {
            Ok(resp) => Ok(resp.text_or_err()?),
            Err(err) => Err(err.into()),
        }
    }
}
pub fn pull<W>(mut args: PullArgs<W>, opts: ClientOpts) -> Result<String, ErrorKind>
where
    W: io::Write,
{
    const DEFAULT_ID: &str = "latest";
    let mut pull = request(
        "GET",
        &args.endpoint,
        &opts,
        &format!("file/{}", args.id.unwrap_or(DEFAULT_ID.to_string())),
    );
    if let Some(api_key) = args.api_key {
        pull = pull.set("Authorization", &format!("Bearer {}", api_key));
    }
    if let Some(pw) = args.pw {
        pull = pull.query("pw", &pw);
    }
    let resp = pull.call()?;
    match io::copy(&mut resp.into_reader(), &mut args.output) {
        Ok(_) => {}
        Err(err) => return Err(ErrorKind::LocalIoError(err)),
    };
    Ok("".into())
}
pub fn list(args: ListArgs, opts: ClientOpts) -> Result<String, ErrorKind> {
    let list = request("GET", &args.endpoint, &opts, "file")
        .set("Authorization", &format!("Bearer {}", args.api_key));
    let resp = list.call()?;
    Ok(resp.text_or_err()?)
}
pub fn delete(args: DeleteArgs, opts: ClientOpts) -> Result<String, ErrorKind> {
    let delete = request(
        "DELETE",
        &args.endpoint,
        &opts,
        &format!("file/{}", args.id),
    )
    .set("Authorization", &format!("Bearer {}", args.api_key));
    let resp = delete.call()?;
    Ok(resp.text_or_err()?)
}
pub fn stats(args: StatsArgs, opts: ClientOpts) -> Result<String, ErrorKind> {
    let stats = request("GET", &args.endpoint, &opts, "me/stats")
        .set("Authorization", &format!("Bearer {}", args.api_key));
    let resp = stats.call()?;
    Ok(resp.text_or_err()?)
}
pub fn bootstrap(args: BootstrapArgs) -> Result<BootstrapResponse, ErrorKind> {
    let authorization = format!(
        "Basic {}",
        base64::encode_config(
            (format!("{}:{}", args.handle, args.password)).as_bytes(),
            base64::STANDARD
        )
    );
    macro_rules! req {
        ($component:expr) => {
            ureq::get(format!("https://kilobytetools.io/bootstrap/{}", $component).as_str())
                .set("Authorization", &authorization)
                .call()?
                .text_or_err()?
                .trim()
                .to_string()
        };
    }
    Ok(BootstrapResponse {
        api_key: req!("api_key"),
        dataplane_endpoint: req!("dataplane_endpoint"),
    })
}
