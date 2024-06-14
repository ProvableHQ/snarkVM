<p align="center">
    <img alt="snarkVM" width="1412" src="https://repository-images.githubusercontent.com/247216177/29db4fb6-1330-4a3d-a27f-f07b7391592d">
</p>

<p align="center">
    <a href="https://circleci.com/gh/AleoNet/snarkVM"><img src="https://dl.circleci.com/status-badge/img/gh/AleoNet/snarkVM/tree/mainnet.svg?style=svg"></a>
    <a href="https://codecov.io/gh/AleoNet/snarkVM"><img src="https://codecov.io/gh/AleoNet/snarkVM/branch/master/graph/badge.svg?token=cck8tS9HpO"/></a>
    <a href="https://discord.gg/aleo"><img src="https://img.shields.io/discord/700454073459015690?logo=discord"/></a>
    <a href="https://twitter.com/AleoHQ"><img src="https://img.shields.io/twitter/follow/AleoHQ?style=social"/></a>
    <a href="https://github.com/AleoNet/snarkVM"><img src="https://img.shields.io/badge/contributors-41-ee8449"/></a>
</p>

## Table of Contents

* [1. Overview](#1-overview)
* [2. Build Guide](#2-build-guide)
* [3. Usage Guide](#3-usage-guide)
* [4. Contributors](#4-contributors)
* [5. License](#5-license)

## 1. Overview

|       Package       | Crate.io                                                                         |        `std`       |       `wasm`       |
|:-------------------:|----------------------------------------------------------------------------------|:------------------:|:------------------:|
|       snarkvm       | ![crates.io](https://img.shields.io/crates/v/snarkvm.svg?color=neon)             | :white_check_mark: | :white_check_mark: |
| snarkvm-algorithms  | ![crates.io](https://img.shields.io/crates/v/snarkvm-algorithms.svg?color=neon)  | :white_check_mark: | :white_check_mark: |
|   snarkvm-circuit   | ![crates.io](https://img.shields.io/crates/v/snarkvm-circuit.svg?color=neon)     | :white_check_mark: | :white_check_mark: |
|   snarkvm-console   | ![crates.io](https://img.shields.io/crates/v/snarkvm-console.svg?color=neon)     | :white_check_mark: | :white_check_mark: |
|   snarkvm-curves    | ![crates.io](https://img.shields.io/crates/v/snarkvm-curves.svg?color=neon)      | :white_check_mark: | :white_check_mark: |
|   snarkvm-fields    | ![crates.io](https://img.shields.io/crates/v/snarkvm-fields.svg?color=neon)      | :white_check_mark: | :white_check_mark: |
|   snarkvm-ledger    | ![crates.io](https://img.shields.io/crates/v/snarkvm-ledger.svg?color=neon)      | :white_check_mark: | :white_check_mark: |
| snarkvm-parameters  | ![crates.io](https://img.shields.io/crates/v/snarkvm-parameters.svg?color=neon)  | :white_check_mark: | :white_check_mark: |
| snarkvm-synthesizer | ![crates.io](https://img.shields.io/crates/v/snarkvm-synthesizer.svg?color=neon) | :white_check_mark: | :white_check_mark: |
|  snarkvm-utilities  | ![crates.io](https://img.shields.io/crates/v/snarkvm-utilities.svg?color=neon)   | :white_check_mark: | :white_check_mark: |
|    snarkvm-wasm     | ![crates.io](https://img.shields.io/crates/v/snarkvm-wasm.svg?color=neon)        | :white_check_mark: | :white_check_mark: |

For more information, visit [Welcome to Aleo](https://github.com/AleoNet/welcome) to get started.

## 2. Build Guide

### 2.1 Install Rust

We recommend installing Rust using [rustup](https://www.rustup.rs/). You can install `rustup` as follows:

- macOS or Linux:
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

- Windows (64-bit):

  Download the [Windows 64-bit executable](https://win.rustup.rs/x86_64) or
  [Windows 32-bit executable](https://win.rustup.rs/i686) and follow the on-screen instructions.

### 2.2.1 Build from Crates.io

We recommend installing `snarkvm` this way. In your terminal, run:

```bash
cargo install snarkvm
```

Now to use `snarkvm`, in your terminal, run:
```bash
snarkvm
```

### 2.2.2 Build from Source Code

Alternatively, you can install `snarkvm` by building from the source code as follows:

```bash
# Download the source code
git clone https://github.com/AleoNet/snarkvm && cd snarkvm

# Install snarkVM
$ cargo install --path .
```

Now to use `snarkvm`, in your terminal, run:
```bash
snarkvm
```

## 3. Usage Guide

## 4. Contributors

Thank you for helping make snarkVM better!  
[🧐 What do the emojis mean?](https://allcontributors.org/docs/en/emoji-key)

<!-- ALL-CONTRIBUTORS-LIST:START - Do not remove or modify this section -->
<!-- prettier-ignore-start -->
<!-- markdownlint-disable -->
<table>
  <tbody>
    <tr>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/howardwu"><img src="https://avatars.githubusercontent.com/u/9260812?v=4?s=100" width="100px;" alt="Howard Wu"/><br /><sub><b>Howard Wu</b></sub></a><br /><a href="https://github.com/AleoNet/snarkVM/commits?author=howardwu" title="Code">💻</a> <a href="#maintenance-howardwu" title="Maintenance">🚧</a> <a href="#ideas-howardwu" title="Ideas, Planning, & Feedback">🤔</a> <a href="https://github.com/AleoNet/snarkvm/pulls?q=is%3Apr+reviewed-by%3Ahowardwu" title="Reviewed Pull Requests">👀</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/raychu86"><img src="https://avatars.githubusercontent.com/u/14917648?v=4?s=100" width="100px;" alt="Raymond Chu"/><br /><sub><b>Raymond Chu</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=raychu86" title="Code">💻</a> <a href="#maintenance-raychu86" title="Maintenance">🚧</a> <a href="#ideas-raychu86" title="Ideas, Planning, & Feedback">🤔</a> <a href="https://github.com/AleoNet/snarkvm/pulls?q=is%3Apr+reviewed-by%3Araychu86" title="Reviewed Pull Requests">👀</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/d0cd"><img src="https://avatars.githubusercontent.com/u/23022326?v=4?s=100" width="100px;" alt="d0cd"/><br /><sub><b>d0cd</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=d0cd" title="Code">💻</a> <a href="#maintenance-d0cd" title="Maintenance">🚧</a> <a href="#ideas-d0cd" title="Ideas, Planning, & Feedback">🤔</a> <a href="https://github.com/AleoNet/snarkvm/pulls?q=is%3Apr+reviewed-by%3Ad0cd" title="Reviewed Pull Requests">👀</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/Pratyush"><img src="https://avatars.githubusercontent.com/u/3220730?v=4?s=100" width="100px;" alt="Pratyush Mishra"/><br /><sub><b>Pratyush Mishra</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=Pratyush" title="Code">💻</a> <a href="#maintenance-Pratyush" title="Maintenance">🚧</a> <a href="#ideas-Pratyush" title="Ideas, Planning, & Feedback">🤔</a> <a href="https://github.com/AleoNet/snarkvm/pulls?q=is%3Apr+reviewed-by%3APratyush" title="Reviewed Pull Requests">👀</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://victorsintnicolaas.com/"><img src="https://avatars.githubusercontent.com/u/24724627?v=4?s=100" width="100px;" alt="vicsn"/><br /><sub><b>vicsn</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=vicsn" title="Code">💻</a> <a href="#maintenance-vicsn" title="Maintenance">🚧</a> <a href="https://github.com/AleoNet/snarkvm/commits?author=vicsn" title="Documentation">📖</a> <a href="https://github.com/AleoNet/snarkvm/pulls?q=is%3Apr+reviewed-by%3Avicsn" title="Reviewed Pull Requests">👀</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/ljedrz"><img src="https://avatars.githubusercontent.com/u/3750347?v=4?s=100" width="100px;" alt="ljedrz"/><br /><sub><b>ljedrz</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=ljedrz" title="Code">💻</a> <a href="#tool-ljedrz" title="Tools">🔧</a> <a href="https://github.com/AleoNet/snarkvm/pulls?q=is%3Apr+reviewed-by%3Aljedrz" title="Reviewed Pull Requests">👀</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/iamalwaysuncomfortable"><img src="https://avatars.githubusercontent.com/u/26438809?v=4?s=100" width="100px;" alt="Mike Turner"/><br /><sub><b>Mike Turner</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=iamalwaysuncomfortable" title="Code">💻</a> <a href="https://github.com/AleoNet/snarkvm/commits?author=iamalwaysuncomfortable" title="Documentation">📖</a> <a href="https://github.com/AleoNet/snarkvm/pulls?q=is%3Apr+reviewed-by%3Aiamalwaysuncomfortable" title="Reviewed Pull Requests">👀</a></td>
    </tr>
    <tr>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/collinc97"><img src="https://avatars.githubusercontent.com/u/16715212?v=4?s=100" width="100px;" alt="Collin Chin"/><br /><sub><b>Collin Chin</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=collinc97" title="Code">💻</a> <a href="https://github.com/AleoNet/snarkvm/commits?author=collinc97" title="Documentation">📖</a> <a href="https://github.com/AleoNet/snarkvm/pulls?q=is%3Apr+reviewed-by%3Acollinc97" title="Reviewed Pull Requests">👀</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://alessandrocoglio.info"><img src="https://avatars.githubusercontent.com/u/2409151?v=4?s=100" width="100px;" alt="Alessandro Coglio"/><br /><sub><b>Alessandro Coglio</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=acoglio" title="Code">💻</a> <a href="https://github.com/AleoNet/snarkvm/commits?author=acoglio" title="Documentation">📖</a> <a href="https://github.com/AleoNet/snarkvm/commits?author=acoglio" title="Tests">⚠️</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/niklaslong"><img src="https://avatars.githubusercontent.com/u/13221615?v=4?s=100" width="100px;" alt="Niklas Long"/><br /><sub><b>Niklas Long</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=niklaslong" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/jules"><img src="https://avatars.githubusercontent.com/u/30194392?v=4?s=100" width="100px;" alt="jules"/><br /><sub><b>jules</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=jules" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/amousa11"><img src="https://avatars.githubusercontent.com/u/12452142?v=4?s=100" width="100px;" alt="Ali Mousa"/><br /><sub><b>Ali Mousa</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=amousa11" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://www.chenweikeng.com/"><img src="https://avatars.githubusercontent.com/u/14937807?v=4?s=100" width="100px;" alt="Weikeng Chen"/><br /><sub><b>Weikeng Chen</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=weikengchen" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/evan-schott"><img src="https://avatars.githubusercontent.com/u/53463459?v=4?s=100" width="100px;" alt="Evan Schott"/><br /><sub><b>Evan Schott</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=evan-schott" title="Code">💻</a></td>
    </tr>
    <tr>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/Protryon"><img src="https://avatars.githubusercontent.com/u/8600837?v=4?s=100" width="100px;" alt="Max Bruce"/><br /><sub><b>Max Bruce</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=Protryon" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/zhiqiangxu"><img src="https://avatars.githubusercontent.com/u/1265027?v=4?s=100" width="100px;" alt="zhiqiangxu"/><br /><sub><b>zhiqiangxu</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=zhiqiangxu" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/jrchatruc"><img src="https://avatars.githubusercontent.com/u/49622509?v=4?s=100" width="100px;" alt="Javier Rodríguez Chatruc"/><br /><sub><b>Javier Rodríguez Chatruc</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=jrchatruc" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/emmorais"><img src="https://avatars.githubusercontent.com/u/13047772?v=4?s=100" width="100px;" alt="Eduardo Morais"/><br /><sub><b>Eduardo Morais</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=emmorais" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/zvolin"><img src="https://avatars.githubusercontent.com/u/34972409?v=4?s=100" width="100px;" alt="Maciej Zwoliński"/><br /><sub><b>Maciej Zwoliński</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=zvolin" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/ilitteri"><img src="https://avatars.githubusercontent.com/u/67517699?v=4?s=100" width="100px;" alt="Ivan Litteri"/><br /><sub><b>Ivan Litteri</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=ilitteri" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/FranFiuba"><img src="https://avatars.githubusercontent.com/u/5733366?v=4?s=100" width="100px;" alt="Francisco Strambini"/><br /><sub><b>Francisco Strambini</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=FranFiuba" title="Code">💻</a></td>
    </tr>
    <tr>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/HarukaMa"><img src="https://avatars.githubusercontent.com/u/861659?v=4?s=100" width="100px;" alt="Haruka"/><br /><sub><b>Haruka</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/issues?q=author%3AHarukaMa" title="Bug reports">🐛</a> <a href="https://github.com/AleoNet/snarkvm/commits?author=HarukaMa" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/StarLI-Trapdoor"><img src="https://avatars.githubusercontent.com/u/55707687?v=4?s=100" width="100px;" alt="StarLI-Trapdoor"/><br /><sub><b>StarLI-Trapdoor</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=StarLI-Trapdoor" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/vvp"><img src="https://avatars.githubusercontent.com/u/700877?v=4?s=100" width="100px;" alt="Vesa-Ville"/><br /><sub><b>Vesa-Ville</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=vvp" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/joske"><img src="https://avatars.githubusercontent.com/u/532423?v=4?s=100" width="100px;" alt="Jos Dehaes"/><br /><sub><b>Jos Dehaes</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=joske" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/apruden2008"><img src="https://avatars.githubusercontent.com/u/39969542?v=4?s=100" width="100px;" alt="apruden2008"/><br /><sub><b>apruden2008</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=apruden2008" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/evanmarshall"><img src="https://avatars.githubusercontent.com/u/1102811?v=4?s=100" width="100px;" alt="Evan Marshall"/><br /><sub><b>Evan Marshall</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/issues?q=author%3Aevanmarshall" title="Bug reports">🐛</a> <a href="https://github.com/AleoNet/snarkvm/commits?author=evanmarshall" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/psivesely"><img src="https://avatars.githubusercontent.com/u/3538418?v=4?s=100" width="100px;" alt="Psi Vesely"/><br /><sub><b>Psi Vesely</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=psivesely" title="Code">💻</a></td>
    </tr>
    <tr>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/swift-mx"><img src="https://avatars.githubusercontent.com/u/80231732?v=4?s=100" width="100px;" alt="swift-mx"/><br /><sub><b>swift-mx</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=swift-mx" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://www.linkedin.com/in/ignacio-avecilla-39386a191/"><img src="https://avatars.githubusercontent.com/u/63374472?v=4?s=100" width="100px;" alt="Nacho Avecilla"/><br /><sub><b>Nacho Avecilla</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=IAvecilla" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/qy3u"><img src="https://avatars.githubusercontent.com/u/65523321?v=4?s=100" width="100px;" alt="qy3u"/><br /><sub><b>qy3u</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=qy3u" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/happysalada"><img src="https://avatars.githubusercontent.com/u/5317234?v=4?s=100" width="100px;" alt="Yt"/><br /><sub><b>Yt</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=happysalada" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/unordered-set"><img src="https://avatars.githubusercontent.com/u/78592281?v=4?s=100" width="100px;" alt="Kostyan"/><br /><sub><b>Kostyan</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=unordered-set" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/stanlagermin"><img src="https://avatars.githubusercontent.com/u/40028493?v=4?s=100" width="100px;" alt="stanlagermin"/><br /><sub><b>stanlagermin</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=stanlagermin" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/sukey2008"><img src="https://avatars.githubusercontent.com/u/35202440?v=4?s=100" width="100px;" alt="Sukey"/><br /><sub><b>Sukey</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=sukey2008" title="Code">💻</a></td>
    </tr>
    <tr>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/AlexZhao6666"><img src="https://avatars.githubusercontent.com/u/136443781?v=4?s=100" width="100px;" alt="Alex Zhao"/><br /><sub><b>Alex Zhao</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=AlexZhao6666" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/ghostant-1017"><img src="https://avatars.githubusercontent.com/u/53888545?v=4?s=100" width="100px;" alt="ghost ant"/><br /><sub><b>ghost ant</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=ghostant-1017" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/psivesely"><img src="https://avatars.githubusercontent.com/u/3538418?v=4?s=100" width="100px;" alt="Psi Vesely"/><br /><sub><b>Psi Vesely</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=psivesely" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/features/security"><img src="https://avatars.githubusercontent.com/u/27347476?v=4?s=100" width="100px;" alt="Dependabot"/><br /><sub><b>Dependabot</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=dependabot" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://github.com/apps/dependabot-preview"><img src="https://avatars.githubusercontent.com/u/27347476?v=4?s=100" width="100px;" alt="Dependabot Preview"/><br /><sub><b>Dependabot Preview</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=dependabot-preview" title="Code">💻</a></td>
      <td align="center" valign="top" width="14.28%"><a href="https://allcontributors.org/"><img src="https://avatars.githubusercontent.com/u/46410174?v=4?s=100" width="100px;" alt="All Contributors"/><br /><sub><b>All Contributors</b></sub></a><br /><a href="https://github.com/AleoNet/snarkvm/commits?author=all-contributors" title="Documentation">📖</a></td>
    </tr>
  </tbody>
  <tfoot>
    <tr>
      <td align="center" size="13px" colspan="7">
        <img src="https://raw.githubusercontent.com/all-contributors/all-contributors-cli/1b8533af435da9854653492b1327a23a4dbd0a10/assets/logo-small.svg">
          <a href="https://all-contributors.js.org/docs/en/bot/usage">Add your contributions</a>
        </img>
      </td>
    </tr>
  </tfoot>
</table>

<!-- markdownlint-restore -->
<!-- prettier-ignore-end -->

<!-- ALL-CONTRIBUTORS-LIST:END -->

This project follows the [all-contributors](https://github.com/all-contributors/all-contributors) specification. Contributions of any kind welcome!

## 5. License

[![License: GPL v3](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](./LICENSE.md)
