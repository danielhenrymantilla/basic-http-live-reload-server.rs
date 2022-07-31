//! A simple HTTP server, for learning and local development.

#[macro_use]
extern crate derive_more;

use {
    ::bytes::{
        BytesMut,
    },
    ::env_logger::{
        Builder, Env,
    },
    ::futures::{
        future, FutureExt, stream::StreamExt,
    },
    ::handlebars::{
        Handlebars,
    },
    ::http::{
        header::{HeaderMap, HeaderValue},
        status::StatusCode,
        Uri,
    },
    ::hyper::{
        header, Body, Method, Request, Response, Server,
        service::{
            make_service_fn, service_fn,
        },
    },
    ::log::{
        debug, error, info, trace, warn,
    },
    ::percent_encoding::{
        percent_decode_str,
    },
    ::serde::{
        Serialize,
    },
    ::std::{
        error::Error as StdError,
        io,
        net::SocketAddr,
        ops::Not,
        path::{Path, PathBuf},
        pin::Pin,
    },
    ::structopt::{
        StructOpt,
    },
    ::tokio::{
        fs::File,
        io::{AsyncRead, AsyncReadExt},
        runtime::Runtime,
    },
    ::tokio_util::{
        codec::{BytesCodec, FramedRead},
    },
    crate::utils::{
        Also as _,
    },
};

#[macro_use]
extern crate extension_traits;

pub use error::Error;
mod error;

mod utils;

/// A custom `Result` typedef
pub
type Result<T, E = Error> = ::std::result::Result<T, E>;

fn main ()
{
    // Set up error handling immediately
    if let Err(e) = run() {
        log_error_chain(&e);
    }
}

/// Basic error reporting, including the "cause chain". This is used both by the
/// top-level error reporting and to report internal server errors.
fn log_error_chain (mut e: &dyn StdError)
{
    error!("error: {}", e);
    while let Some(source) = e.source() {
        error!("caused by: {}", source);
        e = source;
    }
}

/// The configuration object, parsed from command line options.
#[derive(Clone, StructOpt)]
#[structopt(about = "A basic HTTP file server")]
pub
struct Config {
    /// The IP:PORT combination.
    #[structopt(
        name = "ADDR",
        short = "a",
        long = "addr",
        parse(try_from_str),
        default_value = "0.0.0.0:4000", // "127.0.0.1:4000"
    )]
    addr: SocketAddr,

    /// The port to use for the websocket server (for the live-reload feature)
    #[structopt(
        name = "PORT",
        long = "ws-port",
        default_value = "8090",
    )]
    ws_port: u16,

    /// The root directory for serving files.
    #[structopt(name = "ROOT", parse(from_os_str), default_value = ".")]
    root_dir: PathBuf,
}

fn run ()
  -> Result<()>
{
    // Initialize logging, and log the "info" level for this crate only, unless
    // the environment contains `RUST_LOG`.
    Builder::from_env(
        Env::new().default_filter_or(concat!(
            env!("CARGO_CRATE_NAME"), "=", "info",
        ))
    )
    .default_format_module_path(false)
    .default_format_timestamp(false)
    .init()
    ;
    // Create the configuration from the command line arguments. It
    // includes the IP address and port to listen on and the path to use
    // as the HTTP server's root directory.
    let config = Config::from_args();

    // Display the configuration to be helpful
    info!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    info!("addr: http://{}", config.addr);
    info!("root dir: {}", config.root_dir.display());

    ::local_ip_address::list_afinet_netifas()
        .ok()
        .map(|it| { info!("Available (IPv4 LAN) address(es):"); it })
        .into_iter()
        .flatten()
        .for_each(|(_name, ip)| {
            if matches!(ip, ::std::net::IpAddr::V4(ip) if ip.is_private()) {
                info!("\t-a {0}:{1} | http://{0}:{1}", ip, config.addr.port());
            }
        })
    ;

    // Create the MakeService object that creates a new Hyper service for every
    // connection. Both these closures need to return a Future of Result, and we
    // use two different mechanisms to achieve that.
    let make_service = make_service_fn(|_| {
        let config = config.clone();

        let service = service_fn(move |req| {
            let config = config.clone();

            // Handle the request, returning a Future of Response,
            // and map it to a Future of Result of Response.
            serve(config, req).map(Ok::<_, Error>)
        });

        // Convert the concrete (non-future) service function to a Future of Result.
        future::ok::<_, Error>(service)
    });

    // Create a Tokio runtime and block on Hyper forever.
    let rt = Runtime::new()?;

    rt.spawn({
        let config = config.clone();
        async move {
            spin_ws_server(&config)
                .await
                .unwrap()
        }
    });
    rt.block_on(async {
        // Create a Hyper Server, binding to an address, and use
        // our service builder.
        Server::bind(&config.addr)
            .serve(make_service)
            .await
    })?;

    Ok(())
}

async
fn spin_ws_server (config: &'_ Config)
  -> ::anyhow::Result<()>
{
    loop {
        let ws =
            ::tokio_tungstenite::accept_async(
                ::tokio::net::TcpListener::bind((config.addr.ip(), config.ws_port))
                    .await?
                    .accept()
                    .await?
                    .0
            )
            .await?
        ;
        let _ = ::tokio::task::spawn(ws.for_each(|_| async {}));
    }
}

/// Create an HTTP Response future for each Request.
///
/// Errors are turned into an appropriate HTTP error response, and never
/// propagated upward for hyper to deal with.
async
fn serve (config: Config, req: Request<Body>)
  -> Response<Body>
{
    // Serve the requested file.
    let resp = serve_or_error(config, req).await;
    // Transform internal errors to error responses.
    transform_error(resp)
}

/// Handle all types of requests, but don't deal with transforming internal
/// errors to HTTP error responses.
async
fn serve_or_error (config: Config, req: Request<Body>)
  -> Result<Response<Body>>
{
    // This server only supports the GET method. Return an appropriate
    // response otherwise.
    if let Some(resp) = handle_unsupported_request(&req) {
        return resp;
    }
    // Serve the requested file.
    serve_file(&req, &config).await
}

/// Serve static files from a root directory.
async
fn serve_file (
    req: &Request<Body>,
    config: &'_ Config,
) -> Result<Response<Body>>
{
    let root_dir = &config.root_dir;
    // First, try to do a redirect. If that doesn't happen, then find the path
    // to the static file we want to serve - which may be `index.html` for
    // directories - and send a response containing that file.
    if let Some(redir_resp) = try_dir_redirect(req, &root_dir)? {
        Ok(redir_resp)
    } else {
        respond_with_file(
            &local_path_with_maybe_index(req.uri(), &root_dir)?,
            config,
        )
        .await
    }
}

/// Try to do a 302 redirect for directories.
///
/// If we get a URL without trailing "/" that can be mapped to a directory, then
/// return a 302 redirect to the path with the trailing "/".
///
/// Without this we couldn't correctly return the contents of `index.html` for a
/// directory - for the purpose of building absolute URLs from relative URLs,
/// agents appear to only treat paths with trailing "/" as directories, so we
/// have to redirect to the proper directory URL first.
///
/// In other words, if we returned the contents of `index.html` for URL `docs`
/// then all the relative links in that file would be broken, but that is not
/// the case for URL `docs/`.
///
/// This seems to match the behavior of other static web servers.
fn try_dir_redirect (
    req: &Request<Body>,
    root_dir: &PathBuf,
) -> Result<Option<Response<Body>>>
{
    if req.uri().path().ends_with("/") {
        return Ok(None);
    }
    debug!("path does not end with /");
    let path = local_path_for_request(req.uri(), root_dir)?;
    if path.is_dir().not() {
        return Ok(None);
    }
    let mut new_loc = req.uri().path().to_string();
    new_loc.push_str("/");
    if let Some(query) = req.uri().query() {
        new_loc.push_str("?");
        new_loc.push_str(query);
    }
    info!("redirecting {} to {}", req.uri(), new_loc);
    Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, new_loc)
        .body(Body::empty())
        .map(Some)
        .map_err(Error::from)
}

/// Construct a 200 response with the file as the body, streaming it to avoid
/// loading it fully into memory.
///
/// If the I/O here fails then an error future will be returned, and `serve`
/// will convert it into the appropriate HTTP error response.
async
fn respond_with_file (
    path: &Path,
    config: &Config,
) -> Result<Response<Body>>
{
    let mime_type = file_path_mime(&path);
    let file = File::open(path).await?;
    let meta = file.metadata().await?;
    let mut len = meta.len();
    let stream: Pin<Box<dyn AsyncRead + Send>> =
        if matches!(
            path.extension(), Some(ext) if ext.eq_ignore_ascii_case("html")
        )
        {

            let injected_js = format!(
                include_str!("client_template.html"),
                port = config.ws_port,
            );
            len += injected_js.len() as u64;
            Box::pin(file.chain(::std::io::Cursor::new(injected_js)))
        } else {
            Box::pin(file)
        }
    ;

    // Here's the streaming code.
    // Codecs are how Tokio creates Streams; a FramedRead
    // turns an AsyncRead plus a Decoder into a Stream; and BytesCodec is a
    // Decoder. FramedRead though creates a Stream<Result<BytesMut>> and Hyper's
    // Body wants a Stream<Result<Bytes>>, and BytesMut::freeze will give us a
    // Bytes.
    let codec = BytesCodec::new();
    let stream = FramedRead::new(stream, codec);
    let stream = stream.map(|b| b.map(BytesMut::freeze));
    let body = Body::wrap_stream(stream);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_LENGTH, len as u64)
        .header(header::CONTENT_TYPE, mime_type.as_ref())
        .body(body)
        .map_err(Into::into)
}

/// Get a MIME type based on the file extension.
///
/// If the extension is unknown then return "application/octet-stream".
fn file_path_mime (file_path: &Path)
  -> ::mime::Mime
{
    ::mime_guess::from_path(file_path)
        .first_or_octet_stream()
}

/// Find the local path for a request URI, converting directories to the
/// `index.html` file.
fn local_path_with_maybe_index (uri: &Uri, root_dir: &Path)
  -> Result<PathBuf>
{
    local_path_for_request(uri, root_dir).map(|mut p: PathBuf| {
        if p.is_dir() {
            p.push("index.html");
            debug!("trying {} for directory URL", p.display());
        } else {
            trace!("trying path as from URL");
        }
        p
    })
}

/// Map the request's URI to a local path
fn local_path_for_request (uri: &Uri, root_dir: &Path)
  -> Result<PathBuf>
{
    debug!("raw URI: {}", uri);
    let request_path = uri.path();
    debug!("raw URI to path: {}", request_path);
    // Trim off the url parameters starting with '?'
    let end = request_path.find('?').unwrap_or(request_path.len());
    let request_path = &request_path[0..end];
    // Convert %-encoding to actual values
    let decoded = percent_decode_str(&request_path);
    let request_path = if let Ok(p) = decoded.decode_utf8() {
        p
    } else {
        error!("non utf-8 URL: {}", request_path);
        return Err(Error::UriNotUtf8);
    };
    // Append the requested path to the root directory
    let mut path = root_dir.to_owned();
    if request_path.starts_with('/') {
        path.push(&request_path[1..]);
    } else {
        warn!("found non-absolute path {}", request_path);
        return Err(Error::UriNotAbsolute);
    }
    debug!("URL · path : {} · {}", uri, path.display());
    Ok(path)
}

/// Create an error response if the request contains unsupported methods,
/// headers, etc.
fn handle_unsupported_request (req: &Request<Body>)
  -> Option<Result<Response<Body>>>
{
    let unsup = get_unsupported_request_message(req)?;
    Some(make_error_response_from_code_and_headers(unsup.code, unsup.headers))
}

/// Description of an unsupported request.
struct Unsupported {
    code: StatusCode,
    headers: HeaderMap,
}

/// Create messages for unsupported requests.
fn get_unsupported_request_message (req: &Request<Body>)
  -> Option<Unsupported>
{
    // https://tools.ietf.org/html/rfc7231#section-6.5.5
    (req.method() != Method::GET).then(|| Unsupported {
        code: StatusCode::METHOD_NOT_ALLOWED,
        headers: ::core::iter::once(
            (header::ALLOW, HeaderValue::from_static("GET")),
        ).collect(),
    })
}

/// Turn any errors into an HTTP error response.
fn transform_error (resp: Result<Response<Body>>)
  -> Response<Body>
{
    resp.or_else(|ref err| make_error_response_from_code(
            if let Error::Io(io_err) = err {
                debug!("{}", io_err);
                StatusCode::NOT_FOUND
            } else {
                log_error_chain(err);
                StatusCode::INTERNAL_SERVER_ERROR
            }
        ))
        .unwrap_or_else(|e| {
            // Last-ditch error reporting if
            // even making the error response failed.
            error!("unexpected internal error: {}", e);
            Response::new(Body::from(
                format!("unexpected internal error: {}", e),
            ))
        })
}
/// Make an error response given an HTTP status code.
fn make_error_response_from_code (status: StatusCode)
  -> Result<Response<Body>>
{
    make_error_response_from_code_and_headers(status, HeaderMap::new())
}

/// Make an error response given an HTTP status code and response headers.
fn make_error_response_from_code_and_headers (
    status: StatusCode,
    headers: HeaderMap,
) -> Result<Response<Body>>
{
    let body = render_error_html(status)?;
    html_str_to_response_with_headers(body, status, headers)
}

/// Make an HTTP response from a HTML string and response headers.
fn html_str_to_response_with_headers (
    body: String,
    status: StatusCode,
    headers: HeaderMap,
) -> Result<Response<Body>>
{
    Response::builder()
        .also(|b| { b.headers_mut().map(|h| h.extend(headers)); })
        .status(status)
        .header(header::CONTENT_LENGTH, body.len())
        .header(header::CONTENT_TYPE, ::mime::TEXT_HTML.as_ref())
        .body(Body::from(body))
        .map_err(Error::from)
}

/// A handlebars HTML template.
const HTML_TEMPLATE: &str = include_str!("template.html");

/// The data for the handlebars HTML template. Handlebars will use serde to get
/// the data out of the struct and mapped onto the template.
#[derive(Serialize)]
struct HtmlCfg {
    title: String,
    body: String,
}

/// Render an HTML page with handlebars, the template and the configuration data.
fn render_html (cfg: HtmlCfg)
  -> Result<String>
{
    Handlebars::new()
        .render_template(HTML_TEMPLATE, &cfg)
        .map_err(Error::TemplateRender)
}

/// Render an HTML page from an HTTP status code
fn render_error_html (status: StatusCode)
  -> Result<String>
{
    render_html(HtmlCfg {
        title: format!("{}", status),
        body: String::new(),
    })
}
