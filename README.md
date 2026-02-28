# cliip-show

## 背景

コピーしたと思ったのにできてなかった 😡

ペーストしたら意図したコピー内容と違った 😡 😡

そんなことありませんか？

ありますよねぇ〜 🤓

てなわけで、

コピーされたプレーンテキストを画面中央に約1秒だけ表示する、macOS向けの常駐アプリ`cliip-show`です 🐟

## 概要

- クリップボードの更新を監視し、コピー直後にHUD表示します
- HUDは数秒で自動的に消えます
- アプリはバックグラウンドで常駐して動作します

## 動作環境

- macOS（AppKitを使用）
- Homebrew（通常利用時のインストール手段）
- Rust toolchain（開発・ビルド時のみ）

## HUDイメージ

![cliip-show HUDの表示イメージ](docs/assets/cliip-show-hud.png)

## インストール手順 （Homebrew経由）

```bash
brew tap somei-san/tools
brew install somei-san/tools/cliip-show
brew services start cliip-show
```

## 開発者向け

### 開発起動

```bash
cargo run
```

### .app化して動作確認

ローカルで `.app` として起動確認したい場合のみ実行してください。  
通常は Homebrew 経由での利用を想定しています。

```bash
cargo install cargo-bundle
cargo bundle --release
open target/release/bundle/osx/cliip-show.app
```

### ビジュアルリグレッションテスト

HUDの描画結果をPNGで比較します。

```bash
# 初回または意図的なUI変更時にベースラインを更新
./scripts/visual_regression.sh --update

# 通常の差分チェック
./scripts/visual_regression.sh
```

差分がある場合は `tests/visual/artifacts/*.current.png` と `*.diff.png` が出力されます。  
`*.diff.png` は差分箇所を赤で強調表示します。

運用ルール:
- 通常のPRでは `./scripts/visual_regression.sh` のみ実行
- 意図したUI変更を入れたPRだけ `./scripts/visual_regression.sh --update` でベースラインを更新
- GitHub側で `visual-regression` ワークフローを必須チェックに設定して運用

## Homebrewで公開する手順

[Homebrewで公開する手順](docs/Homebrewで公開する手順.md)

## Homebrew tapリポジトリ

<https://github.com/somei-san/homebrew-tools>
