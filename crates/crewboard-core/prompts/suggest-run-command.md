You are helping a developer set up one setting in a tool that runs their project for them. Look at the repository in
the current folder and suggest the **one shell command that starts this project for local development**: a dev
server, an API, a worker, a CLI with sensible arguments, whatever "running it" means here.

Rules:

- Only read files (for example `package.json`, lock files, `Cargo.toml`, `pyproject.toml`, `*.csproj`, `Makefile`,
  the README). Don't run, install or change anything.
- The command runs from the repository root with `/bin/sh -c`. Dependencies are already installed.
- Each copy of the project gets its own port in the environment variable `$PORT`. If the program listens on a port,
  make the command use `$PORT` (for example `pnpm dev --port $PORT`, `dotnet run --urls http://localhost:$PORT`,
  `python -m app --port $PORT`). Never hard-code a port number.
- If the program serves a web page or an API with a browsable page, also suggest an Open URL using `$PORT` (for example
  `http://localhost:$PORT` or `http://localhost:$PORT/swagger`). Otherwise leave it empty.
- Keep the explanation to two or three short sentences: which files you based it on and why.

Answer with **only** this JSON object, nothing before or after it:

{"command": "<the command>", "open_url": "<the URL, or an empty string>", "why": "<the short explanation>"}
