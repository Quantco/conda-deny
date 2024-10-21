<picture>
  <source media="(prefers-color-scheme: dark)" srcset="./.github/assets/conda-deny-banner-dark.png">
  <source media="(prefers-color-scheme: light)" srcset="./.github/assets/conda-deny-banner-light.png">
  <img alt="conda-deny" src="./.github/assets/conda-deny-banner-light.png">
</picture>

<div align="center">

![Cargo Build & Test](https://github.com/quantco/conda-deny/actions/workflows/ci.yml/badge.svg)
![Binary Build](https://github.com/quantco/conda-deny/actions/workflows/build.yml/badge.svg)
<!-- ![Conda Package](https://github.com/quantco/conda-deny/actions/workflows/package.yml/badge.svg) -->
[![codecov](https://codecov.io/gh/Quantco/conda-deny/graph/badge.svg?token=uixrZFJln7)](https://codecov.io/gh/Quantco/conda-deny)

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
license-whitelist = "https://raw.githubusercontent.com/PaulKMueller/conda-deny-test/refs/heads/main/conda-deny-license_whitelist.toml" # or ["license_whitelist.toml", "other_license_whitelist.toml"]
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
