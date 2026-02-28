# 開発手順

## 前提

- macOS
- Rust toolchain

## 開発起動

```bash
cargo run
```

## 表示設定

Homebrewアプリとしての通常運用では、設定ファイルに保存して管理します。

設定ファイル:
- 既定パス: `~/Library/Application Support/cliip-show/config.toml`
- パス変更: `CLIIP_SHOW_CONFIG_PATH=/path/to/config.toml`

初期化と確認:

```bash
cliip-show --config init
cliip-show --config show
```

設定値を保存:

```bash
cliip-show --config set hud_duration_secs 2.5
cliip-show --config set max_lines 3
cliip-show --config set hud_position top
cliip-show --config set hud_scale 1.2
cliip-show --config set hud_background_color blue
```

設定キー:
- `poll_interval_secs`（既定値: `0.3`、`0.05` - `5.0`）
- `hud_duration_secs`（既定値: `1.0`、`0.1` - `10.0`）
- `max_chars_per_line`（既定値: `100`、`1` - `500`）
- `max_lines`（既定値: `5`、`1` - `20`）
- `hud_position`（既定値: `center`、`top` / `center` / `bottom`）
- `hud_scale`（既定値: `1.0`、`0.5` - `2.0`）
- `hud_background_color`（既定値: `default`、`default` / `yellow` / `blue` / `green` / `red` / `purple`）

環境変数でも上書き可能です（設定ファイルより優先）。

```bash
CLIIP_SHOW_HUD_DURATION_SECS=2.5 \
CLIIP_SHOW_MAX_LINES=3 \
CLIIP_SHOW_HUD_POSITION=top \
CLIIP_SHOW_HUD_SCALE=1.2 \
CLIIP_SHOW_HUD_BACKGROUND_COLOR=blue \
cargo run
```

## `.app` 化して動作確認

ローカルで `.app` として起動確認したい場合のみ実行してください。  
通常は Homebrew 経由での利用を想定しています。

```bash
cargo install cargo-bundle
cargo bundle --release
open target/release/bundle/osx/cliip-show.app
```

## ビジュアルリグレッションテスト

HUDの描画結果をPNGで比較します。

### 実行方法

```bash
# 初回または意図的なUI変更時にベースラインを更新
./scripts/visual_regression.sh --update

# 通常の差分チェック
./scripts/visual_regression.sh
```

このスクリプトは以下の観点を比較します。

- デフォルト設定での表示
- 設定プロファイルごとの表示（例: `max_lines=2`, `max_chars_per_line=24`）

### 生成物

- `tests/visual/baseline/*.png`: 比較基準となるベースライン画像
- `tests/visual/artifacts/*.current.png`: 現在の描画結果
- `tests/visual/artifacts/*.diff.png`: 差分を赤で強調した画像（差分がある場合）

### 判定ルール

- 判定はピクセル差分率で行います
- 既定の許容値は `MAX_DIFF_PERMILLE=120`（12%）です
- 必要に応じて環境変数で調整できます

```bash
MAX_DIFF_PERMILLE=80 ./scripts/visual_regression.sh
```

### 運用ルール

- 通常のPRでは `./scripts/visual_regression.sh` のみ実行
- 意図したUI変更を入れたPRのみ `./scripts/visual_regression.sh --update` を実行
- CI失敗時は `visual-regression-artifacts` の diff 画像を確認

## Homebrewで公開する手順

### 1. バイナリのバージョンを更新する

`Cargo.toml` の `package.version` をリリース対象バージョンに更新します。

例: `0.1.0` から `0.1.1` に更新する

```toml
[package]
version = "0.1.1"
```

`Cargo.toml` の更新をコミットして push してから、次の手順に進んでください。

### 2. タグを作成して push する

例: `v0.1.1` タグを作成して push する

```bash
git tag v0.1.1
git push origin v0.1.1
```

タグのバージョンは `Cargo.toml` の `version` と同じ値にしてください（例: `0.1.1` -> `v0.1.1`）。

### 3. Homebrew tap リポジトリ

<https://github.com/somei-san/homebrew-tools>

### 4. Formulaを生成する

このリポジトリで以下を実行します。

```bash
./scripts/homebrew/generate_formula.sh somei-san 0.1.1 ./Formula/cliip-show.rb
```

バージョンは `0.1.1` のように `v` なしで指定してください（タグは内部で `v0.1.1` として参照されます）。

生成された `Formula/cliip-show.rb` を [tap リポジトリ](https://github.com/somei-san/homebrew-tools) の `Formula/cliip-show.rb` としてコミットして push してください。

テンプレートは `packaging/homebrew/cliip-show.rb.template` にあります。

### 5. ユーザーのインストール手順

[TapリポジトリのREADME参照](https://github.com/somei-san/homebrew-tools/blob/main/README.md)
