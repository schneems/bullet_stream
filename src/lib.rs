#![doc = include_str!("../README.md")]

use crate::ansi_escape::ANSI;
use crate::util::{
    mpsc_stream_to_output, prefix_first_rest_lines, prefix_lines, ParagraphInspectWrite,
};
use crate::write::line_mapped;
use std::fmt::Debug;
use std::io::Write;
use std::time::Instant;

mod ansi_escape;
mod background_printer;
mod duration_format;
pub mod style;
mod util;
mod write;

/// Use [`Output`] to output structured text as a buildpack/script executes. The output
/// is intended to be read by the application user.
///
/// ```rust
/// use bullet_stream::Output;
///
/// let mut output = Output::new(std::io::stdout())
///     .h2("Example Buildpack")
///     .warning("No Gemfile.lock found");
///
/// output = output
///     .bullet("Ruby version")
///     .done();
///
/// output.done();
/// ```
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct Output<T> {
    pub(crate) started: Option<Instant>,
    pub(crate) state: T,
}

/// Various states for [`Output`] to contain.
///
/// The [`Output`] struct acts as an output state machine. These structs
/// represent the various states. See struct documentation for more details.
pub mod state {
    use crate::background_printer::PrintGuard;
    use crate::util::ParagraphInspectWrite;
    use crate::write::MappedWrite;
    use std::time::Instant;

    /// At the start of a stream you can output a header (h1) or subheader (h2).
    ///
    /// In this state, represented by `state::Header` the user hasn't seen any output yet.
    /// You can have multiple subheaders (h2) but only one header (h1), so as soon as
    /// h1 is called you the state will be transitioned to `state::Bullet`.
    ///
    /// If using for a buildpack output, consider that each buildpack is run via a top level
    /// context which could be considered H1. Therefore each buildpack should announce it's name
    /// via the `h2` function.
    ///
    /// Example:
    ///
    /// ```rust
    /// use bullet_stream::{Output, state::{Bullet, Header}};
    /// use std::io::Write;
    ///
    /// let mut not_started = Output::new(std::io::stdout());
    /// let output = start_buildpack(not_started);
    ///
    /// output.bullet("Ruby version").sub_bullet("Installing Ruby").done();
    ///
    /// fn start_buildpack<W>(mut output: Output<Header<W>>) -> Output<Bullet<W>>
    /// where W: Write + Send + Sync + 'static {
    ///     output.h2("Example Buildpack")
    ///}
    /// ```
    #[derive(Debug)]
    pub struct Header<W> {
        pub(crate) write: ParagraphInspectWrite<W>,
    }

    /// After the buildpack output has started, its top-level output will be represented by the
    /// `state::Bullet` type and is transitioned into a `state::SubBullet` to provide additional
    /// details.
    ///
    /// Example:
    ///
    /// ```rust
    /// use bullet_stream::{Output, state::{Bullet, Header, SubBullet}};
    /// use std::io::Write;
    ///
    /// let mut output = Output::new(std::io::stdout())
    ///     .h2("Example Buildpack");
    ///
    /// output = install_ruby(output).done();
    ///
    /// fn install_ruby<W>(mut output: Output<Bullet<W>>) -> Output<SubBullet<W>>
    /// where W: Write + Send + Sync + 'static {
    ///     let out = output.bullet("Ruby version")
    ///         .sub_bullet("Installing Ruby");
    ///     // ...
    ///     out
    ///}
    /// ```
    #[derive(Debug)]
    pub struct Bullet<W> {
        pub(crate) write: ParagraphInspectWrite<W>,
    }

    /// The `state::SubBullet` is intended to provide additional details about the buildpack's
    /// actions. When a section is finished, it transitions back to a `state::Bullet` type.
    ///
    /// A streaming type can be started from a `state::Bullet`, usually to run and stream a
    /// `process::Command` to the end user.
    ///
    /// Example:
    ///
    /// ```rust
    /// use bullet_stream::{Output, state::{Bullet, SubBullet}};
    /// use std::io::Write;
    ///
    /// let mut output = Output::new(std::io::stdout())
    ///     .h2("Example Buildpack")
    ///     .bullet("Ruby version");
    ///
    /// install_ruby(output).done();
    ///
    /// fn install_ruby<W>(mut output: Output<SubBullet<W>>) -> Output<Bullet<W>>
    /// where W: Write + Send + Sync + 'static {
    ///     let output = output.sub_bullet("Installing Ruby");
    ///     // ...
    ///
    ///     output.done()
    ///}
    /// ```
    #[derive(Debug)]
    pub struct SubBullet<W> {
        pub(crate) write: ParagraphInspectWrite<W>,
    }

    /// This state is intended for streaming output from a process to the end user. It is
    /// started from a `state::SubBullet` and finished back to a `state::SubBullet`.
    ///
    /// The `Output<state::Stream<W>>` implements [`std::io::Write`], so you can stream
    /// from anything that accepts a [`std::io::Write`].
    ///
    /// ```rust
    /// use bullet_stream::{Output, state::{Bullet, SubBullet}};
    /// use std::io::Write;
    ///
    /// let mut output = Output::new(std::io::stdout())
    ///     .h2("Example Buildpack")
    ///     .bullet("Ruby version");
    ///
    /// install_ruby(output).done();
    ///
    /// fn install_ruby<W>(mut output: Output<SubBullet<W>>) -> Output<SubBullet<W>>
    /// where W: Write + Send + Sync + 'static {
    ///     let mut stream = output.sub_bullet("Installing Ruby")
    ///         .start_stream("Streaming stuff");
    ///
    ///     write!(&mut stream, "...").unwrap();
    ///
    ///     stream.done()
    ///}
    /// ```
    #[derive(Debug)]
    pub struct Stream<W: std::io::Write> {
        pub(crate) started: Instant,
        pub(crate) write: MappedWrite<ParagraphInspectWrite<W>>,
    }

    /// This state is intended for long-running tasks that do not stream but wish to convey progress
    /// to the end user. For example, while downloading a file.
    ///
    /// This state is started from a [`SubBullet`] and finished back to a [`SubBullet`].
    ///
    /// ```rust
    /// use bullet_stream::{Output, state::{Bullet, SubBullet}};
    /// use std::io::Write;
    ///
    /// let mut output = Output::new(std::io::stdout())
    ///     .h2("Example Buildpack")
    ///     .bullet("Ruby version");
    ///
    /// install_ruby(output).done();
    ///
    /// fn install_ruby<W>(mut output: Output<SubBullet<W>>) -> Output<SubBullet<W>>
    /// where W: Write + Send + Sync + 'static {
    ///     let mut timer = output.sub_bullet("Installing Ruby")
    ///         .start_timer("Installing");
    ///
    ///     /// ...
    ///
    ///     timer.done()
    ///}
    /// ```
    #[derive(Debug)]
    pub struct Background<W: std::io::Write> {
        pub(crate) started: Instant,
        pub(crate) write: PrintGuard<ParagraphInspectWrite<W>>,
    }
}

/// Used for announcements such as warning and error states
trait AnnounceSupportedState {
    type Inner: Write;

    fn write_mut(&mut self) -> &mut ParagraphInspectWrite<Self::Inner>;
}

/// Used for announcements such as warning and error states
impl<W> AnnounceSupportedState for state::SubBullet<W>
where
    W: Write,
{
    type Inner = W;

    fn write_mut(&mut self) -> &mut ParagraphInspectWrite<Self::Inner> {
        &mut self.write
    }
}

/// Used for announcements such as warning and error states
impl<W> AnnounceSupportedState for state::Bullet<W>
where
    W: Write,
{
    type Inner = W;

    fn write_mut(&mut self) -> &mut ParagraphInspectWrite<Self::Inner> {
        &mut self.write
    }
}

/// Used for announcements such as warning and error states
#[allow(private_bounds)]
impl<S> Output<S>
where
    S: AnnounceSupportedState,
{
    /// Emit an error and end the build output.
    ///
    /// When an unrecoverable situation is encountered, you can emit an error message to the user.
    /// This associated function will consume the build output, so you may only emit one error per
    /// build output.
    ///
    /// An error message should describe what went wrong and why the buildpack cannot continue.
    /// It is best practice to include debugging information in the error message. For example,
    /// if a file is missing, consider showing the user the contents of the directory where the
    /// file was expected to be and the full path of the file.
    ///
    /// If you are confident about what action needs to be taken to fix the error, you should include
    /// that in the error message. Do not write a generic suggestion like "try again later" unless
    /// you are certain that the error is transient.
    ///
    /// If you detect something problematic but not bad enough to halt buildpack execution, consider
    /// using a [`Output::warning`] instead.
    pub fn error(mut self, s: impl AsRef<str>) {
        self.write_paragraph(&ANSI::Red, s);
    }

    /// Emit a warning message to the end user.
    ///
    /// A warning should be used to emit a message to the end user about a potential problem.
    ///
    /// Multiple warnings can be emitted in sequence. The buildpack author should take care not to
    /// overwhelm the end user with unnecessary warnings.
    ///
    /// When emitting a warning, describe the problem to the user, if possible, and tell them how
    /// to fix it or where to look next.
    ///
    /// Warnings should often come with some disabling mechanism, if possible. If the user can turn
    /// off the warning, that information should be included in the warning message. If you're
    /// confident that the user should not be able to turn off a warning, consider using a
    /// [`Output::error`] instead.
    ///
    /// Warnings will be output in a multi-line paragraph style. A warning can be emitted from any
    /// state except for [`state::Header`].
    #[must_use]
    pub fn warning(mut self, s: impl AsRef<str>) -> Output<S> {
        self.write_paragraph(&ANSI::Yellow, s);
        self
    }

    /// Emit an important message to the end user.
    ///
    /// When something significant happens but is not inherently negative, you can use an important
    /// message. For example, if a buildpack detects that the operating system or architecture has
    /// changed since the last build, it might not be a problem, but if something goes wrong, the
    /// user should know about it.
    ///
    /// Important messages should be used sparingly and only for things the user should be aware of
    /// but not necessarily act on. If the message is actionable, consider using a
    /// [`Output::warning`] instead.
    #[must_use]
    pub fn important(mut self, s: impl AsRef<str>) -> Output<S> {
        self.write_paragraph(&ANSI::BoldCyan, s);
        self
    }

    fn write_paragraph(&mut self, color: &ANSI, s: impl AsRef<str>) {
        let io = self.state.write_mut();
        let contents = s.as_ref().trim();

        if !io.was_paragraph {
            writeln_now(io, "");
        }

        writeln_now(
            io,
            ansi_escape::wrap_ansi_escape_each_line(
                color,
                prefix_lines(contents, |_, line| {
                    // Avoid adding trailing whitespace to the line, if there was none already.
                    // The `\n` case is required since `prefix_lines` uses `str::split_inclusive`,
                    // which preserves any trailing newline characters if present.
                    if line.is_empty() || line == "\n" {
                        String::from("!")
                    } else {
                        String::from("! ")
                    }
                }),
            ),
        );
        writeln_now(io, "");
    }
}

impl<W> Output<state::Header<W>>
where
    W: Write,
{
    /// Create a buildpack output struct, but do not announce the buildpack's start.
    ///
    /// See the [`Output::h1`] and [`Output::h2`] methods for more details.
    #[must_use]
    pub fn new(io: W) -> Self {
        Self {
            state: state::Header {
                write: ParagraphInspectWrite::new(io),
            },
            started: None,
        }
    }

    /// Announce the start of the buildpack.
    ///
    /// The input should be the human-readable name of your buildpack. Most buildpack names include
    /// the feature they provide.
    ///
    /// It is common to use a title case for the buildpack name and to include the word "Buildpack" at the end.
    /// For example, `Ruby Buildpack`. Do not include a period at the end of the name.
    ///
    /// Avoid starting your buildpack with "Heroku" unless you work for Heroku. If you wish to express that your
    /// buildpack is built to target only Heroku; you can include that in the description of the buildpack.
    ///
    /// This function will transition your buildpack output to [`state::Bullet`].
    #[must_use]
    pub fn h1(mut self, buildpack_name: impl AsRef<str>) -> Output<state::Bullet<W>> {
        writeln_now(
            &mut self.state.write,
            ansi_escape::wrap_ansi_escape_each_line(
                &ANSI::BoldPurple,
                format!("\n# {}\n", buildpack_name.as_ref().trim()),
            ),
        );

        self.without_header()
    }

    /// Announce the start of the buildpack.
    ///
    /// The input should be the human-readable name of your buildpack. Most buildpack names include
    /// the feature they provide.
    ///
    /// It is common to use a title case for the buildpack name and to include the word "Buildpack" at the end.
    /// For example, `Ruby Buildpack`. Do not include a period at the end of the name.
    ///
    /// Avoid starting your buildpack with "Heroku" unless you work for Heroku. If you wish to express that your
    /// buildpack is built to target only Heroku; you can include that in the description of the buildpack.
    ///
    /// This function will transition your buildpack output to [`state::Bullet`].
    #[must_use]
    pub fn h2(mut self, buildpack_name: impl AsRef<str>) -> Output<state::Bullet<W>> {
        if !self.state.write.was_paragraph {
            writeln_now(&mut self.state.write, "");
        }

        writeln_now(
            &mut self.state.write,
            ansi_escape::wrap_ansi_escape_each_line(
                &ANSI::BoldPurple,
                format!("## {}\n", buildpack_name.as_ref().trim()),
            ),
        );

        self.without_header()
    }

    /// Start a buildpack output without announcing the name.
    #[must_use]
    pub fn without_header(self) -> Output<state::Bullet<W>> {
        Output {
            started: Some(Instant::now()),
            state: state::Bullet {
                write: self.state.write,
            },
        }
    }
}

impl<W> Output<state::Bullet<W>>
where
    W: Write + Send + Sync + 'static,
{
    const PREFIX_FIRST: &'static str = "- ";
    const PREFIX_REST: &'static str = "  ";

    fn style(s: impl AsRef<str>) -> String {
        prefix_first_rest_lines(Self::PREFIX_FIRST, Self::PREFIX_REST, s.as_ref().trim())
    }

    /// A top-level bullet point section
    ///
    /// A section should be a noun, e.g., 'Ruby version'. Anything emitted within the section
    /// should be in the context of this output.
    ///
    /// If the following steps can change based on input, consider grouping shared information
    /// such as version numbers and sources in the section name e.g.,
    /// 'Ruby version ``3.1.3`` from ``Gemfile.lock``'.
    ///
    /// This function will transition your buildpack output to [`state::SubBullet`].
    #[must_use]
    pub fn bullet(mut self, s: impl AsRef<str>) -> Output<state::SubBullet<W>> {
        writeln_now(&mut self.state.write, Self::style(s));

        Output {
            started: self.started,
            state: state::SubBullet {
                write: self.state.write,
            },
        }
    }

    /// Outputs an H2 header
    #[must_use]
    pub fn h2(mut self, buildpack_name: impl AsRef<str>) -> Output<state::Bullet<W>> {
        if !self.state.write.was_paragraph {
            writeln_now(&mut self.state.write, "");
        }

        writeln_now(
            &mut self.state.write,
            ansi_escape::wrap_ansi_escape_each_line(
                &ANSI::BoldPurple,
                format!("## {}\n", buildpack_name.as_ref().trim()),
            ),
        );

        self
    }

    /// Announce that your buildpack has finished execution successfully.
    pub fn done(mut self) -> W {
        if let Some(started) = &self.started {
            let elapsed = duration_format::human(&started.elapsed());
            let details = style::details(format!("finished in {elapsed}"));
            writeln_now(
                &mut self.state.write,
                Self::style(format!("Done {details}")),
            );
        } else {
            writeln_now(&mut self.state.write, Self::style("Done"));
        }

        self.state.write.inner
    }
}

impl<W> Output<state::Background<W>>
where
    W: Write + Send + Sync + 'static,
{
    /// Finalize a timer's output.
    ///
    /// Once you're finished with your long running task, calling this function
    /// finalizes the timer's output and transitions back to a [`state::SubBullet`].
    #[must_use]
    pub fn done(self) -> Output<state::SubBullet<W>> {
        let duration = self.state.started.elapsed();
        let mut io = match self.state.write.stop() {
            Ok(io) => io,
            // Stdlib docs recommend using `resume_unwind` to resume the thread panic
            // <https://doc.rust-lang.org/std/thread/type.Result.html>
            Err(e) => std::panic::resume_unwind(e),
        };

        writeln_now(&mut io, style::details(duration_format::human(&duration)));
        Output {
            started: self.started,
            state: state::SubBullet { write: io },
        }
    }
}

impl<W> Output<state::SubBullet<W>>
where
    W: Write + Send + Sync + 'static,
{
    const PREFIX_FIRST: &'static str = "  - ";
    const PREFIX_REST: &'static str = "    ";
    const CMD_INDENT: &'static str = "      ";

    fn style(s: impl AsRef<str>) -> String {
        prefix_first_rest_lines(Self::PREFIX_FIRST, Self::PREFIX_REST, s.as_ref().trim())
    }

    /// Emit a sub bullet point step in the output under a bullet point.
    ///
    /// A step should be a verb, i.e., 'Downloading'. Related verbs should be nested under a single section.
    ///
    /// Some example verbs to use:
    ///
    /// - Downloading
    /// - Writing
    /// - Using
    /// - Reading
    /// - Clearing
    /// - Skipping
    /// - Detecting
    /// - Compiling
    /// - etc.
    ///
    /// Steps should be short and stand-alone sentences within the context of the section header.
    ///
    /// In general, if the buildpack did something different between two builds, it should be
    /// observable by the user through the buildpack output. For example, if a cache needs to be
    /// cleared, emit that your buildpack is clearing it and why.
    ///
    /// Multiple steps are allowed within a section. This function returns to the same [`state::SubBullet`].
    #[must_use]
    pub fn sub_bullet(mut self, s: impl AsRef<str>) -> Output<state::SubBullet<W>> {
        writeln_now(&mut self.state.write, Self::style(s));
        self
    }

    /// Stream output to the end user.
    ///
    /// The most common use case is to stream the output of a running `std::process::Command` to the
    /// end user. Streaming lets the end user know that something is happening and provides them with
    /// the output of the process.
    ///
    /// The result of this function is a `Output<state::Stream<W>>` which implements [`std::io::Write`].
    ///
    /// If you do not wish the end user to view the output of the process, consider using a `step` instead.
    ///
    /// This function will transition your buildpack output to [`state::Stream`].
    #[must_use]
    pub fn start_stream(mut self, s: impl AsRef<str>) -> Output<state::Stream<W>> {
        writeln_now(&mut self.state.write, Self::style(s));
        writeln_now(&mut self.state.write, "");

        Output {
            started: self.started,
            state: state::Stream {
                started: Instant::now(),
                write: line_mapped(self.state.write, |mut line| {
                    // Avoid adding trailing whitespace to the line, if there was none already.
                    // The `[b'\n']` case is required since `line` includes the trailing newline byte.
                    if line.is_empty() || line == [b'\n'] {
                        line
                    } else {
                        let mut result: Vec<u8> = Self::CMD_INDENT.into();
                        result.append(&mut line);
                        result
                    }
                }),
            },
        }
    }

    /// Output periodic timer updates to the end user.
    ///
    /// If a buildpack author wishes to start a long-running task that does not stream, starting a timer
    /// will let the user know that the buildpack is performing work and that the UI is not stuck.
    ///
    /// One common use case is when downloading a file. Emitting periodic output when downloading is especially important for the local
    /// buildpack development experience where the user's network may be unexpectedly slow, such as
    /// in a hotel or on a plane.
    ///
    /// This function will transition your buildpack output to [`state::Background`].
    #[allow(clippy::missing_panics_doc)]
    pub fn start_timer(mut self, s: impl AsRef<str>) -> Output<state::Background<W>> {
        // Do not emit a newline after the message
        write!(self.state.write, "{}", Self::style(s)).expect("Output error: UI writer closed");
        self.state
            .write
            .flush()
            .expect("Output error: UI writer closed");

        Output {
            started: self.started,
            state: state::Background {
                started: Instant::now(),
                write: background_printer::print_interval(
                    self.state.write,
                    std::time::Duration::from_secs(1),
                    ansi_escape::wrap_ansi_escape_each_line(&ANSI::Dim, " ."),
                    ansi_escape::wrap_ansi_escape_each_line(&ANSI::Dim, "."),
                    ansi_escape::wrap_ansi_escape_each_line(&ANSI::Dim, ". "),
                ),
            },
        }
    }

    fn format_stream_writer<S>(stream_to: S) -> crate::write::MappedWrite<S>
    where
        S: Write + Send + Sync,
    {
        line_mapped(stream_to, |mut line| {
            // Avoid adding trailing whitespace to the line, if there was none already.
            // The `[b'\n']` case is required since `line` includes the trailing newline byte.
            if line.is_empty() || line == [b'\n'] {
                line
            } else {
                let mut result: Vec<u8> = Self::CMD_INDENT.into();
                result.append(&mut line);
                result
            }
        })
    }

    /// Stream two inputs without consuming
    ///
    /// The `start_stream` returns a single writer, but running a command often requires two.
    /// This function allows you to stream both stdout and stderr to the end user using a single writer.
    ///
    /// It takes a step string that will be advertized and a closure that takes two writers and returns a value.
    /// The return value is returned from the function.
    ///
    /// Example:
    ///
    ///
    /// ```no_run
    /// use bullet_stream::{style, Output};
    /// use fun_run::CommandWithName;
    /// use std::process::Command;
    ///
    /// let mut output = Output::new(std::io::stdout())
    ///     .h2("Example Buildpack")
    ///     .bullet("Streaming");
    ///
    /// let mut cmd = Command::new("echo");
    /// cmd.arg("hello world");
    ///
    /// // Use the result of the Streamed command
    /// let result = output.stream_with(
    ///     format!("Running {}", style::command(cmd.name())),
    ///     |stdout, stderr| cmd.stream_output(stdout, stderr),
    /// );
    ///
    /// output.done().done();
    /// ```
    #[allow(clippy::missing_panics_doc)]
    pub fn stream_with<F, T>(&mut self, s: impl AsRef<str>, mut f: F) -> T
    where
        F: FnMut(Box<dyn Write + Send + Sync>, Box<dyn Write + Send + Sync>) -> T,
        T: 'static,
    {
        writeln_now(&mut self.state.write, Self::style(s));
        writeln_now(&mut self.state.write, "");

        let duration = Instant::now();
        mpsc_stream_to_output(
            |sender| {
                f(
                    // The Senders are boxed to hide the types from the caller so it can be changed
                    // in the future. They only need to know they have a `Write + Send + Sync` type.
                    Box::new(Self::format_stream_writer(sender.clone())),
                    Box::new(Self::format_stream_writer(sender.clone())),
                )
            },
            move |recv| {
                // When it receives input, it writes it to the current `Write` value.
                //
                // When the senders close their channel this loop will exit
                for message in recv {
                    self.state
                        .write
                        .write_all(&message)
                        .expect("Writer to not be closed");
                }

                if !self.state.write_mut().was_paragraph {
                    writeln_now(&mut self.state.write, "");
                }

                writeln_now(
                    &mut self.state.write,
                    Self::style(format!(
                        "Done {}",
                        style::details(duration_format::human(&duration.elapsed()))
                    )),
                );
            },
        )
    }

    /// Finish a section and transition back to [`state::Bullet`].
    pub fn done(self) -> Output<state::Bullet<W>> {
        Output {
            started: self.started,
            state: state::Bullet {
                write: self.state.write,
            },
        }
    }
}

impl<W> Output<state::Stream<W>>
where
    W: Write + Send + Sync + 'static,
{
    /// Finalize a stream's output
    ///
    /// Once you're finished streaming to the output, calling this function
    /// finalizes the stream's output and transitions back to a [`state::Bullet`].
    pub fn done(self) -> Output<state::SubBullet<W>> {
        let duration = self.state.started.elapsed();

        let mut output = Output {
            started: self.started,
            state: state::SubBullet {
                write: self.state.write.unwrap(),
            },
        };

        if !output.state.write_mut().was_paragraph {
            writeln_now(&mut output.state.write, "");
        }

        output.sub_bullet(format!(
            "Done {}",
            style::details(duration_format::human(&duration))
        ))
    }
}

impl<W> Write for Output<state::Stream<W>>
where
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.state.write.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.state.write.flush()
    }
}

/// Internal helper, ensures that all contents are always flushed (never buffered).
fn writeln_now<D: Write>(destination: &mut D, msg: impl AsRef<str>) {
    writeln!(destination, "{}", msg.as_ref()).expect("Output error: UI writer closed");

    destination.flush().expect("Output error: UI writer closed");
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::util::LockedWriter;
    use fun_run::CommandWithName;
    use indoc::formatdoc;
    use libcnb_test::assert_contains;
    use std::fs::File;

    #[test]
    fn double_h2_h2_newlines() {
        let writer = Vec::new();
        let output = Output::new(writer).h2("Header 2").h2("Header 2");

        let io = output.done();
        let expected = formatdoc! {"

            ## Header 2

            ## Header 2

            - Done (finished in < 0.1s)
        "};

        assert_eq!(
            expected,
            strip_ansi_escape_sequences(String::from_utf8_lossy(&io))
        )
    }

    #[test]
    fn double_h1_h2_newlines() {
        let writer = Vec::new();
        let output = Output::new(writer).h1("Header 1").h2("Header 2");

        let io = output.done();
        let expected = formatdoc! {"

            # Header 1

            ## Header 2

            - Done (finished in < 0.1s)
        "};

        assert_eq!(
            expected,
            strip_ansi_escape_sequences(String::from_utf8_lossy(&io))
        )
    }

    #[test]
    fn stream_with() {
        let writer = Vec::new();
        let mut output = Output::new(writer)
            .h2("Example Buildpack")
            .bullet("Streaming");
        let mut cmd = std::process::Command::new("echo");
        cmd.arg("hello world");

        let _result = output.stream_with(
            format!("Running {}", style::command(cmd.name())),
            |stdout, stderr| cmd.stream_output(stdout, stderr),
        );

        let io = output.done().done();
        let expected = formatdoc! {"

            ## Example Buildpack

            - Streaming
              - Running `echo \"hello world\"`

                  hello world

              - Done (< 0.1s)
            - Done (finished in < 0.1s)
        "};

        assert_eq!(
            expected,
            strip_ansi_escape_sequences(String::from_utf8_lossy(&io))
        );
    }

    #[test]
    fn background_timer() {
        let io = Output::new(Vec::new())
            .without_header()
            .bullet("Background")
            .start_timer("Installing")
            .done()
            .done()
            .done();

        // Test human readable timer output
        let expected = formatdoc! {"
            - Background
              - Installing ... (< 0.1s)
            - Done (finished in < 0.1s)
        "};

        assert_eq!(
            expected,
            strip_ansi_escape_sequences(String::from_utf8_lossy(&io))
        );

        // Test timer dot colorization
        let expected = formatdoc! {"
            - Background
              - Installing\u{1b}[2;1m .\u{1b}[0m\u{1b}[2;1m.\u{1b}[0m\u{1b}[2;1m. \u{1b}[0m(< 0.1s)
            - Done (finished in < 0.1s)
        "};

        assert_eq!(expected, String::from_utf8_lossy(&io));
    }

    #[test]
    fn write_paragraph_empty_lines() {
        let io = Output::new(Vec::new())
            .h1("Example Buildpack\n\n")
            .warning("\n\nhello\n\n\t\t\nworld\n\n")
            .bullet("Version\n\n")
            .sub_bullet("Installing\n\n")
            .done()
            .done();

        let tab_char = '\t';
        let expected = formatdoc! {"

            # Example Buildpack

            ! hello
            !
            ! {tab_char}{tab_char}
            ! world

            - Version
              - Installing
            - Done (finished in < 0.1s)
        "};

        assert_eq!(
            expected,
            strip_ansi_escape_sequences(String::from_utf8_lossy(&io))
        );
    }

    #[test]
    fn paragraph_color_codes() {
        let tmpdir = tempfile::tempdir().unwrap();
        let path = tmpdir.path().join("output.txt");

        Output::new(File::create(&path).unwrap())
            .h1("Buildpack Header is Bold Purple")
            .important("Important is bold cyan")
            .warning("Warnings are yellow")
            .error("Errors are red");

        let expected = formatdoc! {"

            \u{1b}[1;35m# Buildpack Header is Bold Purple\u{1b}[0m

            \u{1b}[1;36m! Important is bold cyan\u{1b}[0m

            \u{1b}[0;33m! Warnings are yellow\u{1b}[0m

            \u{1b}[0;31m! Errors are red\u{1b}[0m

        "};

        assert_eq!(expected, std::fs::read_to_string(path).unwrap());
    }

    #[test]
    fn test_important() {
        let writer = Vec::new();
        let io = Output::new(writer)
            .h1("Heroku Ruby Buildpack")
            .important("This is important")
            .done();

        let expected = formatdoc! {"

            # Heroku Ruby Buildpack

            ! This is important

            - Done (finished in < 0.1s)
        "};

        assert_eq!(
            expected,
            strip_ansi_escape_sequences(String::from_utf8_lossy(&io))
        );
    }

    #[test]
    fn test_error() {
        let tmpdir = tempfile::tempdir().unwrap();
        let path = tmpdir.path().join("output.txt");

        Output::new(File::create(&path).unwrap())
            .h1("Heroku Ruby Buildpack")
            .error("This is an error");

        let expected = formatdoc! {"

            # Heroku Ruby Buildpack

            ! This is an error

        "};

        assert_eq!(
            expected,
            strip_ansi_escape_sequences(std::fs::read_to_string(path).unwrap())
        );
    }

    #[test]
    fn test_captures() {
        let writer = Vec::new();
        let mut first_stream = Output::new(writer)
            .h1("Heroku Ruby Buildpack")
            .bullet("Ruby version `3.1.3` from `Gemfile.lock`")
            .done()
            .bullet("Hello world")
            .start_stream("Streaming with no newlines");

        writeln!(&mut first_stream, "stuff").unwrap();

        let mut second_stream = first_stream
            .done()
            .start_stream("Streaming with blank lines and a trailing newline");

        writeln!(&mut second_stream, "foo\nbar\n\n\t\nbaz\n").unwrap();

        let io = second_stream.done().done().done();

        let tab_char = '\t';
        let expected = formatdoc! {"

            # Heroku Ruby Buildpack

            - Ruby version `3.1.3` from `Gemfile.lock`
            - Hello world
              - Streaming with no newlines

                  stuff

              - Done (< 0.1s)
              - Streaming with blank lines and a trailing newline

                  foo
                  bar

                  {tab_char}
                  baz

              - Done (< 0.1s)
            - Done (finished in < 0.1s)
        "};

        assert_eq!(
            expected,
            strip_ansi_escape_sequences(String::from_utf8_lossy(&io))
        );
    }

    #[test]
    fn test_streaming_a_command() {
        let writer = Vec::new();
        let mut stream = Output::new(writer)
            .h1("Streaming buildpack demo")
            .bullet("Command streaming")
            .start_stream("Streaming stuff");

        let locked_writer = LockedWriter::new(stream);

        std::process::Command::new("echo")
            .arg("hello world")
            .stream_output(locked_writer.clone(), locked_writer.clone())
            .unwrap();

        stream = locked_writer.unwrap();

        let io = stream.done().done().done();

        let actual = strip_ansi_escape_sequences(String::from_utf8_lossy(&io));

        assert_contains!(actual, "      hello world\n");
    }

    #[test]
    fn warning_after_buildpack() {
        let writer = Vec::new();
        let io = Output::new(writer)
            .h1("RCT")
            .warning("It's too crowded here\nI'm tired")
            .bullet("Guest thoughts")
            .sub_bullet("The jumping fountains are great")
            .sub_bullet("The music is nice here")
            .done()
            .done();

        let expected = formatdoc! {"

            # RCT

            ! It's too crowded here
            ! I'm tired

            - Guest thoughts
              - The jumping fountains are great
              - The music is nice here
            - Done (finished in < 0.1s)
        "};

        assert_eq!(
            expected,
            strip_ansi_escape_sequences(String::from_utf8_lossy(&io))
        );
    }

    #[test]
    fn warning_step_padding() {
        let writer = Vec::new();
        let io = Output::new(writer)
            .h1("RCT")
            .bullet("Guest thoughts")
            .sub_bullet("The scenery here is wonderful")
            .warning("It's too crowded here\nI'm tired")
            .sub_bullet("The jumping fountains are great")
            .sub_bullet("The music is nice here")
            .done()
            .done();

        let expected = formatdoc! {"

            # RCT

            - Guest thoughts
              - The scenery here is wonderful

            ! It's too crowded here
            ! I'm tired

              - The jumping fountains are great
              - The music is nice here
            - Done (finished in < 0.1s)
        "};

        assert_eq!(
            expected,
            strip_ansi_escape_sequences(String::from_utf8_lossy(&io))
        );
    }

    #[test]
    fn double_warning_step_padding() {
        let writer = Vec::new();
        let output = Output::new(writer)
            .h1("RCT")
            .bullet("Guest thoughts")
            .sub_bullet("The scenery here is wonderful");

        let io = output
            .warning("It's too crowded here")
            .warning("I'm tired")
            .sub_bullet("The jumping fountains are great")
            .sub_bullet("The music is nice here")
            .done()
            .done();

        let expected = formatdoc! {"

            # RCT

            - Guest thoughts
              - The scenery here is wonderful

            ! It's too crowded here

            ! I'm tired

              - The jumping fountains are great
              - The music is nice here
            - Done (finished in < 0.1s)
        "};

        assert_eq!(
            expected,
            strip_ansi_escape_sequences(String::from_utf8_lossy(&io))
        );
    }

    fn strip_ansi_escape_sequences(contents: impl AsRef<str>) -> String {
        let mut result = String::new();
        let mut in_ansi_escape = false;
        for char in contents.as_ref().chars() {
            if in_ansi_escape {
                if char == 'm' {
                    in_ansi_escape = false;
                    continue;
                }
            } else {
                if char == '\x1B' {
                    in_ansi_escape = true;
                    continue;
                }

                result.push(char);
            }
        }

        result
    }
}
