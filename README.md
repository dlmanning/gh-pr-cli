# gh-pr-cli - A silly tool to tell you which PR's you need to review.

## Overview

This is a CLI application that fetches pull requests (PRs) concerning a specific user from a GitHub repository. It considers PRs as concerning a user if they:

- are requested for review
- are mentioned in the PR's body
- are assigned to the user's team
- are assigned to the user directly

The application also fetches comments mentioning the user and displays the PRs with those comments. It retrieves the total number of additions and deletions for each PR and generates a formatted table output.

## Requirements

- Rust 1.55.0 or higher

## Installation

Clone the repository and run `cargo build --release` to compile the application.

## Usage

1. Set up a `.env` file in the project's root directory with the following format:

   ```
   GH_TOKEN=your_personal_github_token
   ```

   Replace `your_personal_github_token` with your GitHub personal access token.

   Note: you can also set `GH_TOKEN` in your environment, or pass it to the command like so:

   ```
   GH_TOKEN=your_personal_github_token cargo run -- [FLAGS] [OPTIONS]
   ```

2. Run the application using the following command:
   ```
   cargo run -- [FLAGS] [OPTIONS]
   ```
   Replace `[FLAGS]` and `[OPTIONS]` with the appropriate flags and options for your use case.

## Flags

- `--comments`: Include PRs that mention the user in their comments.

## Options

- `--last N`: Display the last N PRs. Default is 50.
- `--repo OWNER/REPOSITORY`: Specify the repository to fetch PRs from. If only the repository name is provided, the application will use the current user's login as the owner. Default is the current user's repositories.
- `--state open|closed|all`: Fetch PRs with the specified state. Default is "open".

## Output

The application generates a formatted table displaying the following information for each PR:

Like this:

```
+------+------------------+------------------------------------+----------------------------------------+-----------+-----+
| PR # |  Last Updated    |               Title                |                   URL                  |  Author   | +/- |
+------+------------------+------------------------------------+----------------------------------------+-----------+-----+
| 42   | 2023-04-20 18:30 | Update README.md                   | https://github.com/user/repo/pull/42   | @user1    |+5/-3|
| 39   | 2023-04-18 15:45 | Fix issue #123                     | https://github.com/user/repo/pull/39   | @user2    |+3/-1|
| 36   | 2023-04-17 12:15 | Add new feature                    | https://github.com/user/repo/pull/36   | @user3    |+8/-2|
+------+------------------+------------------------------------+----------------------------------------+-----------+-----+
```
