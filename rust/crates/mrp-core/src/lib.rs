//! Manto.RustPad core.
//!
//! Regole che valgono per tutto il crate:
//! - un file non viene mai letto per intero in memoria: si usa una mappatura in sola lettura;
//! - posizioni e conteggi sono `u64` (i file superano 4 GiB);
//! - il contenuto e' trattato come byte, non come UTF-8.

pub mod lines;
pub mod search;
pub mod source;

pub use lines::{BuildOptions, Cancelled, LineIndex};
pub use search::{find_next, find_prev, SearchOptions, SearchResult};
pub use source::FileSource;
