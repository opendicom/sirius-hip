use actix_web::{error::ErrorInternalServerError, web::Bytes, Error};
use async_stream::stream;
use anyhow::Context as _;
use futures::io::AsyncWriteExt;
use futures::{Stream, StreamExt};
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::AsyncReadExt;
use tokio::sync::oneshot;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::bytes::BytesMut;
use tokio_util::sync::PollSender;

const CREATED_BY: &str = "Created by Opendicom - Sirius HIP (www.opendicom.com)";

const FILE_READ_BUF_BYTES: usize = 64 * 1024;
const CHANNEL_CAPACITY: usize = 32;

// ChannelWriter coalescing behavior:
// - Accumulate many small `poll_write` calls into ~64KiB chunks.
// - Keep a bounded staging buffer to preserve backpressure.
const CHANNEL_CHUNK_BYTES: usize = 64 * 1024;
const CHANNEL_MAX_STAGING_BYTES: usize = 256 * 1024;

/// A tiny adapter that turns `tokio::io::AsyncWrite` calls into a stream of `Bytes`.
///
/// Why this exists:
/// - `async_zip` writes the ZIP file by calling `AsyncWrite::poll_write` repeatedly.
/// - Actix wants a `Stream<Item = Result<Bytes, Error>>` for streaming responses.
///
/// `ChannelWriter` bridges both worlds by forwarding every write into a bounded MPSC channel.
/// The receiver side becomes the HTTP response body stream.
///
/// Notes:
/// - We *coalesce* small writes into larger chunks to reduce allocations and channel sends.
/// - Backpressure is provided by the bounded channel and `PollSender::poll_reserve`.
struct ChannelWriter {
    sender: PollSender<Bytes>,
    staging: BytesMut,
}

impl ChannelWriter {
    fn new(sender: tokio::sync::mpsc::Sender<Bytes>) -> Self {
        Self {
            sender: PollSender::new(sender),
            staging: BytesMut::with_capacity(CHANNEL_CHUNK_BYTES),
        }
    }

    fn poll_flush_staging(
        &mut self,
        cx: &mut Context<'_>,
        force: bool,
    ) -> Poll<io::Result<()>> {
        loop {
            let to_send = if force {
                if self.staging.is_empty() {
                    break;
                }
                self.staging.len().min(CHANNEL_CHUNK_BYTES)
            } else {
                if self.staging.len() < CHANNEL_CHUNK_BYTES {
                    break;
                }
                CHANNEL_CHUNK_BYTES
            };

            // Reserve capacity in the bounded channel. If the consumer is slow,
            // this yields Pending and applies backpressure upstream.
            match self.sender.poll_reserve(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Ok(())) => {
                    let chunk = self.staging.split_to(to_send).freeze();
                    self.sender
                        .send_item(chunk)
                        .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "stream receiver dropped"))?;
                }
                Poll::Ready(Err(_)) => {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "stream receiver dropped",
                    )))
                }
            }
        }

        Poll::Ready(Ok(()))
    }
}

impl tokio::io::AsyncWrite for ChannelWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }

        // Keep staging bounded to preserve backpressure.
        // If the channel is full and staging is already large, don't accept more bytes.
        if self.staging.len() >= CHANNEL_MAX_STAGING_BYTES {
            match self.poll_flush_staging(cx, false) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Ok(())) => {}
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            }
        }

        // Buffer the write. We'll flush in ~64KiB chunks.
        self.staging.extend_from_slice(buf);

        // Best-effort flush full chunks. If this blocks, we still report that we accepted
        // the bytes into staging (bounded by CHANNEL_MAX_STAGING_BYTES).
        match self.poll_flush_staging(cx, false) {
            Poll::Pending => Poll::Ready(Ok(buf.len())),
            Poll::Ready(Ok(())) => Poll::Ready(Ok(buf.len())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Flush any pending buffered bytes, even if below the chunk threshold.
        self.poll_flush_staging(cx, true)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // Force-flush anything left in staging, then close.
        match self.poll_flush_staging(cx, true) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(())) => {
                // Closing the sender tells the receiver stream that no more bytes will arrive.
                self.sender.close();
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
        }
    }
}

/// Builder for a streaming DICOM ZIP response.
///
/// High-level flow:
/// 1. Call `add_entry(name, url)` for each instance you want to include.
/// 2. (Optional) Call `set_http_client(...)` if you add `http(s)://` entries.
/// 3. Call `build()` to obtain an Actix-compatible byte stream.
///
/// Implementation details:
/// - A background task runs `write_zip(...)` using `async_zip` with ZIP64 enforced and
///   `Compression::Stored` (no compression).
/// - `write_zip(...)` writes into `ChannelWriter`, which forwards each write into an MPSC channel.
/// - The returned stream yields those chunks as the HTTP response body.
pub struct DicomStreamZip {
    build_entries: Vec<Entry>,
    http_client: Option<reqwest::Client>,
}

#[derive(Debug)]
struct Entry {
    name: String,
    source: EntrySource,
}

/// Source for a ZIP entry payload.
///
/// We keep this typed (instead of parsing `file://`/`http(s)://` strings at write time)
/// so call sites can:
/// - avoid string parsing/allocations;
/// - attach a WADO fallback when a filesystem path is missing.
#[derive(Debug, Clone)]
pub enum EntrySource {
    /// Read bytes from a local filesystem path.
    FilesystemPath(PathBuf),
    /// Read bytes from an HTTP(S) URL.
    HttpUrl(String),
    /// Prefer filesystem, but if the file is missing, fallback to HTTP(S).
    FilesystemWithHttpFallback {
        path: PathBuf,
        fallback_url: String,
    },
}

impl EntrySource {
    fn from_url_string(url: &str) -> anyhow::Result<Self> {
        if let Some(file) = url.strip_prefix("file://") {
            return Ok(EntrySource::FilesystemPath(PathBuf::from(file)));
        }
        if url.starts_with("http://") || url.starts_with("https://") {
            return Ok(EntrySource::HttpUrl(url.to_string()));
        }
        Err(anyhow::anyhow!("Unsupported URL scheme: {url}"))
    }

    fn file_with_http_fallback(path: impl Into<PathBuf>, fallback_url: impl Into<String>) -> Self {
        EntrySource::FilesystemWithHttpFallback {
            path: path.into(),
            fallback_url: fallback_url.into(),
        }
    }
}

impl Entry {
    /// Create a new Zip Entry
    /// - `name` Name to use in the zip file
    fn new(name: String, source: EntrySource) -> Self {
        Self { name, source }
    }
}

impl DicomStreamZip {
    pub fn new() -> Self {
        Self {
            build_entries: Vec::new(),
            http_client: None,
        }
    }

    /// Set the HTTP client used for `http(s)://` sources.
    ///
    /// This is intentionally injected so handlers can reuse the app/state `reqwest::Client`
    /// (connection pooling, DNS cache, etc.) instead of creating a new global client.
    pub fn set_http_client(&mut self, client: reqwest::Client) {
        self.http_client = Some(client);
    }

    /// Add one file to the ZIP using a URL string.
    ///
    /// This keeps backwards compatibility with earlier code, but prefer the typed APIs:
    /// - `add_filesystem_entry(...)`
    /// - `add_http_entry(...)`
    /// - `add_filesystem_entry_with_http_fallback(...)`
    ///
    /// `url` can be `file://...` or `http(s)://...`.
    pub fn add_entry(&mut self, name: &str, url: &str) -> anyhow::Result<()> {
        let source = EntrySource::from_url_string(url)?;
        self.build_entries.push(Entry::new(name.to_string(), source));
        Ok(())
    }

    /// Add a filesystem-backed entry.
    pub fn add_filesystem_entry(&mut self, name: &str, path: impl Into<PathBuf>) {
        self.build_entries
            .push(Entry::new(name.to_string(), EntrySource::FilesystemPath(path.into())));
    }

    /// Add an HTTP(S)-backed entry.
    pub fn add_http_entry(&mut self, name: &str, url: impl Into<String>) {
        self.build_entries
            .push(Entry::new(name.to_string(), EntrySource::HttpUrl(url.into())));
    }

    /// Add a filesystem entry with HTTP(S) fallback when the file is missing.
    pub fn add_filesystem_entry_with_http_fallback(
        &mut self,
        name: &str,
        path: impl Into<PathBuf>,
        fallback_url: impl Into<String>,
    ) {
        self.build_entries.push(Entry::new(
            name.to_string(),
            EntrySource::file_with_http_fallback(path, fallback_url),
        ));
    }

    /// Build an Actix stream that yields the ZIP file bytes.
    ///
    /// The ZIP is produced in a background Tokio task. This stream concurrently:
    /// - yields produced chunks as they arrive; and
    /// - watches the writer task result, so errors are surfaced as 500.
    pub fn build(mut self) -> impl Stream<Item = Result<Bytes, Error>> {
        // Keep insertion order.
        let entries = std::mem::take(&mut self.build_entries);
        let http_client = self.http_client.clone();

        let (tx_bytes, rx_bytes) = tokio::sync::mpsc::channel::<Bytes>(CHANNEL_CAPACITY);
        let writer = ChannelWriter::new(tx_bytes);
        let (tx, rx) = oneshot::channel::<anyhow::Result<()>>();

        tokio::spawn(async move {
            let res = write_zip(entries, writer, http_client).await;
            let _ = tx.send(res);
        });

        stream! {
            let mut reader_stream = ReceiverStream::new(rx_bytes);
            let mut writer_result = Some(rx);

            loop {
                if let Some(rx) = writer_result.as_mut() {
                    // Race ZIP writer completion vs. more bytes arriving.
                    //
                    // - If the writer finishes successfully, we keep draining the channel
                    //   until it naturally ends.
                    // - If it errors or gets cancelled, we surface a 500 and stop.
                    tokio::select! {
                        res = rx => {
                            writer_result = None;
                            match res {
                                Ok(Ok(())) => {
                                    // Writer finished successfully; keep draining until EOF.
                                }
                                Ok(Err(e)) => {
                                    log::error!("ZIP build failed: {e:?}");
                                    yield Err(ErrorInternalServerError(""));
                                    break;
                                }
                                Err(_) => {
                                    log::error!("ZIP writer task cancelled");
                                    yield Err(ErrorInternalServerError(""));
                                    break;
                                }
                            }
                        }
                        chunk = reader_stream.next() => {
                            match chunk {
                                Some(bytes) => yield Ok(bytes),
                                None => break,
                            }
                        }
                    }
                } else {
                    // Writer already reported completion; just drain remaining chunks.
                    match reader_stream.next().await {
                        Some(bytes) => yield Ok(bytes),
                        None => break,
                    }
                }
            }
        }
    }
}

async fn write_zip<W>(
    entries: Vec<Entry>,
    writer: W,
    http_client: Option<reqwest::Client>,
) -> anyhow::Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    use async_zip::base::write::ZipFileWriter;
    use async_zip::{Compression, ZipEntryBuilder};
    use futures::io::AsyncWriteExt as FuturesAsyncWriteExt;

    let mut zip = ZipFileWriter::with_tokio(writer).force_zip64();
    zip.comment(CREATED_BY.to_string());

    // Reuse a single buffer for filesystem reads to avoid per-entry allocations.
    let mut file_read_buffer = vec![0u8; FILE_READ_BUF_BYTES];

    for entry in entries {
        log::debug!("Add ZIP entry: {} ({:?})", entry.name, entry.source);

        // Always use Stored (no compression) and always write a streaming entry.
        // This avoids buffering whole instances in memory.
        let zip_entry = ZipEntryBuilder::new(entry.name.into(), Compression::Stored).build();
        let mut entry_writer = zip.write_entry_stream(zip_entry).await?;

        match entry.source {
            EntrySource::FilesystemPath(path) => {
                stream_file_into_zip_entry(&path, &mut file_read_buffer, &mut entry_writer).await?
            }
            EntrySource::HttpUrl(url) => {
                stream_http_into_zip_entry(&url, http_client.as_ref(), &mut entry_writer).await?
            }
            EntrySource::FilesystemWithHttpFallback { path, fallback_url } => {
                // Fallback is only safe if we didn't write any bytes yet.
                // Therefore we only fallback when the initial open() fails with NotFound.
                match tokio::fs::File::open(&path).await {
                    Ok(handler) => {
                        stream_open_file_into_zip_entry(
                            handler,
                            &path,
                            &mut file_read_buffer,
                            &mut entry_writer,
                        )
                        .await?
                    }
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {
                        log::warn!(
                            "Filesystem entry missing ({}), falling back to HTTP for {}",
                            path.display(),
                            fallback_url
                        );
                        stream_http_into_zip_entry(&fallback_url, http_client.as_ref(), &mut entry_writer).await?
                    }
                    Err(e) => {
                        return Err(anyhow::Error::new(e)
                            .context(format!("Failed to open file `{}`", path.display())));
                    }
                }
            }
        }

        // Finalize entry (writes data descriptor, updates central directory info, etc.).
        entry_writer.close().await?;
    }

    // Finalize the ZIP (central directory + end-of-central-directory, ZIP64 records).
    // Important: ensure the underlying writer is shutdown so any coalesced bytes
    // are flushed to the response stream.
    let mut writer = zip.close().await?;
    writer.close().await?;
    Ok(())
}

async fn stream_file_into_zip_entry(
    path: &Path,
    buffer: &mut [u8],
    entry_writer: &mut (impl futures::io::AsyncWrite + Unpin),
) -> anyhow::Result<()> {
    // Filesystem source.
    let handler = tokio::fs::File::open(path)
        .await
        .with_context(|| format!("Failed to open file `{}`", path.display()))?;

    stream_open_file_into_zip_entry(handler, path, buffer, entry_writer).await
}

async fn stream_open_file_into_zip_entry(
    mut handler: tokio::fs::File,
    path: &Path,
    buffer: &mut [u8],
    entry_writer: &mut (impl futures::io::AsyncWrite + Unpin),
) -> anyhow::Result<()> {

    loop {
        let n = handler
            .read(buffer)
            .await
            .with_context(|| format!("Failed to read file `{}`", path.display()))?;
        if n == 0 {
            break;
        }
        entry_writer
            .write_all(&buffer[..n])
            .await
            .context("Failed to write ZIP entry bytes")?;
    }

    Ok(())
}

async fn stream_http_into_zip_entry(
    url: &str,
    http_client: Option<&reqwest::Client>,
    entry_writer: &mut (impl futures::io::AsyncWrite + Unpin),
) -> anyhow::Result<()> {
    // HTTP source (WADO, proxy, etc.). The injected reqwest client provides pooling.
    let client = http_client.ok_or_else(|| {
        anyhow::anyhow!("HTTP client not configured (set it via DicomStreamZip::set_http_client)")
    })?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to GET `{}`: {e}", url))?;

    let resp = resp
        .error_for_status()
        .map_err(|e| anyhow::anyhow!("Non-success status for `{}`: {e}", url))?;

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|e| anyhow::anyhow!("Failed to read HTTP body from `{}`: {e}", url))?;
        entry_writer
            .write_all(chunk.as_ref())
            .await
            .context("Failed to write ZIP entry bytes")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::DicomStreamZip;
    use futures::StreamExt;
    use std::io::{Cursor, Read, Write};

    #[tokio::test]
    async fn dicom_stream_zip_zip64_store_smoke() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");

        let p1 = temp_dir.path().join("one.dcm");
        let p2 = temp_dir.path().join("two.dcm");

        let b1: Vec<u8> = (0u8..=255u8).collect();
        let b2: Vec<u8> = b"DICOM_TEST_PAYLOAD_2".to_vec();

        {
            let mut f = std::fs::File::create(&p1).expect("create file 1");
            f.write_all(&b1).expect("write file 1");
        }
        {
            let mut f = std::fs::File::create(&p2).expect("create file 2");
            f.write_all(&b2).expect("write file 2");
        }

        let mut zip = DicomStreamZip::new();
        zip.add_filesystem_entry("0001.dcm", p1);
        zip.add_filesystem_entry("0002.dcm", p2);

        let mut bytes = Vec::new();
        let stream = zip.build();
        futures::pin_mut!(stream);
        while let Some(item) = stream.next().await {
            let chunk = item.expect("zip stream chunk ok");
            bytes.extend_from_slice(&chunk);
        }

        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("open zip");
        assert_eq!(archive.len(), 2);

        assert_eq!(
            archive.comment(),
            b"Created by Opendicom - Sirius HIP (www.opendicom.com)"
        );

        {
            let mut f1 = archive.by_name("0001.dcm").expect("entry 0001.dcm");
            let mut out1 = Vec::new();
            f1.read_to_end(&mut out1).expect("read entry 1");
            assert_eq!(out1, b1);
        }

        {
            let mut f2 = archive.by_name("0002.dcm").expect("entry 0002.dcm");
            let mut out2 = Vec::new();
            f2.read_to_end(&mut out2).expect("read entry 2");
            assert_eq!(out2, b2);
        }
    }
}
