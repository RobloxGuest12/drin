# Contributing to DRin

Thank you for your interest in contributing to DRin!

DRin is an open-source package manager project designed specifically for Ubuntu. Contributions from developers, testers, and the community are welcome.

This document explains how to contribute to the project.

## 1\. Before Contributing

Before submitting a contribution:

* Check existing issues and pull requests.
* Make sure your proposed change fits DRin's goals.
* Avoid submitting duplicate bug reports or features.
* Be respectful toward other contributors.

## 2\. Development Requirements

To work on DRin, you will need:

* Ubuntu Linux
* Git
* Rust
* Cargo
* Build tools

Install the required system packages:

```bash
sudo apt update
sudo apt install git build-essential pkg-config curl
```

Install Rust using rustup:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Verify your installation:

```bash
rustc --version
cargo --version
```

## 3\. Getting the Source Code

Clone the repository:

```bash
git clone https://github.com/YOUR\_USERNAME/drin.git
cd drin
```

Replace `YOUR\_USERNAME` with the repository owner's GitHub username.

## 4\. Building DRin

Compile the project:

```bash
cargo build --release
```

The executable will be generated at:

```text
target/release/drin
```

For development builds:

```bash
cargo build
```

## 5\. Testing Changes

Before submitting your changes, run:

```bash
cargo check
cargo test
cargo build --release
```

Test relevant DRin commands in an Ubuntu virtual machine.

Avoid testing experimental package-management changes on your primary operating system.

## 6\. Reporting Bugs

When reporting a bug, include:

* DRin version
* Ubuntu version
* Rust version, if relevant
* Command that caused the issue
* Expected behavior
* Actual behavior
* Relevant terminal output or error messages

Please do not include passwords, private keys, access tokens, or other sensitive information in bug reports.

## 7\. Suggesting Features

Feature suggestions are welcome!

When proposing a feature, explain:

* What the feature should do.
* Why it would be useful.
* How it could improve DRin.
* Any potential compatibility concerns.

Large changes should be discussed before implementation.

## 8\. Submitting Pull Requests

To contribute code:

1. Fork the repository.
2. Clone your fork.
3. Create a new branch.
4. Make your changes.
5. Test your changes.
6. Commit your work.
7. Push your branch.
8. Open a pull request.

Example:

```bash
git checkout -b feature/my-feature

# Make your changes

cargo check
cargo test
cargo build --release

git add .
git commit -m "Add my feature"
git push origin feature/my-feature
```

Then open a pull request on GitHub.

## 9\. Code Guidelines

Please follow these guidelines:

* Write readable and maintainable Rust code.
* Use meaningful variable and function names.
* Handle errors instead of silently ignoring them.
* Avoid unnecessary dependencies.
* Keep compatibility with supported Ubuntu releases in mind.
* Document important changes.
* Do not introduce unsafe system operations without careful justification.

## 10\. Security

DRin interacts with the system package manager and may perform privileged operations.

Contributions must:

* Respect Ubuntu's permission system.
* Avoid executing untrusted commands.
* Verify downloaded package data where applicable.
* Handle administrator privileges carefully.
* Avoid exposing user credentials.

Please report security vulnerabilities privately to the repository maintainer rather than publishing exploitable details in a public issue.

## 11\. Project Goals

DRin aims to become a lightweight, reliable, and efficient package manager for Ubuntu.

Potential areas for future development include:

* Faster package operations.
* Improved download management.
* Better progress reporting.
* Package metadata caching.
* Improved dependency handling.
* Repository management.
* Better error messages.
* Automated testing.

## 12\. License

By contributing to DRin, you agree that your contributions will be distributed under the project's **GPL-3.0-only** license.

See the `LICENSE` file for the complete license text.

\---

Thank you for helping improve DRin!

**DRin — Durian Package Manager**

