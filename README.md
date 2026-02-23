# clip-show

## 背景

コピーしたと思ったのにできてなかった！ペーストしたら意図したコピー内容と違った！！

😡 😡 😡

そんなことありませんか？

ありますよねぇ〜

てなわけで、

クリップボードにコピーしたときにコピー内容が表示されるアプリ `clip-show`

のソースコードです

## 概要

コピーされたプレーンテキストを画面中央に1秒だけHUD表示する、macOS常駐アプリです。

## 開発起動

```bash
cargo run
```

## .app化

```bash
cargo install cargo-bundle
cargo bundle --release
open target/release/bundle/osx/clip-show.app
```

普通はHomebrew経由でインストールするので不要。

## Homebrew tapリポジトリ

<https://github.com/somei-san/homebrew-tools>

[README](https://github.com/somei-san/homebrew-tools/README.md)

## ドキュメント

- Homebrew公開と常駐起動の手順: <https://github.com/somei-san/clip-show/blob/main/docs/Homebrewで公開する手順.md>
