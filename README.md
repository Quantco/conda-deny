<picture>
  <source media="(prefers-color-scheme: dark)" srcset="./.github/assets/conda-deny-banner-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="./.github/assets/conda-deny-banner-light.png">
  <img alt="conda-deny" src="./.github/assets/conda-deny-banner-light.png">
</picture>

<div align="center">

[![License][license-badge]](LICENSE)
[![CI Status][ci-badge]][ci]
[![Binary Build][binary-build-badge]][binary-build]
[![Conda Platform][conda-badge]][conda-url]
[![Codecov][codecov]][codecov-url]

[license-badge]: https://img.shields.io/github/license/quantco/conda-deny?style=flat-square

[ci-badge]: https://img.shields.io/github/actions/workflow/status/quantco/conda-deny/ci.yml?branch=main&style=flat-square&label=CI
[ci]: https://github.com/quantco/conda-deny/actions/workflows/ci.yml

[binary-build-badge]: https://img.shields.io/github/actions/workflow/status/quantco/conda-deny/build.yml?branch=main&style=flat-square&label=Binary%20Build
[binary-build]: https://github.com/quantco/conda-deny/actions/workflows/build.yml

[conda-badge]: https://img.shields.io/conda/vn/conda-forge/conda-deny?style=flat-square
[conda-url]: https://prefix.dev/channels/conda-forge/packages/conda-deny

[codecov]: https://img.shields.io/codecov/c/github/quantco/conda-deny/main?style=flat-square
[codecov-url]: https://codecov.io/gh/Quantco/conda-deny

</div>

## 🗂 Table of Contents

- [Introduction](#-introduction)
- [Installation](#-installation)
- [Usage](#-usage)

## 📖 Introduction

conda-deny is a CLI tool for checking software environment dependencies for license compliance.
Compliance is checked with regard to a whitelist of licenses provided by the user. 

## 💿 Installation

You can install `conda-deny` using `pixi`:

```bash
pixi global install conda-deny
```

Or by downloading our pre-built binaries from the [releases page](https://github.com/quantco/conda-deny/releases).

## 🎯 Usage

![conda-deny demo](.github/assets/demo/demo-light.gif#gh-light-mode-only)
![conda-deny demo](.github/assets/demo/demo-dark.gif#gh-dark-mode-only)

`conda-deny` can be configured in your `pixi.toml` or `pyproject.toml` (`pixi.toml` is preferred).
The tool expects a configuration in the following format:

```toml
[tool.conda-deny]
#--------------------------------------------------------
# General setup options:
#--------------------------------------------------------
license-whitelist = "https://raw.githubusercontent.com/QuantCo/conda-deny/main/tests/test_remote_base_configs/conda-deny-license_whitelist.toml" # or ["license_whitelist.toml", "other_license_whitelist.toml"]
platform = "linux-64" # or ["linux-64", "osx-arm64"]
environment = "default" # or ["default", "py39", "py310", "prod"]
lockfile = "environment/pixi.lock" # or ["environment1/pixi.lock", "environment2/pixi.lock"]

#--------------------------------------------------------
# License whitelist directly in configuration file:
#--------------------------------------------------------
safe-licenses = ["MIT", "BSD-3-Clause"]
ignore-packages = [
    { package = "make", version = "0.1.0" },
]
```

After installing `conda-deny`, you can run `conda-deny check` in your project.
This then checks `pixi.lock` to determine the packages (and their versions) used in your project.

### ✨ Output Formats

`conda-deny` supports different output formats via the `--output` (or `-o`) flag.
Output formatting works for both, the `list` and the `check` command.
To get an overview of the different format options, try:

```bash
conda-deny check --help
# Or:
conda-deny list --help
```
