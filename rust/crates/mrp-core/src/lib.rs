//! Manto.RustPad core.
//!
//! Regole che valgono per tutto il crate:
//! - un file non viene mai letto per intero in memoria: si usa una mappatura in sola lettura;
//! - posizioni e conteggi sono `u64` (i file superano 4 GiB);
//! - il contenuto e' trattato come byte, non come UTF-8.

pub mod block;
pub mod charsets;
pub mod docsearch;
pub mod document;
pub mod edit;
pub mod encoding;
pub mod lang;
pub mod lineending;
pub mod lines;
pub mod multibyte;
pub mod newline;
pub mod pattern;
pub mod scan;
pub mod search;
pub mod session;
pub mod source;
pub mod view;

pub use document::{Document, Follow, RestoreError, SaveError};
pub use lines::{BuildOptions, Cancelled, LineIndex};
pub use search::{find_next, find_prev, SearchOptions, SearchResult};
pub use source::FileSource;
