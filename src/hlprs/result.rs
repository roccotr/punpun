use std::io;
use std::convert::From;
use std::fmt;
use std::str::Utf8Error;
use std::error::Error;
use openssl::ssl::error::SslError;
use byteorder;

pub type PunpunResult<T> = Result<T, PunpunError>;

#[derive(Debug)]
pub enum PunpunError {
	/// A Punpun protocol error
	ProtocolError(String),
	/// Invalid Punpun request error
	RequestError(String),
	/// Invalid Punpun response error
	ResponseError(String),
	/// Invalid Punpun data frame error
	DataFrameError(String),
	/// No data available
	NoDataAvailable,
	/// An input/output error
	IoError(io::Error)
	// An HTTP parsing error
	//HttpError(HttpError),
	// A URL parsing error
	//UrlError(ParseError),
    // A Punpun URL error
    //PunpunUrlError(WSUrlErrorKind),
	// An SSL error
	//SslError(SslError),
	// A UTF-8 error
	//Utf8Error(Utf8Error),
}

impl fmt::Display for PunpunError {
	fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
		try!(fmt.write_str("PunpunError: "));
		try!(fmt.write_str(self.description()));
		Ok(())
	}
}

impl Error for PunpunError {
	fn description(&self) -> &str {
		match *self {
            PunpunError::ProtocolError(_) => "Punpun protocol error",
			PunpunError::RequestError(_) => "Punpun request error",
			PunpunError::ResponseError(_) => "Punpun response error",
			PunpunError::DataFrameError(_) => "Punpun data frame error",
			PunpunError::NoDataAvailable => "No data available",
			PunpunError::IoError(_) => "I/O failure"
			//PunpunError::HttpError(_) => "HTTP failure",
			//PunpunError::UrlError(_) => "URL failure",
			//PunpunError::SslError(_) => "SSL failure",
			//PunpunError::Utf8Error(_) => "UTF-8 failure",
            //PunpunError::PunpunUrlError(_) => "Punpun URL failure",
		}
	}

	fn cause(&self) -> Option<&Error> {
		match *self {
			PunpunError::IoError(ref error) => Some(error),
			/*PunpunError::HttpError(ref error) => Some(error),
			PunpunError::UrlError(ref error) => Some(error),
			PunpunError::SslError(ref error) => Some(error),
			PunpunError::Utf8Error(ref error) => Some(error),
            PunpunError::PunpunUrlError(ref error) => Some(error),*/
			_ => None,
		}
	}
}

impl From<io::Error> for PunpunError {
	fn from(err: io::Error) -> PunpunError {
		PunpunError::IoError(err)
	}
}

/*impl From<HttpError> for PunpunError {
	fn from(err: HttpError) -> PunpunError {
		PunpunError::HttpError(err)
	}
}

impl From<ParseError> for PunpunError {
	fn from(err: ParseError) -> PunpunError {
		PunpunError::UrlError(err)
	}
}

impl From<SslError> for PunpunError {
	fn from(err: SslError) -> PunpunError {
		PunpunError::SslError(err)
	}
}

impl From<Utf8Error> for PunpunError {
	fn from(err: Utf8Error) -> PunpunError {
		PunpunError::Utf8Error(err)
	}
}*/

impl From<byteorder::Error> for PunpunError {
	fn from(err: byteorder::Error) -> PunpunError {
		match err {
			byteorder::Error::UnexpectedEOF => PunpunError::NoDataAvailable,
			byteorder::Error::Io(err) => From::from(err)
		}
	}
}
/*
impl From<WSUrlErrorKind> for PunpunError {
    fn from(err: WSUrlErrorKind) -> PunpunError {
        PunpunError::PunpunUrlError(err)
    }
}*/

/// Represents a Punpun URL error
#[derive(Debug)]
pub enum WSUrlErrorKind {
    /// Fragments are not valid in a Punpun URL
    CannotSetFragment,
    /// The scheme provided is invalid for a Punpun
    InvalidScheme,
}

impl fmt::Display for WSUrlErrorKind {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        try!(fmt.write_str("Punpun Url Error: "));
        try!(fmt.write_str(self.description()));
        Ok(())
    }
}

impl Error for WSUrlErrorKind {
    fn description(&self) -> &str {
        match *self {
            WSUrlErrorKind::CannotSetFragment => "Punpun URL cannot set fragment",
            WSUrlErrorKind::InvalidScheme => "Punpun URL invalid scheme"
        }
    }
}
