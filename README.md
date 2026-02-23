# cliip-show

## 背景

コピーしたと思ったのにできてなかった 😡 ペーストしたら意図したコピー内容と違った 😡 😡

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

![cliip-show HUDの表示イメージ](docs/assets/cliip-show-hud.svg)

## 開発起動

```bash
cargo run
```

## 開発者向け: .app化して動作確認

ローカルで `.app` として起動確認したい場合のみ実行してください。  
通常は Homebrew 経由での利用を想定しています。

```bash
cargo install cargo-bundle
cargo bundle --release
open target/release/bundle/osx/cliip-show.app
```

## Homebrew tapリポジトリ

<https://github.com/somei-san/homebrew-tools>

[README](https://github.com/somei-san/homebrew-tools/README.md)


## ドキュメント

- [Homebrew公開と常駐起動の手順](docs/Homebrewで公開する手順.md)
