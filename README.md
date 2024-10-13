# Bullet stream

Bulletproof printing for bullet point text

## What

An opinionated logger aimed at streaming text output (of scripts or buildpacks) to users. The format is loosely based on markdown headers and bullet points, hence the name.

## Why

This work started as a shared output format for Heroku's Cloud Native Buildpack (CNB) efforts, which are written in Rust. You can learn more about Heroku's [Cloud Native Buildpacks here](https://github.com/heroku/buildpacks).

# Use

Add `bullet_stream` to your project:

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

To view the output format and read a living style guide, you can run:

```ignore
$ git clone <repo-url>
$ cargo run --example style_guide
```

## Colors

In nature, colors and contrasts are used to emphasize differences and danger. [`Print`]
utilizes common ANSI escape characters to highlight what's important and deemphasize what's not.
The output experience is designed from the ground up to be streamed to a user's terminal correctly.

## Consistent indentation and newlines

Help your users focus on what's happening rather than on inconsistent formatting. The [`Print`] is a consuming, stateful design. That means you can use Rust's powerful type system to ensure
only the output you expect, in the style you want, is emitted to the screen. See the documentation
in the [`state`] module for more information.

## Requirements

The project has some unique requirements that might not be obvious at first glance:

- Assume screen clearing is not available: Text UI tools such as progress bars rely on ANSI escape codes to clear and redraw lines, which simulates animation. A primary goal of this project is to be used in contexts like a git hook, where each line is prefixed via `remote >`. The library provides tooling for alternative append-only "spinners" that denote the passage of time without requiring a screen redraw.
- Atomic ANSI: Bullet stream uses ANSI codes to colorize output, but it cannot predict if/when the stream will be disconnected. In that event, we don't want to leave the user's screen accidentally blue (or some other color), so the library favors always writing an ANSI reset code for every line of output. This also ensures that any wrapped prefixes like a `remote >` are not accidentally colorized.
- Accessibility over style: While the project uses ANSI codes to colorize output, it relies on the most common colors likely to be supported by most shells, terminals, and command prompts.
- Distinguish between owned and unowned output: Any messages a script author emits are "owned" while calling another process and streaming the output (like `bundle install`) are "unowned". Bullet stream uses leader characters and color to denote "owned" output, while unowned output carries no markers and is generally indented.
- Favor ease of use over runtime performance: It's assumed that the script/buildpack will call commands and perform network or system IO that should dwarf the cost of allocating a String. It's not that this project aims to be needlessly expensive; however, if raw streaming performance is your goal, this project is not for you.

## Usage

### Ricochet

The library design relies on a consuming struct design to guarantee output consistency. That means that you'll end up needing to assign the `bullet_stream` result just about every time you use it, for example:

```rust
use bullet_stream::{Print, state::{Bullet, Header, SubBullet}};
use std::io::Write;

let mut log = Print::new(std::io::stderr()).h1("Building Ruby");
log = {
    let mut bullet = log.bullet("Doing things");
    // ..
    bullet.done()
};
log = {
    let mut bullet = log.bullet("Noun");
    // ...
    bullet = bullet.sub_bullet("Verb");
    // ...
    bullet = bullet.sub_bullet("Another verb");
    // ...
    bullet.done()
};
log.done();
```

### Push logic down, bubble information up

Any state you send to a function must be retrieved. There are examples in:

- [`state::Header`]
- [`state::Bullet`]
- [`state::SubBullet`]
- [`state::Stream`]
- [`state::Background`]

In general, we recommend breaking business logic down into functions. Rather than threading the logging state throughout every possible function, rely on functions to bubble up information to log.

Here's an example of logging by passing the output state into functions:

```rust
// Example of logging by passing state into a function, requires a large signature
// ❌😾❌

use bullet_stream::{
    state::{Bullet, SubBullet},
    Print, style
};
use std::io::Stdout;
use std::path::Path;

/// Large function signature, it works but might not always be needed
fn install_ruby(
    mut output: Print<Bullet<Stdout>>,
    path: &Path,
) -> Result<(Print<SubBullet<Stdout>>, String), std::io::Error>
{
    let version = std::fs::read_to_string(path)?
        .trim()
        .to_owned();

    let timer = output.bullet(format!("Ruby version {}", style::value(&version)))
        .start_timer("Installing");

    // ...
    Ok((timer.done(), version))
}

let mut output = Print::new(std::io::stdout()).h2("Example Buildpack");

let (bullet, version) = install_ruby(output, &Path::new("/dev/null"))
    .unwrap();
output = bullet.done();
```

In the above example, the `install_ruby` function both performs logic and logs information, resulting in a very large function signature. If the function also needed to return information, it would need to use a tuple to return both the logger and the information.

Here's an alternative where the all information needed to log is brought up to the same top-level, and the functions don't need to have massive type signatures:

```rust
// Example of bubbling up information to the logger
// ✅😸✅
use bullet_stream::{Print, style};

/// Smaller signature
fn install_ruby_version(version: impl AsRef<str>) -> Result<(), std::io::Error> {
    // ...
    Ok(())
}

let mut output = Print::new(std::io::stdout()).h2("Example Buildpack");

// Bubble up data
let version = std::fs::read_to_string(std::path::Path::new("/dev/null"))
    .unwrap()
    .trim()
    .to_owned();

// Output data
let timer = output.bullet(format!("Ruby version {}", style::value(&version)))
    .start_timer("Installing");

// Call logic
install_ruby_version(&version).unwrap();

output = timer.done()
    .done();
```

It's not **bad** if you want to pass your output around to functions, but it is cumbersome.

### Async support

> Status: Experimental/WIP; if you've got a better suggestion, let us know.

Because the logger is stateful, consuming logging from within an async or parallel execution context is tricky. We recommend using the same pattern as above to bubble up information that can be logged between synchronization points in the program.

For example, here's some hand-rolled output from code that uses async:

```text
## Distribution Info

- Name: ubuntu
- Version: 22.04
- Codename: jammy
- Architecture: amd64

## Creating package index

  [GET] http://archive.ubuntu.com/ubuntu/dists/jammy-updates/InRelease
  [CACHED] http://archive.ubuntu.com/ubuntu/dists/jammy/InRelease
  [GET] http://archive.ubuntu.com/ubuntu/dists/jammy-security/InRelease
  [CACHED] http://archive.ubuntu.com/ubuntu/dists/jammy/universe/binary-amd64/by-hash/SHA256/9939f6554c5cbea6607e3886634d7e393d8b0364ae0a43c2549d7191840c66c1
  [CACHED] http://archive.ubuntu.com/ubuntu/dists/jammy/main/binary-amd64/by-hash/SHA256/712ee19b50fa5a5963b82b8dd00438f59ef1f088db8e3e042f4306d2b7c89c69
  [GET] http://archive.ubuntu.com/ubuntu/dists/jammy-updates/main/binary-amd64/by-hash/SHA256/9be23783bb2295aedcb02760ffaa8980c58573d0318ec67f2f409b8f3d2f27bb
  [GET] http://archive.ubuntu.com/ubuntu/dists/jammy-updates/universe/binary-amd64/by-hash/SHA256/c6a66ee7fb32ca0f0662b0b1b2a2f58ab18a10b749a8dcc61a9fc7d0fde17754
  [CACHED] http://archive.ubuntu.com/ubuntu/dists/jammy-security/universe/binary-amd64/by-hash/SHA256/86e543e7b5cccc2537a4f6451f7f9c0cd459803cf4403beca71a459848dd9a0f
  [GET] http://archive.ubuntu.com/ubuntu/dists/jammy-security/main/binary-amd64/by-hash/SHA256/9943ee3b3104b37d0ee219fae65f261d0c61c96bb3978fe92e6573c9dcd88862
```

In this example, the get and cached lines are logged within an async context. Here's an example of a refactor that could use the bullet stream library:

```text
# Heroku Debian Packages Buildpack (v0.0.1)

- Package index sources
  - `http://archive.ubuntu.com/ubuntu/dists/jammy/InRelease`
  - `http://archive.ubuntu.com/ubuntu/dists/jammy-updates/InRelease`
  - `http://archive.ubuntu.com/ubuntu/dists/jammy-security/InRelease`
  - Downloading ...................................... (Done 35s)
- Downloaded indexes
  - `http://archive.ubuntu.com/ubuntu/dists/jammy/universe/binary-amd64/by-hash/SHA256/9939f6554c5cbea6607e3886634d7e393d8b0364ae0a43c2549d7191840c66c1`
  - `http://archive.ubuntu.com/ubuntu/dists/jammy/main/binary-amd64/by-hash/SHA256/712ee19b50fa5a5963b82b8dd00438f59ef1f088db8e3e042f4306d2b7c89c69`
  - `http://archive.ubuntu.com/ubuntu/dists/jammy-updates/main/binary-amd64/by-hash/SHA256/9be23783bb2295aedcb02760ffaa8980c58573d0318ec67f2f409b8f3d2f27bb`
  - `http://archive.ubuntu.com/ubuntu/dists/jammy-updates/universe/binary-amd64/by-hash/SHA256/c6a66ee7fb32ca0f0662b0b1b2a2f58ab18a10b749a8dcc61a9fc7d0fde17754`
  - `http://archive.ubuntu.com/ubuntu/dists/jammy-security/universe/binary-amd64/by-hash/SHA256/86e543e7b5cccc2537a4f6451f7f9c0cd459803cf4403beca71a459848dd9a0f`
  - `http://archive.ubuntu.com/ubuntu/dists/jammy-security/main/binary-amd64/by-hash/SHA2569943ee3b3104b37d0ee219fae65f261d0c61c96bb3978fe92e6573c9dcd88862`
  - Processing ..... (Done 4s)
```

In this example, the output states what it's going to do by listing the package source locations. After it downloads them, there's a synchronization point before it has enough information to output which archives were downloaded and their SHAs and begin processing them (again asynchronously).

Alternatively, you could wrap a `SubBullet` state struct in an Arc and try passing it around, or use `bullet_stream` for top-level printing. Printing inside an async context could happen via `println`.

### Generics

Bullet stream works with anything that is `Write + Send + Sync + 'static,` but most people will use `std::io::Stdout` or `std::io::Stderr`. If you know a specific type you want to output to, you can simplify your method definitions.

For example:

```rust
use bullet_stream::{
    state::{Bullet, SubBullet},
    Print,
};
use std::path::{Path, PathBuf};
use std::io::Stdout;

fn install_ruby(
    mut output: Print<Bullet<Stdout>>,
    path: &Path,
) -> Result<Print<SubBullet<Stdout>>, std::io::Error>
{
    todo!();
}
```

If that's still too much typing for you, you can simplify more with type aliases:

```rust
use bullet_stream::{Print, state};
use std::io::Stdout;
use std::path::Path;

pub(crate) type Header = Print<state::Header<Stdout>>;
pub(crate) type Bullet = Print<state::Bullet<Stdout>>;
pub(crate) type SubBullet = Print<state::SubBullet<Stdout>>;

fn install_ruby(
    mut output: Bullet,
    path: &Path,
) -> Result<SubBullet, std::io::Error>
{
    todo!();
}
```
