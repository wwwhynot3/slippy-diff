# Changelog

## [0.3.1](https://github.com/wwwhynot3/slippy-diff/compare/slippy-v0.3.0...slippy-v0.3.1) (2026-06-30)


### Bug Fixes

* install linux desktop icons ([ae3da2c](https://github.com/wwwhynot3/slippy-diff/commit/ae3da2c4d06c4ea46f04fcb3fc3c09b0d9542b7b))

## [0.3.0](https://github.com/wwwhynot3/slippy-diff/compare/slippy-v0.2.3...slippy-v0.3.0) (2026-06-30)


### Features

* add desktop packaging installers ([9a446e3](https://github.com/wwwhynot3/slippy-diff/commit/9a446e3b1e39b19c85d29b983de893d278bb354b))


### Bug Fixes

* avoid api-only latest release lookup ([71ff0ba](https://github.com/wwwhynot3/slippy-diff/commit/71ff0ba4a32604cfe45383881e20ed1d3f7ff311))
* use release tags in linux installer ([1a32ad6](https://github.com/wwwhynot3/slippy-diff/commit/1a32ad6f988a95c9af4b29107f1ad2123d86c93f))

## [0.2.3](https://github.com/wwwhynot3/slippy-diff/compare/slippy-v0.2.2...slippy-v0.2.3) (2026-06-29)


### Bug Fixes

* package wayland appimages and macos dmg ([a8f01b7](https://github.com/wwwhynot3/slippy-diff/commit/a8f01b7849778971de6ad6746146b6c405f31c10))

## [0.2.2](https://github.com/wwwhynot3/slippy-diff/compare/slippy-v0.2.1...slippy-v0.2.2) (2026-06-29)


### Bug Fixes

* restore linux arm64 release builds ([c89a416](https://github.com/wwwhynot3/slippy-diff/commit/c89a416f313c0567b436edb6512b6909d1039b29))

## [0.2.1](https://github.com/wwwhynot3/slippy-diff/compare/slippy-v0.2.0...slippy-v0.2.1) (2026-06-29)


### Bug Fixes

* restore linux release builds ([2493b79](https://github.com/wwwhynot3/slippy-diff/commit/2493b7900cb376ad78485792779b387c6a31c7a3))

## [0.2.0](https://github.com/wwwhynot3/slippy-diff/compare/slippy-v0.1.0...slippy-v0.2.0) (2026-06-29)


### Features

* add a live theme toggle button ([728a578](https://github.com/wwwhynot3/slippy-diff/commit/728a578e905ba9c990d8f3d3ca460d03d776dca1))
* add change-region computation to diff view ([1b8f8a8](https://github.com/wwwhynot3/slippy-diff/commit/1b8f8a8df10a7f1e562c3ba644477dc648c0bd97))
* add diff character selection extraction ([e80bd70](https://github.com/wwwhynot3/slippy-diff/commit/e80bd70d563014fc2edbb8243a60cb0fb67a8ac3))
* add diff overview rail marks ([42f061d](https://github.com/wwwhynot3/slippy-diff/commit/42f061dbf726e978533026f6716c5555656756d1))
* add diff selection UI helpers ([cadcfcc](https://github.com/wwwhynot3/slippy-diff/commit/cadcfcc02fb1dbcd340df2c89610931dc184ce15))
* add semantic diff view model ([efad591](https://github.com/wwwhynot3/slippy-diff/commit/efad591b0b35a89863ac31db7ead354373025e6f))
* add Slippy app logo ([07cc5b1](https://github.com/wwwhynot3/slippy-diff/commit/07cc5b1cbddf534efa89ba2c2d0188cdecf23233))
* add unified diff toolbar ([8f88f70](https://github.com/wwwhynot3/slippy-diff/commit/8f88f70f50bb27c542356d296f23d5acda8f40c5))
* **config:** add sanitized DiffOverrides block to AppConfig ([91ad5b4](https://github.com/wwwhynot3/slippy-diff/commit/91ad5b402c136381aade7d6cb80c428a86d34da7))
* **diff_core:** add DiffOp enum and char-level segment/ratio helpers ([b16eb2d](https://github.com/wwwhynot3/slippy-diff/commit/b16eb2dceb5d9102b19164f0a2d886a560ff56e8))
* **diff_core:** add DiffOptions tunables struct with defaults ([5ee00e0](https://github.com/wwwhynot3/slippy-diff/commit/5ee00e03c3b39d6aac92a63c61f00c54ad525aad))
* **diff_core:** render_unified_diff over DisplayDiff for copy ([84d137b](https://github.com/wwwhynot3/slippy-diff/commit/84d137bdc7fd8a9b7bd53ad3e2c9bd622e12edd8))
* **diff_core:** similarity-weighted line alignment (build_display_diff) ([dc298be](https://github.com/wwwhynot3/slippy-diff/commit/dc298be34953ab0490c00d151e3645e1e39d7df7))
* draw character selection in diff canvas ([93b29fc](https://github.com/wwwhynot3/slippy-diff/commit/93b29fceb2b96c984a1bde8b154a5aa5640389a2))
* draw diff canvas and input gutters ([9c8cd29](https://github.com/wwwhynot3/slippy-diff/commit/9c8cd29ad8a07e61e28fd492cd2bce819e2cd393))
* highlight current diff change on the canvas ([4247d66](https://github.com/wwwhynot3/slippy-diff/commit/4247d66b39c8e6a64809481eadf5babe0f867616))
* navigate diff changes with prev/next buttons ([77f6811](https://github.com/wwwhynot3/slippy-diff/commit/77f68111da6ebc84a21c4097d7e4a9182092f450))
* persist pin and split state, add compact chrome ([bbd4bd2](https://github.com/wwwhynot3/slippy-diff/commit/bbd4bd2d470af3b30d2553b57d5b38119ce45980))
* render semantic unified diff text ([a370129](https://github.com/wwwhynot3/slippy-diff/commit/a3701291705c7a544c38131391ee90b3f6a69648))
* select and copy rows from the diff canvas ([116c9e0](https://github.com/wwwhynot3/slippy-diff/commit/116c9e01d1ef503144d706652f8a870a31953f8b))
* show character count for copied lines ([94e0f59](https://github.com/wwwhynot3/slippy-diff/commit/94e0f598acec895a97a751f7ed18bd1d64dccbd6))
* **ui:** IntelliJ-style bg-color op-driven diff renderer with adaptive folding ([c13609c](https://github.com/wwwhynot3/slippy-diff/commit/c13609c93cf36237e82d987ef2bcc1706e13da77))
* wire character selection copy into diff canvas ([f4ee82d](https://github.com/wwwhynot3/slippy-diff/commit/f4ee82d38377d67097fcb93a36c09d81ef86cd31))


### Bug Fixes

* improve character selection copying ([45950ff](https://github.com/wwwhynot3/slippy-diff/commit/45950fffd06768ee2d519653b59b8e07667a5a14))
* keep blank overview rail slots ([493df4c](https://github.com/wwwhynot3/slippy-diff/commit/493df4ccaa8df748af0b4443b263f1510afb61fd))
* keep clipboard handle alive so Copy Diff pastes on Linux ([cd092e4](https://github.com/wwwhynot3/slippy-diff/commit/cd092e4d858e59b0c4183dd8f868d7166640e1c4))
* order deletions before insertions in the diff view ([56f91b1](https://github.com/wwwhynot3/slippy-diff/commit/56f91b142e9804d3a11b10732b571823342523ae))
* render Copy Diff as a standard unified diff ([74565f1](https://github.com/wwwhynot3/slippy-diff/commit/74565f1b83bcbf2bde8e0b237e8035622a806c21))
* **ui_fltk:** keep styles aligned on stale-diff path; drop unused palette args ([54f42db](https://github.com/wwwhynot3/slippy-diff/commit/54f42db34a2a22712448abee00c38c9359a278a0))
* use system linker for linux fltk builds ([e14f075](https://github.com/wwwhynot3/slippy-diff/commit/e14f0751da97ea53430ad30a14312a3e790847e7))
