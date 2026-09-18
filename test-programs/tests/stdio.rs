use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};

fn run(component: &str, p3: bool) -> Result<()> {
    let mut command = Command::new("wasmtime");
    command.arg("run");
    if p3 {
        command.arg("-Sp3");
    }

    let mut child = command
        .arg(component)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawning stdio example")?;

    let mut stdin = child.stdin.take().context("stdin is piped")?;
    let mut stdout = BufReader::new(child.stdout.take().context("stdout is piped")?);
    let mut stderr = BufReader::new(child.stderr.take().context("stderr is piped")?);

    stdin
        .write_all(b"hello from stdin\n")
        .context("writing example stdin")?;
    stdin.flush().context("flushing example stdin")?;

    let mut line = String::new();
    stdout
        .read_line(&mut line)
        .context("reading echoed stdout line")?;
    assert_eq!(line, "stdout: hello from stdin\n");

    line.clear();
    stderr
        .read_line(&mut line)
        .context("reading echoed stderr line")?;
    assert_eq!(line, "stderr: hello from stdin\n");

    // Closing stdin ends the guest's read loop. Closing stdout makes its next
    // write fail, which the guest reports through the still-open stderr pipe.
    drop(stdin);
    drop(stdout);

    let mut errors = String::new();
    stderr
        .read_to_string(&mut errors)
        .context("reading errors from stderr")?;

    let status = child.wait().context("waiting for stdio example")?;
    assert!(status.success(), "stdio example failed: {status}");
    assert_eq!(
        errors,
        "stdin error: UnexpectedEof\nstdout error: ConnectionReset\n"
    );

    Ok(())
}

#[test_log::test]
fn stdio_p2() -> Result<()> {
    run(test_programs::STDIO, false)
}

#[test_log::test]
fn stdio_p3() -> Result<()> {
    // TODO: Remove this nightly check once wasm32-wasip3 is available on stable.
    if test_programs::NIGHTLY_TOOLCHAIN {
        run(test_programs::STDIO_P3, true)?;
    }

    Ok(())
}
