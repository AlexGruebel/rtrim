use std::fmt::{self, Display, Formatter};

pub enum RTrimError {
    Git(git2::Error),
    Io(std::io::Error),
}

impl From<git2::Error> for RTrimError {
    fn from(e: git2::Error) -> Self {
        RTrimError::Git(e)
    }
}

impl From<std::io::Error> for RTrimError {
    fn from(e: std::io::Error) -> Self {
        RTrimError::Io(e)
    }
}

impl Display for RTrimError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            RTrimError::Git(e) => e.fmt(f),
            RTrimError::Io(e) => e.fmt(f),
        }
    }
}
