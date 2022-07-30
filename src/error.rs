use super::*;

/// The basic-http-server error type.
///
/// This is divided into two types of errors: "semantic" errors and "blanket"
/// errors. Semantic errors are custom to the local application semantics and
/// are usually preferred, since they add context and meaning to the error
/// chain. They don't require boilerplate `From` implementations, but do require
/// `map_err` to create when they have interior `causes`.
///
/// Blanket errors are just wrappers around other types, like `Io(io::Error)`.
/// These are common errors that occur in many places so are easier to code and
/// maintain, since e.g. every occurrence of an I/O error doesn't need to be
/// given local semantics.
///
/// The criteria of when to use which type of error variant, and their pros and
/// cons, aren't obvious.
///
/// These errors use `derive(Display)` from the `derive-more` crate to reduce
/// boilerplate.
#[derive(Debug, Display)]
pub
enum Error {
    #[display(fmt = "HTTP error")]
    Http(http::Error),

    #[display(fmt = "Hyper error")]
    Hyper(hyper::Error),

    #[display(fmt = "I/O error")]
    Io(io::Error),

    // custom "semantic" error types
    #[display(fmt = "failed to parse IP address")]
    AddrParse(std::net::AddrParseError),

    #[display(fmt = "failed to render template")]
    TemplateRender(handlebars::TemplateRenderError),

    #[display(fmt = "requested URI is not an absolute path")]
    UriNotAbsolute,

    #[display(fmt = "requested URI is not UTF-8")]
    UriNotUtf8,
}

impl StdError for Error {
    fn source (self: &'_ Error)
      -> Option<&'_ (dyn StdError + 'static)>
    {
        match self {
            Self::Io(e) => Some(e),
            Self::Http(e) => Some(e),
            Self::Hyper(e) => Some(e),
            Self::AddrParse(e) => Some(e),
            Self::TemplateRender(e) => Some(e),
            Self::UriNotAbsolute => None,
            Self::UriNotUtf8 => None,
        }
    }
}

impl From<::http::Error> for Error {
    fn from (e: http::Error)
      -> Error
    {
        Error::Http(e)
    }
}

impl From<::hyper::Error> for Error {
    fn from (e: hyper::Error)
      -> Error
    {
        Error::Hyper(e)
    }
}

impl From<io::Error> for Error {
    fn from (e: io::Error)
      -> Error
    {
        Error::Io(e)
    }
}
