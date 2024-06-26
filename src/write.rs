use std::fmt::{Debug, Formatter};
use std::io;
use std::mem;
use std::sync::Arc;

/// Constructs a writer that buffers written data until given marker byte is encountered and
/// then applies the given mapping function to the data before passing the result to the wrapped
/// writer.
pub fn mapped<W: io::Write, F: (Fn(Vec<u8>) -> Vec<u8>) + Sync + Send + 'static>(
    w: W,
    marker_byte: u8,
    f: F,
) -> MappedWrite<W> {
    MappedWrite::new(w, marker_byte, f)
}

/// Constructs a writer that buffers written data until an ASCII/UTF-8 newline byte (`b'\n'`) is
/// encountered and then applies the given mapping function to the data before passing the result to
/// the wrapped writer.
pub fn line_mapped<W: io::Write, F: (Fn(Vec<u8>) -> Vec<u8>) + Sync + Send + 'static>(
    w: W,
    f: F,
) -> MappedWrite<W> {
    mapped(w, b'\n', f)
}

/// A mapped writer that was created with the [`mapped`] or [`line_mapped`] function.
#[derive(Clone)]
pub struct MappedWrite<W: io::Write> {
    // To support unwrapping the inner `Write` while also implementing `Drop` for final cleanup, we need to wrap the
    // `W` value so we can replace it in memory during unwrap. Without the wrapping `Option` we'd need to have a way
    // to construct a bogus `W` value which would require additional trait bounds for `W`. `Clone` and/or `Default`
    // come to mind. Not only would this clutter the API, but for most values that implement `Write`, `Clone` or
    // `Default` are hard to implement correctly as they most often involve system resources such as file handles.
    //
    // This semantically means that a `MappedWrite` can exist without an inner `Write`, but users of `MappedWrite` can
    // never construct such a `MappedWrite` as it only represents a state that happens during `MappedWrite::unwrap`.
    //
    // See: https://rustwiki.org/en/error-index/#E0509
    inner: Option<W>,
    marker_byte: u8,
    buffer: Vec<u8>,
    mapping_fn: Arc<dyn (Fn(Vec<u8>) -> Vec<u8>) + Sync + Send>,
}

impl<W> MappedWrite<W>
where
    W: io::Write,
{
    fn new<F: (Fn(Vec<u8>) -> Vec<u8>) + Sync + Send + 'static>(
        w: W,
        marker_byte: u8,
        f: F,
    ) -> MappedWrite<W> {
        MappedWrite {
            inner: Some(w),
            marker_byte,
            buffer: Vec::new(),
            mapping_fn: Arc::new(f),
        }
    }

    pub fn unwrap(mut self) -> W {
        // See `Drop` implementation. This logic cannot be de-duplicated (i.e. by using unwrap in `Drop`) as we would
        // end up in illegal states.
        if self.inner.is_some() {
            let _result = self.map_and_write_current_buffer();
        }

        if let Some(inner) = self.inner.take() {
            inner
        } else {
            // Since `unwrap` is the only function that will cause `self.inner` to be `None` and `unwrap` itself
            // consumes the `MappedWrite`, we can be sure that this case never happens.
            unreachable!("self.inner will never be None")
        }
    }

    fn map_and_write_current_buffer(&mut self) -> io::Result<()> {
        match self.inner {
            Some(ref mut inner) => inner.write_all(&(self.mapping_fn)(mem::take(&mut self.buffer))),
            None => Ok(()),
        }
    }
}

impl<W: io::Write> io::Write for MappedWrite<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for byte in buf {
            self.buffer.push(*byte);

            if *byte == self.marker_byte {
                self.map_and_write_current_buffer()?;
            }
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.inner {
            Some(ref mut inner) => inner.flush(),
            None => Ok(()),
        }
    }
}

impl<W: io::Write> Drop for MappedWrite<W> {
    fn drop(&mut self) {
        // Drop implementations must not panic. We intentionally ignore the potential error here.
        let _result = self.map_and_write_current_buffer();
    }
}

impl<W: io::Write + Debug> Debug for MappedWrite<W> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MappedWrite")
            .field("inner", &self.inner)
            .field("marker_byte", &self.marker_byte)
            .field("buffer", &self.buffer)
            .field("mapping_fn", &"Fn()")
            .finish()
    }
}

#[cfg(test)]
mod test {
    use crate::write::line_mapped;

    #[test]
    fn test_mapped_write() {
        let mut output = Vec::new();

        let mut input = "foo\nbar\nbaz".as_bytes();
        std::io::copy(
            &mut input,
            &mut line_mapped(&mut output, |line| line.repeat(2)),
        )
        .unwrap();

        assert_eq!(output, "foo\nfoo\nbar\nbar\nbazbaz".as_bytes());
    }
}
