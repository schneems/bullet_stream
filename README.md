# Bullet stream

Bulletproof printing for bullet point text

## What

An opinonated logger aimed at streaming text output (of scripts or buildpacks) to users. The format is loosely based on markdown headers and bulletpoints, hence the name.

## Why

This work started as a shared output format for Heroku's Cloud Native Buildpack (CNB) efforts, which are written in Rust. You can learn more about Heroku's [Cloud Native Buildpacks here](https://github.com/heroku/buildpacks).

# Use

Add bullet_stream to your project:

```ignore
$ cargo add bullet_stream
```

Now use [`Print`] to output structured text as a script/buildpack executes. The output
is intended to be read by the end user.

```rust
use bullet_stream::Print;

let mut output = Print::new(std::io::stdout())
    .h2("Example Buildpack")
    .warning("No Gemfile.lock found");

output = output
    .bullet("Ruby version")
    .done();

output.done();
```

## Living style guide

To view the output format and read a living style guide you can run:

```ignore
$ git clone <repo-url>
$ cargo run --example style_guide
```

## Colors

In nature, colors and contrasts are used to emphasize differences and danger. [`Output`]
utilizes common ANSI escape characters to highlight what's important and deemphasize what's not.
The output experience is designed from the ground up to be streamed to a user's terminal correctly.

## Consistent indentation and newlines

Help your users focus on what's happening, not on inconsistent formatting. The [`Output`]
is a consuming, stateful design. That means you can use Rust's powerful type system to ensure
only the output you expect, in the style you want, is emitted to the screen. See the documentation
in the [`state`] module for more information.

## Requirements

The project has some unique requirements that might not be obvious at first glance:

- Assume screen clearing is not available: Text UI tools such as progress bars rely on ANSI excape codes to clear and re-draw lines which simulates animation. A primary goal of this project is to be used in contexts like a git hook where each line is prefixed via `remote >`. The library provides tooling for alternative append-only "spinners" that denote the passage of time without requiring a screen re-draw.
- Atomic ANSI: Bullet stream uses ANSI codes to colorize output, but it cannot predict if/when the stream will be disconnected. In that event, we don't want to leave the user's screen accidentally blue (or some other color) and so the library favors always writing an ANSI reset code for every line of output. This also ensures any wrapped prefixes like a `remote >` are not accidentally colorized.
- Accessability over style: While the project uses ANSI codes to colorize output, it relies on the most common colors likely to be supported by most shells/terminals/command-prompts.
- Distinguish between owned and unowned output: Any messages a script author emits are "owned" while calling another process and streaming the output (like `bundle install`) are "unowned". Bullet stream uses leader characters and color to denote "owned" output while unowned output carries no markers and is generally indented.
- Favor ease of use over runtime performance: It's assumed that the script/buildpack will call commands and perform network or system IO that should dwarf the cost of allocating a String. It's not that this project aims to be needlessly expensive, however if raw streaming performance is your goal, this project is not for you.
