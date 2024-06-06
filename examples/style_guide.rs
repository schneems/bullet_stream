use ascii_table::AsciiTable;
#[allow(clippy::wildcard_imports)]
use bullet_stream::{style, Print};
use fun_run::CommandWithName;
use indoc::formatdoc;
use std::io::stdout;
use std::process::Command;

#[allow(clippy::too_many_lines)]
fn main() {
    {
        let mut log = Print::new(stdout()).h1("Living build output style guide");
        log = log.h2("Bullet section features");
        log = log
            .bullet("Bullet example")
            .sub_bullet("sub bullet example one")
            .sub_bullet("sub bullet example two")
            .done();

        log = log
            .bullet("Bullet section description")
            .sub_bullet(
                "A section should be a noun i.e. 'Ruby Version', consider this the section topic.",
            )
            .sub_bullet("A step should be a verb i.e. 'Downloading'")
            .sub_bullet("Related verbs should be nested under a single section")
            .sub_bullet(
                formatdoc! {"
                Steps can be multiple lines long
                However they're best as short, factual,
                descriptions of what the program is doing.
            "}
                .trim(),
            )
            .sub_bullet("Prefer a single line when possible")
            .sub_bullet("Sections and steps are sentence cased with no ending puncuation")
            .sub_bullet("HELP: capitalize the first letter")
            .done();

        let mut command = Command::new("bash");
        command.args(["-c", "ps aux | grep cargo"]);

        let mut sub_bullet = log.bullet("Timer steps")
        .sub_bullet("Long running code should execute with a timer to indicate the progam did not hang. Example:")
        .start_timer("Background progress timer")
        .done()
        .sub_bullet("Output can be streamed. Mostly from commands. Example:");

        let _result = sub_bullet.stream_with(
            format!("Running {}", style::command(command.name())),
            |stdout, stderr| command.stream_output(stdout, stderr),
        );

        sub_bullet.done();

        // // TODO: Remove usage of unwrap(): https://github.com/heroku/buildpacks-ruby/issues/238
        // #[allow(clippy::unwrap_used)]
        // command.stream_output(stream.io(), stream.io()).unwrap();
        // log = stream.finish_timed_stream().done();
        // drop(log);
    }

    {
        // TODO: Remove usage of unwrap(): https://github.com/heroku/buildpacks-ruby/issues/238
        #[allow(clippy::unwrap_used)]
        let cmd_error = Command::new("iDoNotExist").named_output().err().unwrap();

        let mut log = Print::new(stdout()).h2("Error and warnings");
        log = log
            .bullet("Debug information")
            .sub_bullet("Should go above errors in section/step format")
            .done();

        log = log
            .bullet(style::important("DEBUG INFO:"))
            .sub_bullet(cmd_error.to_string())
            .done();

        log.warning(formatdoc! {"
                Warning: This is a warning header

                This is a warning body. Warnings are for when we know for a fact a problem exists
                but it's not bad enough to abort the build.
            "})
            .important(formatdoc! {"
                Important: This is important

                Important is for when there's critical information that needs to be read
                however it may or may not be a problem. If we know for a fact that there's
                a problem then use a warning instead.

                An example of something that is important but might not be a problem is
                that an application owner upgraded to a new stack.
            "})
            .error(formatdoc! {"
                Error: This is an error header

                This is the error body. Use an error for when the build cannot continue.
                An error should include a header with a short description of why it cannot continue.

                The body should include what error state was observed, why that's a problem, and
                what remediation steps an application owner using the buildpack to deploy can
                take to solve the issue.
            "});
    }

    {
        let log = Print::new(stdout()).h2("Formatting helpers");
        log.bullet("The fmt module")
            .sub_bullet(formatdoc! {"
                Formatting helpers can be used to enhance log output:
            "})
            .done();

        let mut table = AsciiTable::default();
        table.set_max_width(240);
        table.column(0).set_header("Example");
        table.column(1).set_header("Code");
        table.column(2).set_header("When to use");

        let data: Vec<Vec<String>> = vec![
            vec![
                style::value("2.3.4"),
                "style::value(\"2.3.4\")".to_string(),
                "With versions, file names or other important values worth highlighting".to_string(),
            ],
            vec![
                style::url("https://www.schneems.com"),
                "style::url(\"https://www.schneems.com\")".to_string(),
                "With urls".to_string(),
            ],
            vec![
                style::command("bundle install"),
                "style::command(command.name())".to_string(),
                "With commands (alongside of `fun_run::CommandWithName`)".to_string(),
            ],
            vec![
                style::details("extra information"),
                "style::details(\"extra information\")".to_string(),
                "Add specific information at the end of a line i.e. 'Cache cleared (ruby version changed)'".to_string()
            ],
            vec![
                style::important("HELP:").to_string(),
                "style::important(\"HELP:\").to_string()".to_string(),
                "Call attention to individual words, useful when you want to emphasize a prefix but not the whole line.".to_string()
            ],
        ];

        table.print(data);
    }
}
